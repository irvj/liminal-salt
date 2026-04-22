//! System prompt assembly. Full implementation per the CLAUDE.md context
//! order:
//!
//! 1. Persona identity `.md` files
//! 2. Persona-scoped context files (uploaded + local dirs)
//! 3. Global user context files (uploaded + local dirs)
//! 4. Scenario — roleplay threads only
//! 5. Thread memory — per-thread running summary
//! 6. Persona memory — chatbot threads only (suppressed in roleplay)
//!
//! Persona memory writes are still the responsibility of a Phase 5 service
//! (`memory_manager.rs`); this module only reads the resulting markdown file.

use std::path::Path;

use crate::services::{
    context_files::ContextScope,
    memory, persona,
    session::{Mode, Session},
};

/// Build the full system prompt for a chat turn.
pub async fn build_system_prompt(data_dir: &Path, session: &Session) -> String {
    let mut out = String::new();

    // 1. Persona identity.
    let identity_path = persona::persona_dir(data_dir, &session.persona);
    match collect_identity_files(&identity_path).await {
        Ok(files) if !files.is_empty() => {
            for (filename, body) in files {
                out.push_str(&format!("--- SYSTEM INSTRUCTION: {filename} ---\n"));
                out.push_str(&body);
                out.push_str("\n\n");
            }
        }
        _ => {
            out.push_str("--- WARNING: Persona not found ---\n");
            out.push_str(&format!(
                "Expected directory: {}\n\n",
                identity_path.display()
            ));
        }
    }

    // 2. Persona-scoped context (uploaded + local).
    let persona_scope = ContextScope::persona(data_dir, &session.persona);
    let persona_ctx = persona_scope.load_enabled_context().await;
    if !persona_ctx.is_empty() {
        out.push_str(&persona_ctx);
        out.push_str("\n\n");
    }

    // 3. Global user context (uploaded + local).
    let global_scope = ContextScope::global(data_dir);
    let global_ctx = global_scope.load_enabled_context().await;
    if !global_ctx.is_empty() {
        out.push_str(&global_ctx);
        out.push_str("\n\n");
    }

    // 4. Scenario (roleplay only).
    if session.mode == Mode::Roleplay
        && session.scenario.as_deref().is_some_and(|s| !s.is_empty())
    {
        out.push_str("--- SCENARIO ---\n");
        out.push_str(
            "The following defines the scenario for this specific thread. \
             Treat it as authoritative setup for the conversation.\n\n",
        );
        out.push_str(session.scenario.as_deref().unwrap());
        out.push_str("\n\n");
    }

    // 5. Thread memory.
    if !session.thread_memory.is_empty() {
        out.push_str("--- THREAD SUMMARY ---\n");
        out.push_str(
            "Running summary of what has happened in this thread so far. \
             Use this to keep continuity as older messages fall out of the \
             rolling window.\n\n",
        );
        out.push_str(&session.thread_memory);
        out.push_str("\n\n");
    }

    // 6. Persona memory (chatbot threads only — suppressed in roleplay to
    //    preserve immersion; a fictional persona shouldn't know real-user
    //    biographical facts mid-scene).
    if session.mode == Mode::Chatbot {
        let memory_path = memory::memory_file(data_dir, &session.persona);
        if let Ok(body) = tokio::fs::read_to_string(&memory_path).await {
            let trimmed = body.trim();
            if !trimmed.is_empty() {
                out.push_str("--- YOUR MEMORY ABOUT THIS USER ---\n");
                out.push_str(
                    "The following is your memory about the person you're talking to. \
                     It is written to you, about them — these are things you know, \
                     have observed, and carry from previous conversations.\n\n",
                );
                out.push_str(trimmed);
                out.push_str("\n\n");
            }
        }
    }

    out.trim().to_string()
}

async fn collect_identity_files(persona_path: &Path) -> std::io::Result<Vec<(String, String)>> {
    let mut entries = tokio::fs::read_dir(persona_path).await?;
    let mut files = Vec::new();
    while let Some(entry) = entries.next_entry().await? {
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.ends_with(".md") {
            continue;
        }
        let body = tokio::fs::read_to_string(entry.path()).await?;
        files.push((name, body));
    }
    files.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(files)
}

/// Returns the directory names under `<data_dir>/personas/` that contain at
/// least one `.md` file. Re-exported for handlers that still import this
/// symbol from prompt; internally delegates to `persona::list_personas`.
pub async fn available_personas(data_dir: &Path) -> Vec<String> {
    persona::list_personas(data_dir).await
}

/// On startup, copy bundled default personas from `chat/default_personas/` into
/// `<data_dir>/personas/` if the persona doesn't already exist.
pub async fn seed_default_personas(data_dir: &Path, bundled_dir: &Path) {
    let target_root = data_dir.join("personas");
    if let Err(err) = tokio::fs::create_dir_all(&target_root).await {
        tracing::warn!(?target_root, error = %err, "could not create personas dir");
        return;
    }
    let mut entries = match tokio::fs::read_dir(bundled_dir).await {
        Ok(e) => e,
        Err(_) => return,
    };
    while let Ok(Some(entry)) = entries.next_entry().await {
        let Ok(ft) = entry.file_type().await else { continue };
        if !ft.is_dir() {
            continue;
        }
        let name = entry.file_name();
        let target = target_root.join(&name);
        if tokio::fs::try_exists(&target).await.unwrap_or(false) {
            continue;
        }
        if let Err(err) = copy_dir(&entry.path(), &target).await {
            tracing::warn!(persona = ?name, error = %err, "default persona copy failed");
        } else {
            tracing::info!(persona = ?name, "seeded default persona");
        }
    }
}

async fn copy_dir(src: &Path, dst: &Path) -> std::io::Result<()> {
    tokio::fs::create_dir_all(dst).await?;
    let mut entries = tokio::fs::read_dir(src).await?;
    while let Some(entry) = entries.next_entry().await? {
        let ft = entry.file_type().await?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if ft.is_dir() {
            Box::pin(copy_dir(&from, &to)).await?;
        } else if ft.is_file() {
            tokio::fs::copy(&from, &to).await?;
        }
    }
    Ok(())
}
