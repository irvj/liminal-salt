//! End-to-end tests for `services::thread_memory::merge` against a fake LLM.
//! Mode dispatch, persona-memory injection, empty-input short-circuit, and the
//! short-output safety check all validated here; module-level unit tests in
//! `thread_memory.rs` cover prompt shape and settings resolver logic.

use std::path::PathBuf;
use std::sync::Mutex;

use liminal_salt::services::llm::{ChatLlm, LlmError, LlmMessage};
use liminal_salt::services::session::{Message, Mode, Role};
use liminal_salt::services::thread_memory::{self, MergeRequest};

fn data_dir(tmp: &tempfile::TempDir) -> PathBuf {
    tmp.path().join("data")
}

/// Common defaults for `MergeRequest`. Tests override only the fields they care
/// about via struct update syntax (`MergeRequest { foo: ..., ..default_req(...) }`).
/// Bundled prompt content is exercised via the embedded `DefaultPrompts` asset
/// (`prompts::load` reads it at runtime); no fixture needed here.
fn default_req<'a>(data: &'a std::path::Path, messages: &'a [Message]) -> MergeRequest<'a> {
    MergeRequest {
        data_dir: data,
        persona_display_name: "Clara",
        persona_memory: "",
        existing_memory: "",
        new_messages: messages,
        size_limit: 4000,
        mode: Mode::Chatbot,
    }
}

struct FakeLlm {
    response: String,
    seen: Mutex<Vec<String>>,
}

impl FakeLlm {
    fn new(response: impl Into<String>) -> Self {
        Self {
            response: response.into(),
            seen: Mutex::new(Vec::new()),
        }
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

fn msg(role: Role, content: &str, ts: &str) -> Message {
    Message {
        role,
        content: content.to_string(),
        timestamp: ts.to_string(),
    }
}

#[tokio::test]
async fn merge_empty_new_messages_returns_none() {
    let tmp = tempfile::tempdir().unwrap();
    let llm = FakeLlm::new("should not be used");
    let got =
        thread_memory::merge(&llm, default_req(&data_dir(&tmp), &[])).await;
    assert!(got.is_none());
    // LLM not invoked.
    assert!(llm.last_prompt().is_empty());
}

#[tokio::test]
async fn merge_chatbot_uses_chatbot_prompt_and_persona_memory_when_present() {
    let tmp = tempfile::tempdir().unwrap();
    let llm = FakeLlm::new("They settled on Tuesday.");
    let messages = vec![
        msg(Role::User, "let's meet Tuesday", "2026-04-22T10:00:00.000000Z"),
        msg(Role::Assistant, "works for me", "2026-04-22T10:00:01.000000Z"),
    ];
    let got = thread_memory::merge(
        &llm,
        MergeRequest {
            existing_memory: "They had been scheduling a meet.",
            persona_memory: "They're in Pacific time and avoid mornings.",
            ..default_req(&data_dir(&tmp), &messages)
        },
    )
    .await;
    assert_eq!(got.as_deref(), Some("They settled on Tuesday."));

    let p = llm.last_prompt();
    assert!(p.contains("You are Clara"));
    assert!(p.contains("WHAT YOU ALREADY KNOW ABOUT THIS PERSON"));
    assert!(p.contains("They're in Pacific time and avoid mornings."));
    assert!(p.contains("CURRENT THREAD SUMMARY"));
    assert!(p.contains("They had been scheduling a meet."));
    assert!(p.contains("User: let's meet Tuesday"));
    assert!(p.contains("Clara: works for me"));
    // PERSPECTIVE rules now live in the chatbot prompt's `.md`; verify they
    // reach the constructed prompt via the load path.
    assert!(p.contains("PERSPECTIVE — apply to every sentence"));
}

#[tokio::test]
async fn merge_chatbot_omits_persona_memory_section_when_empty() {
    let tmp = tempfile::tempdir().unwrap();
    let llm = FakeLlm::new("fine");
    let messages = vec![msg(Role::User, "hi", "2026-04-22T10:00:00.000000Z")];
    thread_memory::merge(&llm, default_req(&data_dir(&tmp), &messages)).await;

    let p = llm.last_prompt();
    // The conditional data section is omitted when persona memory is absent.
    // (The MERGING-list rule that names it is unconditional in the .md, so
    // we don't assert on that — only the data section is conditional.)
    assert!(!p.contains("--- WHAT YOU ALREADY KNOW ABOUT THIS PERSON ---"));
}

#[tokio::test]
async fn merge_roleplay_uses_roleplay_prompt_and_ignores_persona_memory() {
    let tmp = tempfile::tempdir().unwrap();
    let llm = FakeLlm::new("The duel ended in a draw.");
    let messages = vec![msg(
        Role::Assistant,
        "Steel rang in the courtyard",
        "2026-04-22T10:00:00.000000Z",
    )];

    // Roleplay mode: even if persona_memory is passed, the section must not appear.
    thread_memory::merge(
        &llm,
        MergeRequest {
            persona_display_name: "Sir Evrard",
            existing_memory: "The scene opened at dawn.",
            mode: Mode::Roleplay,
            persona_memory: "REAL-USER FACT: user lives in Seattle.",
            ..default_req(&data_dir(&tmp), &messages)
        },
    )
    .await;

    let p = llm.last_prompt();
    assert!(p.contains("ROLEPLAY"));
    assert!(p.contains("CURRENT SCENE SUMMARY"));
    assert!(!p.contains("WHAT YOU ALREADY KNOW"));
    assert!(!p.contains("REAL-USER FACT"));
}

#[tokio::test]
async fn merge_short_response_rejected_when_existing_is_substantial() {
    let tmp = tempfile::tempdir().unwrap();
    let llm = FakeLlm::new("ok"); // 2 chars.
    let messages = vec![msg(Role::User, "hi", "2026-04-22T10:00:00.000000Z")];
    let existing = "x".repeat(200);
    let got = thread_memory::merge(
        &llm,
        MergeRequest {
            existing_memory: &existing,
            ..default_req(&data_dir(&tmp), &messages)
        },
    )
    .await;
    assert!(got.is_none());
}

#[tokio::test]
async fn merge_short_response_accepted_when_no_existing_summary() {
    let tmp = tempfile::tempdir().unwrap();
    let llm = FakeLlm::new("tiny"); // 4 chars but no existing memory.
    let messages = vec![msg(Role::User, "hi", "2026-04-22T10:00:00.000000Z")];
    let got =
        thread_memory::merge(&llm, default_req(&data_dir(&tmp), &messages)).await;
    assert_eq!(got.as_deref(), Some("tiny"));
}

#[tokio::test]
async fn merge_propagates_llm_error_as_none() {
    let tmp = tempfile::tempdir().unwrap();
    let messages = vec![msg(Role::User, "hi", "2026-04-22T10:00:00.000000Z")];
    let got = thread_memory::merge(
        &FailingLlm,
        MergeRequest {
            existing_memory: "prior",
            ..default_req(&data_dir(&tmp), &messages)
        },
    )
    .await;
    assert!(got.is_none());
}

#[tokio::test]
async fn merge_empty_existing_uses_start_of_thread_placeholder() {
    let tmp = tempfile::tempdir().unwrap();
    let llm = FakeLlm::new("first entry");
    let messages = vec![msg(Role::User, "hi", "2026-04-22T10:00:00.000000Z")];
    thread_memory::merge(&llm, default_req(&data_dir(&tmp), &messages)).await;

    let p = llm.last_prompt();
    assert!(p.contains("No summary yet. This is the start of the thread."));
}

#[tokio::test]
async fn merge_size_limit_zero_omits_size_target() {
    let tmp = tempfile::tempdir().unwrap();
    let llm = FakeLlm::new("body");
    let messages = vec![msg(Role::User, "hi", "2026-04-22T10:00:00.000000Z")];
    thread_memory::merge(
        &llm,
        MergeRequest {
            size_limit: 0,
            ..default_req(&data_dir(&tmp), &messages)
        },
    )
    .await;
    assert!(!llm.last_prompt().contains("SIZE TARGET"));

    let llm = FakeLlm::new("body");
    thread_memory::merge(
        &llm,
        MergeRequest {
            size_limit: 2500,
            ..default_req(&data_dir(&tmp), &messages)
        },
    )
    .await;
    assert!(llm.last_prompt().contains("SIZE TARGET: Aim for roughly 2500"));
}
