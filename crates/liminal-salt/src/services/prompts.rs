//! User-editable prompts. Owns `data/prompts/**`.
//!
//! Two layers:
//! - **Registry** (`PROMPTS`) — compile-time list of editable prompts. Drives
//!   the editor UI list and gates which IDs are valid for load/save/reset.
//! - **Filesystem** — `data/prompts/{id}.md` is the user's editable copy,
//!   seeded from `<bundled_dir>/{id}.md` on first boot. Existing user files
//!   are never overwritten by seeding. Reset reads the bundled default and
//!   overwrites the user copy.
//!
//! `bundled_dir` resolves at boot to `<crate manifest>/default_prompts/`; M4
//! (Tauri) will swap this for embedded assets.

use std::path::{Path, PathBuf};

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

fn default_file(bundled_dir: &Path, id: &str) -> PathBuf {
    bundled_dir.join(format!("{id}.md"))
}

// =============================================================================
// Public API
// =============================================================================

/// Load the user's editable copy. Falls back to the bundled default if the
/// user file is absent (e.g. user manually deleted it). `NotFound` only when
/// both are missing — that's a programmer error (registered id with no
/// bundled default shipped).
pub async fn load(data_dir: &Path, bundled_dir: &Path, id: &str) -> Result<String, PromptError> {
    if find(id).is_none() {
        return Err(PromptError::InvalidId(id.to_string()));
    }
    let user_path = prompt_file(data_dir, id);
    match tokio::fs::read_to_string(&user_path).await {
        Ok(s) => Ok(s),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            load_default(bundled_dir, id).await
        }
        Err(err) => Err(PromptError::Io(err)),
    }
}

/// Load the bundled default. `NotFound` if the bundled file is missing.
pub async fn load_default(bundled_dir: &Path, id: &str) -> Result<String, PromptError> {
    if find(id).is_none() {
        return Err(PromptError::InvalidId(id.to_string()));
    }
    let path = default_file(bundled_dir, id);
    match tokio::fs::read_to_string(&path).await {
        Ok(s) => Ok(s),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            Err(PromptError::NotFound(id.to_string()))
        }
        Err(err) => Err(PromptError::Io(err)),
    }
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
pub async fn reset(
    data_dir: &Path,
    bundled_dir: &Path,
    id: &str,
) -> Result<(), PromptError> {
    let content = load_default(bundled_dir, id).await?;
    save(data_dir, id, &content).await
}

/// On startup, copy any registered prompt that's missing its `data/prompts/`
/// copy from the bundled directory. Existing user files are never overwritten;
/// missing bundled defaults are logged but do not fail boot.
pub async fn seed_default_prompts(data_dir: &Path, bundled_dir: &Path) {
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
        let source = default_file(bundled_dir, meta.id);
        match tokio::fs::read(&source).await {
            Ok(bytes) => {
                if let Err(err) =
                    crate::services::fs::write_atomic(&target, &bytes).await
                {
                    tracing::warn!(prompt = meta.id, error = %err, "default prompt seed failed");
                } else {
                    tracing::info!(prompt = meta.id, "seeded default prompt");
                }
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                tracing::warn!(
                    prompt = meta.id,
                    ?source,
                    "bundled default prompt missing — registry/disk drift"
                );
            }
            Err(err) => {
                tracing::warn!(prompt = meta.id, error = %err, "default prompt read failed");
            }
        }
    }
}
