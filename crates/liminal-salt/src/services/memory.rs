//! Per-persona memory — file I/O + LLM merge/seed/modify.
//!
//! Owns `data/memory/{persona}.md`. Every write to that file goes through this
//! module. The three public LLM-driven operations are variants of the same
//! "existing memory + new data → updated memory via LLM" merge pattern:
//!
//! - `update_memory`: merge in recent conversation threads for the persona
//! - `seed_memory`: merge in a block of text the user uploaded
//! - `modify_memory`: apply a natural-language user command to existing memory
//!
//! The LLM is taken as a generic `ChatLlm`; this module never imports reqwest.

use std::path::{Path, PathBuf};

use tokio::io::AsyncWriteExt;

use crate::services::{
    llm::{ChatLlm, LlmMessage},
    persona::{self, PersonaConfig},
    session::{Role, ThreadSnapshot},
};

pub const DEFAULT_MEMORY_SIZE_LIMIT: u32 = 8000;

// =============================================================================
// Paths
// =============================================================================

pub fn memory_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("memory")
}

pub fn memory_file(data_dir: &Path, persona_name: &str) -> PathBuf {
    memory_dir(data_dir).join(format!("{persona_name}.md"))
}

// =============================================================================
// File I/O
// =============================================================================

/// Read a persona's memory. Returns "" when the file is missing, invalid name,
/// or read fails — matches the "Option-less read" convention the rest of the
/// service layer uses (see `persona::load_identity`).
pub async fn get_memory_content(data_dir: &Path, persona_name: &str) -> String {
    if !persona::valid_persona_name(persona_name) {
        return String::new();
    }
    let path = memory_file(data_dir, persona_name);
    match tokio::fs::read_to_string(&path).await {
        Ok(s) => s,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(err) => {
            tracing::error!(?path, error = %err, "memory read failed");
            String::new()
        }
    }
}

/// Durable write (write → flush → fsync). Creates `memory/` if absent.
pub async fn save_memory_content(data_dir: &Path, persona_name: &str, content: &str) -> bool {
    if !persona::valid_persona_name(persona_name) {
        return false;
    }
    let dir = memory_dir(data_dir);
    if let Err(err) = tokio::fs::create_dir_all(&dir).await {
        tracing::error!(?dir, error = %err, "memory dir create failed");
        return false;
    }
    let path = memory_file(data_dir, persona_name);
    let mut f = match tokio::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&path)
        .await
    {
        Ok(f) => f,
        Err(err) => {
            tracing::error!(?path, error = %err, "memory open failed");
            return false;
        }
    };
    if let Err(err) = f.write_all(content.as_bytes()).await {
        tracing::error!(?path, error = %err, "memory write failed");
        return false;
    }
    if let Err(err) = f.sync_all().await {
        tracing::error!(?path, error = %err, "memory fsync failed");
        return false;
    }
    true
}

/// Delete a persona's memory file. Missing file is treated as success.
pub async fn delete_memory(data_dir: &Path, persona_name: &str) -> bool {
    if !persona::valid_persona_name(persona_name) {
        return false;
    }
    let path = memory_file(data_dir, persona_name);
    match tokio::fs::remove_file(&path).await {
        Ok(()) => true,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => true,
        Err(err) => {
            tracing::error!(?path, error = %err, "memory delete failed");
            false
        }
    }
}

/// Rename memory on persona rename. Called from `persona::rename_persona`.
/// Missing source → no-op (true); used alongside the best-effort cascade.
pub async fn rename_memory(data_dir: &Path, old_name: &str, new_name: &str) -> bool {
    if !persona::valid_persona_name(old_name) || !persona::valid_persona_name(new_name) {
        return false;
    }
    let old_path = memory_file(data_dir, old_name);
    if !tokio::fs::try_exists(&old_path).await.unwrap_or(false) {
        return true;
    }
    let new_path = memory_file(data_dir, new_name);
    if let Some(parent) = new_path.parent() {
        let _ = tokio::fs::create_dir_all(parent).await;
    }
    match tokio::fs::rename(&old_path, &new_path).await {
        Ok(()) => true,
        Err(err) => {
            tracing::error!(old_name, new_name, error = %err, "memory rename failed");
            false
        }
    }
}

/// List persona names that have a memory file on disk. Sorted alphabetically.
pub async fn list_persona_memories(data_dir: &Path) -> Vec<String> {
    let dir = memory_dir(data_dir);
    let Ok(mut entries) = tokio::fs::read_dir(&dir).await else {
        return Vec::new();
    };
    let mut names = Vec::new();
    while let Ok(Some(entry)) = entries.next_entry().await {
        let filename = entry.file_name().to_string_lossy().to_string();
        if let Some(stem) = filename.strip_suffix(".md") {
            names.push(stem.to_string());
        }
    }
    names.sort();
    names
}

// =============================================================================
// Model resolution
// =============================================================================

/// Memory-operation model: explicit `MEMORY_MODEL` config override → persona's
/// `model` → the app's default. Matches Python's `get_memory_model`.
pub fn get_memory_model(
    memory_model: Option<&str>,
    persona_cfg: &PersonaConfig,
    default_model: &str,
) -> String {
    if let Some(m) = memory_model.filter(|s| !s.is_empty()) {
        return m.to_string();
    }
    if let Some(m) = persona_cfg.model.as_deref().filter(|s| !s.is_empty()) {
        return m.to_string();
    }
    default_model.to_string()
}

// =============================================================================
// LLM-driven operations
// =============================================================================

/// Merge recent conversation threads into this persona's cross-thread memory.
/// Returns true on success; false when `threads` is empty, the LLM errors,
/// or the response is suspiciously short relative to existing memory.
pub async fn update_memory<L: ChatLlm>(
    llm: &L,
    data_dir: &Path,
    persona_name: &str,
    identity: &str,
    threads: &[ThreadSnapshot],
    size_limit: u32,
) -> bool {
    if threads.is_empty() {
        return false;
    }
    let display = display_persona_name(persona_name);
    let transcript = format_threads(&display, threads);

    let roleplay_section = "ROLEPLAY AWARENESS:\n\
        Some conversations may be roleplay or creative writing. Signs include: the persona\n\
        name suggests a character, thread titles suggest fiction, messages are written in\n\
        character. For roleplay threads:\n\
        - Do NOT extract character traits as real user traits\n\
        - Instead, note what kind of stories/scenarios they enjoy\n\
        - The creative interests are real even if the content is fictional\n\n";

    merge_memory(
        llm,
        data_dir,
        persona_name,
        identity,
        size_limit,
        Variant {
            new_data_label: "RECENT CONVERSATIONS",
            new_data_content: &transcript,
            instructions_opener: "You are updating your personal memory. This is your inner monologue — notes\n\
                to yourself about the person you talk to. \"You\" always means you, the persona.\n\
                Refer to the user in third person (he/she/they). When you read this back,\n\
                it becomes your own inner knowledge.",
            extra_sections: roleplay_section,
        },
    )
    .await
}

/// Seed/merge a block of user-provided text into the persona's memory.
pub async fn seed_memory<L: ChatLlm>(
    llm: &L,
    data_dir: &Path,
    persona_name: &str,
    identity: &str,
    seed_content: &str,
    size_limit: u32,
) -> bool {
    merge_memory(
        llm,
        data_dir,
        persona_name,
        identity,
        size_limit,
        Variant {
            new_data_label: "NEW INFORMATION FROM THE USER",
            new_data_content: seed_content,
            instructions_opener:
                "You are updating your personal memory. The user has provided additional information\n\
                 they want you to know. This is your inner monologue — notes to yourself. \"You\"\n\
                 always means you, the persona. Refer to the user in third person (he/she/they).\n\
                 When you read this back, it becomes your own inner knowledge.",
            extra_sections: "",
        },
    )
    .await
}

/// Apply a natural-language user command to the persona's existing memory.
/// Returns false immediately if there's no existing memory to modify.
pub async fn modify_memory<L: ChatLlm>(
    llm: &L,
    data_dir: &Path,
    persona_name: &str,
    identity: &str,
    command: &str,
    size_limit: u32,
) -> bool {
    if get_memory_content(data_dir, persona_name).await.is_empty() {
        return false;
    }
    merge_memory(
        llm,
        data_dir,
        persona_name,
        identity,
        size_limit,
        Variant {
            new_data_label: "USER'S COMMAND",
            new_data_content: command,
            instructions_opener:
                "The user has asked you to modify your memory. Apply their request. If they ask\n\
                 to forget something, remove it. If they ask to add or change something, do so.\n\
                 This is your inner monologue — notes to yourself. \"You\" always means you, the\n\
                 persona. Refer to the user in third person (he/she/they). When you read this\n\
                 back, it becomes your own inner knowledge.",
            extra_sections: "",
        },
    )
    .await
}

// =============================================================================
// Helpers
// =============================================================================

/// `"carl_sagan"` → `"Carl Sagan"`. Mirrors Python's `persona.replace('_', ' ').title()`.
fn display_persona_name(persona_name: &str) -> String {
    persona_name
        .split('_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn format_threads(display_name: &str, threads: &[ThreadSnapshot]) -> String {
    let mut out = String::new();
    for (idx, thread) in threads.iter().enumerate() {
        if thread.messages.is_empty() {
            continue;
        }
        out.push_str(&format!("=== THREAD {}: {} ===\n", idx + 1, thread.title));
        for msg in &thread.messages {
            let label = match msg.role {
                Role::User => "User",
                _ => display_name,
            };
            out.push_str(label);
            out.push_str(": ");
            out.push_str(&msg.content);
            out.push('\n');
        }
        out.push('\n');
    }
    out
}

/// Variant-specific prompt parts. All three public ops share the same framing;
/// these fields are what actually differ (the data section header, the data
/// itself, the verb of the instruction, and any extra sections).
struct Variant<'a> {
    new_data_label: &'a str,
    new_data_content: &'a str,
    instructions_opener: &'a str,
    extra_sections: &'a str,
}

/// Shared merge engine for all three public operations. Builds the prompt,
/// runs the LLM, applies the short-output safety check, writes the file.
async fn merge_memory<L: ChatLlm>(
    llm: &L,
    data_dir: &Path,
    persona_name: &str,
    identity: &str,
    size_limit: u32,
    variant: Variant<'_>,
) -> bool {
    let Variant {
        new_data_label,
        new_data_content,
        instructions_opener,
        extra_sections,
    } = variant;
    if !persona::valid_persona_name(persona_name) {
        return false;
    }

    let existing = get_memory_content(data_dir, persona_name).await;
    let display = display_persona_name(persona_name);

    let size_instruction = if size_limit > 0 {
        format!(
            "SIZE TARGET: Aim for roughly {size_limit} characters. You can go over rather than\n\
             lose something important, but consolidate where you can. Quality over quantity.\n\n"
        )
    } else {
        String::new()
    };

    let existing_block = if existing.is_empty() {
        "You do not have any memories yet. This is the beginning.".to_string()
    } else {
        existing.clone()
    };

    let prompt = format!(
        "You are {display}. Below is your identity — who you are, how you\n\
         think, how you talk.\n\n\
         --- YOUR IDENTITY ---\n\
         {identity}\n\n\
         NOTE: Your identity above defines how you talk in conversation. But this task\n\
         is writing memory, not conversation. The memory must use standard capitalization\n\
         and punctuation regardless of your conversational style.\n\n\
         --- YOUR EXISTING MEMORY ABOUT THE USER ---\n\
         {existing_block}\n\n\
         --- {new_data_label} ---\n\
         {new_data_content}\n\n\
         --- INSTRUCTIONS ---\n\n\
         {instructions_opener}\n\n\
         This is not a clinical profile. It's what stuck. The things worth holding onto.\n\
         Write with your personality, your observations, your feelings about what matters.\n\n\
         MERGING RULES:\n\
         - READ your existing memory carefully. Most of it should survive.\n\
         - ADD new details, observations, and developments from the new information.\n\
         - REVISE entries that have been updated or corrected (e.g., they got a new job,\n  \
         changed an opinion, finished a project).\n\
         - COMPRESS patterns: if something has come up many times, consolidate it into\n  \
         a confident observation rather than listing each instance.\n\
         - LET STALE DETAILS FADE: if something minor hasn't come up in a while and\n  \
         isn't anchored by emotional weight, it's okay to drop it.\n\
         - KEEP VIVID ANCHORS: specific quotes, memorable moments, things said with\n  \
         emotional weight — these survive even if old.\n\
         - NEVER remove core identity facts (name, family, career, values) unless\n  \
         explicitly contradicted.\n\n\
         SECTIONS:\n\
         Use markdown ## headers for each section. Let sections emerge organically from what\n\
         you know about this person. Don't force a rigid template. Some natural sections\n\
         might include things like:\n\
         - How you two work together / your dynamic\n\
         - What's going on in their life\n\
         - Patterns you've noticed about them\n\
         - Things they've said that stuck with you\n\
         - People in their life\n\
         - Ongoing threads you're tracking\n\n\
         But these are suggestions, not requirements. Use whatever sections feel right for\n\
         what you actually know. If this is the first memory, start with what you learned.\n\
         If you've been talking a while, the structure will reflect the depth.\n\n\
         {extra_sections}\
         FORMAT:\n\
         - Write in standard, properly capitalized prose and markdown, using ## headers\n  \
         for sections. Do NOT adopt the persona's speaking style for the memory itself.\n\
         - PERSPECTIVE: \"You\" always means YOU, the persona — this is your inner monologue.\n  \
         Refer to the user in third person with pronouns (he/she/they — infer from context,\n  \
         default to \"they\" if unclear).\n  \
         CORRECT: \"You've noticed he tends to...\", \"She told you about...\", \"You feel like they...\"\n  \
         WRONG: \"You like reading\" (meaning the user likes reading) — this confuses who \"you\" is\n\
         - Be specific — names, details, quotes, not vague summaries\n\
         - No timestamps or meta-commentary about the update process\n\
         - No bullet-point databases — write like a person remembering, not a system logging\n\n\
         {size_instruction}\
         CRITICAL PERSPECTIVE CHECK — apply to every sentence you write:\n\
         - ALWAYS \"You\" for yourself: \"You noticed...\", \"You feel...\", \"You remember...\"\n\
         - NEVER \"I\": not \"I feel...\", \"I noticed...\", \"I think...\"\n\
         - ALWAYS third person for the user: \"He...\", \"She...\", \"They...\"\n\
         - NEVER second person for the user: not \"You like reading\" when meaning the user\n\
         If you catch yourself writing \"I\", rewrite it as \"You\".\n\n\
         Return ONLY the updated memory content. No preamble, no explanation."
    );

    let response = match llm
        .complete(&[LlmMessage::new(Role::User, prompt)])
        .await
    {
        Ok(r) => r,
        Err(err) => {
            tracing::error!(persona = persona_name, error = %err, "memory merge LLM call failed");
            return false;
        }
    };

    // Safety: don't replace substantial memory with a suspiciously short output.
    if response.len() < 10 && existing.len() > 50 {
        tracing::warn!(
            persona = persona_name,
            response_len = response.len(),
            "memory merge rejected: response too short"
        );
        return false;
    }

    save_memory_content(data_dir, persona_name, &response).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_name_basic() {
        assert_eq!(display_persona_name("assistant"), "Assistant");
        assert_eq!(display_persona_name("carl_sagan"), "Carl Sagan");
        assert_eq!(display_persona_name("my_persona_name"), "My Persona Name");
        assert_eq!(display_persona_name(""), "");
    }

    #[test]
    fn get_memory_model_fallback_chain() {
        let cfg_override = PersonaConfig {
            model: Some("persona/model".to_string()),
            ..PersonaConfig::default()
        };
        let cfg_empty = PersonaConfig::default();

        assert_eq!(
            get_memory_model(Some("explicit/memory"), &cfg_override, "default"),
            "explicit/memory"
        );
        assert_eq!(
            get_memory_model(None, &cfg_override, "default"),
            "persona/model"
        );
        assert_eq!(get_memory_model(None, &cfg_empty, "default"), "default");
        // Empty string is treated as "not set" — Python's `or` semantics.
        assert_eq!(get_memory_model(Some(""), &cfg_empty, "default"), "default");
    }
}
