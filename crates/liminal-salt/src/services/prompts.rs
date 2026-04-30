//! User-editable prompts. Owns `data/prompts/**`.
//!
//! Two layers:
//! - **Registry** (`PROMPTS`) — compile-time list of editable prompts. Drives
//!   the editor UI list and gates which IDs are valid for load/save/reset.
//! - **Filesystem** — `data/prompts/{id}.md` is the user's editable copy,
//!   seeded from the embedded `DefaultPrompts` bundle on first boot. Existing
//!   user files are never overwritten by seeding. Reset reads the bundled
//!   default and overwrites the user copy.
//!
//! Bundled defaults are compiled into the binary via `crate::assets::DefaultPrompts`
//! (rust-embed). In debug builds they're read from `crates/liminal-salt/default_prompts/`
//! on disk; in release and Tauri builds they're embedded.

use std::path::{Path, PathBuf};

use crate::assets::DefaultPrompts;

// =============================================================================
// Errors
// =============================================================================

#[derive(Debug, thiserror::Error)]
pub enum PromptError {
    #[error("invalid prompt id: {0}")]
    InvalidId(String),
    #[error("prompt not found: {0}")]
    NotFound(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

// =============================================================================
// Registry
// =============================================================================

/// One editable prompt. `id` doubles as the on-disk filename (`{id}.md`).
#[derive(Debug, Clone, Copy)]
pub struct PromptMeta {
    pub id: &'static str,
    pub display_name: &'static str,
    pub description: &'static str,
}

/// The closed set of editable prompts. Adding a prompt = adding a row here +
/// shipping the `default_prompts/{id}.md` file.
pub const PROMPTS: &[PromptMeta] = &[
    PromptMeta {
        id: "thread_memory_merge_chatbot",
        display_name: "Per-chat memory — update (chatbot)",
        description:
            "Instructions for folding new messages into a chat thread's running summary in chatbot mode.",
    },
    PromptMeta {
        id: "thread_memory_merge_roleplay",
        display_name: "Per-chat memory — update (roleplay)",
        description:
            "Instructions for folding new messages into a chat thread's running summary in roleplay mode.",
    },
    PromptMeta {
        id: "persona_memory_merge",
        display_name: "Long-term memory — update from conversations",
        description:
            "Instructions for folding recent conversations into a persona's long-term memory.",
    },
    PromptMeta {
        id: "persona_memory_seed",
        display_name: "Long-term memory — seed from uploaded text",
        description:
            "Instructions for merging user-supplied text into a persona's long-term memory.",
    },
    PromptMeta {
        id: "persona_memory_modify",
        display_name: "Long-term memory — apply user instruction",
        description:
            "Instructions for applying a natural-language user command to a persona's long-term memory.",
    },
];

pub fn list() -> &'static [PromptMeta] {
    PROMPTS
}

fn find(id: &str) -> Option<&'static PromptMeta> {
    PROMPTS.iter().find(|p| p.id == id)
}

// =============================================================================
// Paths
// =============================================================================

pub fn prompts_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("prompts")
}

fn prompt_file(data_dir: &Path, id: &str) -> PathBuf {
    prompts_dir(data_dir).join(format!("{id}.md"))
}

fn read_bundled(id: &str) -> Option<String> {
    let file = DefaultPrompts::get(&format!("{id}.md"))?;
    String::from_utf8(file.data.into_owned()).ok()
}

// =============================================================================
// Public API
// =============================================================================

/// Load the user's editable copy. Falls back to the bundled default if the
/// user file is absent (e.g. user manually deleted it). `NotFound` only when
/// both are missing — that's a programmer error (registered id with no
/// bundled default shipped).
pub async fn load(data_dir: &Path, id: &str) -> Result<String, PromptError> {
    if find(id).is_none() {
        return Err(PromptError::InvalidId(id.to_string()));
    }
    let user_path = prompt_file(data_dir, id);
    match tokio::fs::read_to_string(&user_path).await {
        Ok(s) => Ok(s),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => load_default(id),
        Err(err) => Err(PromptError::Io(err)),
    }
}

/// Load the bundled default. `NotFound` if the bundled file is missing —
/// programmer error (registry/disk drift).
pub fn load_default(id: &str) -> Result<String, PromptError> {
    if find(id).is_none() {
        return Err(PromptError::InvalidId(id.to_string()));
    }
    read_bundled(id).ok_or_else(|| PromptError::NotFound(id.to_string()))
}

/// Persist the user's edit (atomic write).
pub async fn save(data_dir: &Path, id: &str, content: &str) -> Result<(), PromptError> {
    if find(id).is_none() {
        return Err(PromptError::InvalidId(id.to_string()));
    }
    crate::services::fs::write_atomic(&prompt_file(data_dir, id), content.as_bytes()).await?;
    Ok(())
}

/// Restore the user copy from the bundled default.
pub async fn reset(data_dir: &Path, id: &str) -> Result<(), PromptError> {
    let content = load_default(id)?;
    save(data_dir, id, &content).await
}

/// On startup, materialize any registered prompt missing its `data/prompts/`
/// copy from the embedded bundle. Existing user files are never overwritten;
/// missing bundled defaults are logged but do not fail boot.
pub async fn seed_default_prompts(data_dir: &Path) {
    let target_root = prompts_dir(data_dir);
    if let Err(err) = tokio::fs::create_dir_all(&target_root).await {
        tracing::warn!(?target_root, error = %err, "could not create prompts dir");
        return;
    }
    for meta in PROMPTS {
        let target = prompt_file(data_dir, meta.id);
        if tokio::fs::try_exists(&target).await.unwrap_or(false) {
            continue;
        }
        let Some(content) = read_bundled(meta.id) else {
            tracing::warn!(
                prompt = meta.id,
                "bundled default prompt missing — registry/disk drift"
            );
            continue;
        };
        match crate::services::fs::write_atomic(&target, content.as_bytes()).await {
            Ok(()) => tracing::info!(prompt = meta.id, "seeded default prompt"),
            Err(err) => {
                tracing::warn!(prompt = meta.id, error = %err, "default prompt seed failed");
            }
        }
    }
}
