//! Integration tests for `services::memory`. Exercises file I/O, model
//! resolution, and the three LLM-driven operations against a fake LLM. No
//! network traffic, all state in tempdirs.

use std::sync::{Arc, Mutex};

use liminal_salt::services::llm::{ChatLlm, LlmError, LlmMessage};
use liminal_salt::services::memory;
use liminal_salt::services::persona::PersonaConfig;
use liminal_salt::services::session::{Message, Role, ThreadSnapshot};

/// Fake LLM that returns a canned response and records the prompts it saw.
/// Lets tests assert both the output-side behavior AND the shape of the
/// prompt memory.rs constructed.
struct FakeLlm {
    response: String,
    seen: Mutex<Vec<String>>,
}

impl FakeLlm {
    fn new(response: impl Into<String>) -> Arc<Self> {
        Arc::new(Self {
            response: response.into(),
            seen: Mutex::new(Vec::new()),
        })
    }

    fn last_prompt(&self) -> String {
        self.seen
            .lock()
            .unwrap()
            .last()
            .cloned()
            .unwrap_or_default()
    }
}

impl ChatLlm for FakeLlm {
    async fn complete(&self, messages: &[LlmMessage]) -> Result<String, LlmError> {
        // The merge prompt is always a single user message; record it.
        if let Some(m) = messages.first() {
            self.seen.lock().unwrap().push(m.content.clone());
        }
        Ok(self.response.clone())
    }
}

struct FailingLlm;

impl ChatLlm for FailingLlm {
    async fn complete(&self, _: &[LlmMessage]) -> Result<String, LlmError> {
        Err(LlmError::BadResponse("simulated".into()))
    }
}

fn msg(role: Role, content: &str) -> Message {
    Message {
        role,
        content: content.to_string(),
        timestamp: "2026-04-22T10:00:00.000000Z".to_string(),
    }
}

// =============================================================================
// File I/O
// =============================================================================

#[tokio::test]
async fn save_then_get_round_trips() {
    let tmp = tempfile::tempdir().unwrap();
    assert!(memory::get_memory_content(tmp.path(), "assistant").await.is_empty());

    assert!(memory::save_memory_content(tmp.path(), "assistant", "hello\nworld").await);

    let got = memory::get_memory_content(tmp.path(), "assistant").await;
    assert_eq!(got, "hello\nworld");
}

#[tokio::test]
async fn invalid_persona_name_rejected_at_every_entry() {
    let tmp = tempfile::tempdir().unwrap();
    // Traversal / empty / hyphen — all rejected, never touch the filesystem.
    assert!(memory::get_memory_content(tmp.path(), "../escape").await.is_empty());
    assert!(!memory::save_memory_content(tmp.path(), "", "hi").await);
    assert!(!memory::save_memory_content(tmp.path(), "has-hyphen", "hi").await);
    assert!(!memory::delete_memory(tmp.path(), "../escape").await);
    assert!(!memory::rename_memory(tmp.path(), "../a", "b").await);
    assert!(!memory::rename_memory(tmp.path(), "a", "../b").await);
    // Memory dir should not have been created by any of those calls.
    assert!(!memory::memory_dir(tmp.path()).exists());
}

#[tokio::test]
async fn delete_missing_memory_is_ok() {
    let tmp = tempfile::tempdir().unwrap();
    assert!(memory::delete_memory(tmp.path(), "ghost").await);
}

#[tokio::test]
async fn rename_moves_file_and_handles_missing_source() {
    let tmp = tempfile::tempdir().unwrap();
    assert!(memory::save_memory_content(tmp.path(), "old_name", "body").await);

    assert!(memory::rename_memory(tmp.path(), "old_name", "new_name").await);
    assert!(memory::get_memory_content(tmp.path(), "old_name").await.is_empty());
    assert_eq!(memory::get_memory_content(tmp.path(), "new_name").await, "body");

    // Missing source → no-op success. Matches persona.rs rename's best-effort cascade.
    assert!(memory::rename_memory(tmp.path(), "never_existed", "still_gone").await);
}

#[tokio::test]
async fn list_persona_memories_sorts_and_strips_suffix() {
    let tmp = tempfile::tempdir().unwrap();
    memory::save_memory_content(tmp.path(), "zebra", "x").await;
    memory::save_memory_content(tmp.path(), "alpha", "x").await;
    memory::save_memory_content(tmp.path(), "mike", "x").await;

    let names = memory::list_persona_memories(tmp.path()).await;
    assert_eq!(names, vec!["alpha", "mike", "zebra"]);
}

// =============================================================================
// Model resolution
// =============================================================================

#[tokio::test]
async fn get_memory_model_walks_fallback_chain() {
    let cfg_with = PersonaConfig {
        model: Some("persona/x".to_string()),
        ..Default::default()
    };
    let cfg_empty = PersonaConfig::default();

    assert_eq!(
        memory::get_memory_model(Some("explicit/y"), &cfg_with, "default/z"),
        "explicit/y"
    );
    assert_eq!(
        memory::get_memory_model(None, &cfg_with, "default/z"),
        "persona/x"
    );
    assert_eq!(
        memory::get_memory_model(None, &cfg_empty, "default/z"),
        "default/z"
    );
}

// =============================================================================
// LLM-driven operations
// =============================================================================

#[tokio::test]
async fn update_memory_empty_threads_returns_false() {
    let tmp = tempfile::tempdir().unwrap();
    let llm = FakeLlm::new("updated memory");
    let ok = memory::update_memory(&*llm, tmp.path(), "assistant", "identity", &[], 8000).await;
    assert!(!ok);
    // Nothing was written.
    assert!(memory::get_memory_content(tmp.path(), "assistant").await.is_empty());
}

#[tokio::test]
async fn update_memory_writes_file_and_builds_expected_prompt() {
    let tmp = tempfile::tempdir().unwrap();
    let llm = FakeLlm::new("## Them\nThey like hiking.");

    let threads = vec![ThreadSnapshot {
        title: "Hike plans".to_string(),
        persona: "assistant".to_string(),
        messages: vec![
            msg(Role::User, "Planning a hike this weekend"),
            msg(Role::Assistant, "Sounds great"),
        ],
    }];

    let ok = memory::update_memory(
        &*llm,
        tmp.path(),
        "carl_sagan",
        "You are Carl Sagan.",
        &threads,
        8000,
    )
    .await;
    assert!(ok);

    let written = memory::get_memory_content(tmp.path(), "carl_sagan").await;
    assert_eq!(written, "## Them\nThey like hiking.");

    // Prompt shape checks: identity, display name, threads, and size target.
    let p = llm.last_prompt();
    assert!(p.contains("You are Carl Sagan"));
    assert!(p.contains("Below is your identity"));
    assert!(p.contains("RECENT CONVERSATIONS"));
    assert!(p.contains("=== THREAD 1: Hike plans ==="));
    assert!(p.contains("User: Planning a hike this weekend"));
    assert!(p.contains("Carl Sagan: Sounds great"));
    assert!(p.contains("ROLEPLAY AWARENESS"));
    assert!(p.contains("SIZE TARGET: Aim for roughly 8000 characters"));
}

#[tokio::test]
async fn update_memory_size_limit_zero_omits_size_target() {
    let tmp = tempfile::tempdir().unwrap();
    let llm = FakeLlm::new("body");
    let threads = vec![ThreadSnapshot {
        title: "t".to_string(),
        persona: "a".to_string(),
        messages: vec![msg(Role::User, "hi")],
    }];
    memory::update_memory(&*llm, tmp.path(), "assistant", "id", &threads, 0).await;
    assert!(!llm.last_prompt().contains("SIZE TARGET"));
}

#[tokio::test]
async fn seed_memory_uses_seed_label_and_omits_roleplay_section() {
    let tmp = tempfile::tempdir().unwrap();
    let llm = FakeLlm::new("seeded body");
    let ok = memory::seed_memory(
        &*llm,
        tmp.path(),
        "assistant",
        "identity",
        "User bio: lives in Portland.",
        8000,
    )
    .await;
    assert!(ok);
    let p = llm.last_prompt();
    assert!(p.contains("NEW INFORMATION FROM THE USER"));
    assert!(p.contains("User bio: lives in Portland"));
    // Roleplay section is only on update_memory.
    assert!(!p.contains("ROLEPLAY AWARENESS"));
}

#[tokio::test]
async fn modify_memory_refuses_when_no_existing_memory() {
    let tmp = tempfile::tempdir().unwrap();
    let llm = FakeLlm::new("would-be new body");

    let ok = memory::modify_memory(
        &*llm,
        tmp.path(),
        "assistant",
        "identity",
        "Forget their birthday",
        8000,
    )
    .await;
    assert!(!ok);
    // LLM not invoked; no file written.
    assert!(llm.last_prompt().is_empty());
    assert!(memory::get_memory_content(tmp.path(), "assistant").await.is_empty());
}

#[tokio::test]
async fn modify_memory_uses_command_label_when_memory_exists() {
    let tmp = tempfile::tempdir().unwrap();
    memory::save_memory_content(tmp.path(), "assistant", "They love cats.").await;

    let llm = FakeLlm::new("They used to love cats.");
    let ok = memory::modify_memory(
        &*llm,
        tmp.path(),
        "assistant",
        "identity",
        "They don't love cats anymore",
        8000,
    )
    .await;
    assert!(ok);

    let p = llm.last_prompt();
    assert!(p.contains("USER'S COMMAND"));
    assert!(p.contains("They don't love cats anymore"));
    // Existing memory seeded the prompt.
    assert!(p.contains("They love cats."));

    let written = memory::get_memory_content(tmp.path(), "assistant").await;
    assert_eq!(written, "They used to love cats.");
}

#[tokio::test]
async fn short_response_rejected_when_existing_memory_is_substantial() {
    let tmp = tempfile::tempdir().unwrap();
    let existing = "a".repeat(200);
    memory::save_memory_content(tmp.path(), "assistant", &existing).await;

    let llm = FakeLlm::new("oops"); // 4 chars — under the 10-char threshold.
    let threads = vec![ThreadSnapshot {
        title: "t".to_string(),
        persona: "assistant".to_string(),
        messages: vec![msg(Role::User, "hi")],
    }];

    let ok =
        memory::update_memory(&*llm, tmp.path(), "assistant", "identity", &threads, 8000).await;
    assert!(!ok, "short response should be rejected");

    // Existing memory untouched.
    let still = memory::get_memory_content(tmp.path(), "assistant").await;
    assert_eq!(still, existing);
}

#[tokio::test]
async fn short_response_accepted_when_existing_memory_is_empty() {
    let tmp = tempfile::tempdir().unwrap();
    let llm = FakeLlm::new("tiny"); // 4 chars but no existing memory.
    let threads = vec![ThreadSnapshot {
        title: "t".to_string(),
        persona: "assistant".to_string(),
        messages: vec![msg(Role::User, "hi")],
    }];

    let ok =
        memory::update_memory(&*llm, tmp.path(), "assistant", "identity", &threads, 8000).await;
    assert!(ok);
    assert_eq!(
        memory::get_memory_content(tmp.path(), "assistant").await,
        "tiny"
    );
}

#[tokio::test]
async fn llm_error_returns_false_and_preserves_existing_memory() {
    let tmp = tempfile::tempdir().unwrap();
    memory::save_memory_content(tmp.path(), "assistant", "keep me").await;

    let threads = vec![ThreadSnapshot {
        title: "t".to_string(),
        persona: "assistant".to_string(),
        messages: vec![msg(Role::User, "hi")],
    }];

    let ok = memory::update_memory(
        &FailingLlm,
        tmp.path(),
        "assistant",
        "identity",
        &threads,
        8000,
    )
    .await;
    assert!(!ok);
    assert_eq!(
        memory::get_memory_content(tmp.path(), "assistant").await,
        "keep me"
    );
}

#[tokio::test]
async fn merge_includes_existing_memory_in_prompt() {
    let tmp = tempfile::tempdir().unwrap();
    memory::save_memory_content(tmp.path(), "assistant", "They have a dog named Sam.").await;

    let llm = FakeLlm::new("Updated with a cat now.");
    let threads = vec![ThreadSnapshot {
        title: "New pet".to_string(),
        persona: "assistant".to_string(),
        messages: vec![msg(Role::User, "got a cat")],
    }];
    memory::update_memory(&*llm, tmp.path(), "assistant", "identity", &threads, 8000).await;

    let p = llm.last_prompt();
    assert!(p.contains("--- YOUR EXISTING MEMORY ABOUT THE USER ---"));
    assert!(p.contains("They have a dog named Sam."));
}

#[tokio::test]
async fn merge_first_run_uses_beginning_placeholder() {
    let tmp = tempfile::tempdir().unwrap();
    let llm = FakeLlm::new("First entry.");
    let threads = vec![ThreadSnapshot {
        title: "t".to_string(),
        persona: "assistant".to_string(),
        messages: vec![msg(Role::User, "hi")],
    }];
    memory::update_memory(&*llm, tmp.path(), "assistant", "identity", &threads, 8000).await;

    let p = llm.last_prompt();
    assert!(p.contains("You do not have any memories yet. This is the beginning."));
}
