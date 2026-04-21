//! System prompt assembly. Phase 3 implementation is a stub that concatenates
//! persona identity `.md` files only. Phase 4 will add persona/global context
//! files; Phase 5 (chatbot threads) will append per-persona memory and
//! (roleplay threads) scenario + thread memory.

use std::path::{Path, PathBuf};

use crate::services::session::{Mode, Session};

/// Location of a persona's identity directory under the data root.
pub fn persona_dir(data_dir: &Path, persona_name: &str) -> PathBuf {
    data_dir.join("personas").join(persona_name)
}

/// Assemble the system prompt for a chat turn.
///
/// **Phase 3 stub scope:** persona identity `.md` files only, in alphabetical
/// order, each wrapped with a `--- SYSTEM INSTRUCTION: filename ---` header.
/// Other sections (context files, scenario, thread memory, persona memory) are
/// deferred to later phases — see `docs/planning/ARCHITECTURE_ROADMAP.md`.
pub async fn build_system_prompt(data_dir: &Path, session: &Session) -> String {
    let persona_path = persona_dir(data_dir, &session.persona);
    let mut out = String::new();

    match collect_identity_files(&persona_path).await {
        Ok(files) if !files.is_empty() => {
            for (filename, body) in files {
                out.push_str(&format!("--- SYSTEM INSTRUCTION: {filename} ---\n"));
                out.push_str(&body);
                out.push_str("\n\n");
            }
        }
        _ => {
            // Persona directory missing or empty — emit the same warning
            // shape Python does so the LLM gets a clear signal.
            out.push_str("--- WARNING: Persona not found ---\n");
            out.push_str(&format!("Expected directory: {}\n\n", persona_path.display()));
        }
    }

    // Even under the Phase 3 stub, roleplay threads should see their scenario
    // so the test corpus for Phase 3c can exercise both modes. Persona memory
    // is still suppressed — that section lands in Phase 5.
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
/// least one `.md` file. Sorted alphabetically.
pub async fn available_personas(data_dir: &Path) -> Vec<String> {
    let personas_root = data_dir.join("personas");
    let mut entries = match tokio::fs::read_dir(&personas_root).await {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };
    let mut names = Vec::new();
    while let Ok(Some(entry)) = entries.next_entry().await {
        if !entry.file_type().await.map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        let has_md = has_markdown(&entry.path()).await;
        if has_md {
            names.push(entry.file_name().to_string_lossy().to_string());
        }
    }
    names.sort();
    names
}

async fn has_markdown(dir: &Path) -> bool {
    let Ok(mut entries) = tokio::fs::read_dir(dir).await else {
        return false;
    };
    while let Ok(Some(entry)) = entries.next_entry().await {
        if entry
            .file_name()
            .to_string_lossy()
            .ends_with(".md")
        {
            return true;
        }
    }
    false
}

/// On startup, copy bundled default personas from `chat/default_personas/` into
/// `<data_dir>/personas/` if the persona doesn't already exist. Preserves the
/// first-launch experience from Python.
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
