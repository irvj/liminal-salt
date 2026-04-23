//! End-to-end tests for `services::thread_memory::merge` against a fake LLM.
//! Mode dispatch, persona-memory injection, empty-input short-circuit, and the
//! short-output safety check all validated here; module-level unit tests in
//! `thread_memory.rs` cover prompt shape and settings resolver logic.

use std::sync::Mutex;

use liminal_salt::services::llm::{ChatLlm, LlmError, LlmMessage};
use liminal_salt::services::session::{Message, Mode, Role};
use liminal_salt::services::thread_memory;

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
    let llm = FakeLlm::new("should not be used");
    let got = thread_memory::merge(&llm, "Clara", "", &[], 4000, Mode::Chatbot, "").await;
    assert!(got.is_none());
    // LLM not invoked.
    assert!(llm.last_prompt().is_empty());
}

#[tokio::test]
async fn merge_chatbot_uses_chatbot_prompt_and_persona_memory_when_present() {
    let llm = FakeLlm::new("They settled on Tuesday.");
    let messages = vec![
        msg(Role::User, "let's meet Tuesday", "2026-04-22T10:00:00.000000Z"),
        msg(Role::Assistant, "works for me", "2026-04-22T10:00:01.000000Z"),
    ];
    let got = thread_memory::merge(
        &llm,
        "Clara",
        "They had been scheduling a meet.",
        &messages,
        4000,
        Mode::Chatbot,
        "They're in Pacific time and avoid mornings.",
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
    assert!(p.contains("PERSPECTIVE — apply to every sentence"));
}

#[tokio::test]
async fn merge_chatbot_omits_persona_memory_section_when_empty() {
    let llm = FakeLlm::new("fine");
    let messages = vec![msg(Role::User, "hi", "2026-04-22T10:00:00.000000Z")];
    thread_memory::merge(&llm, "Clara", "", &messages, 4000, Mode::Chatbot, "").await;

    let p = llm.last_prompt();
    assert!(!p.contains("WHAT YOU ALREADY KNOW"));
    assert!(!p.contains("DO NOT merge pre-existing knowledge"));
}

#[tokio::test]
async fn merge_roleplay_uses_roleplay_prompt_and_ignores_persona_memory() {
    let llm = FakeLlm::new("The duel ended in a draw.");
    let messages = vec![msg(
        Role::Assistant,
        "Steel rang in the courtyard",
        "2026-04-22T10:00:00.000000Z",
    )];

    // Roleplay mode: even if persona_memory is passed, the section must not appear.
    thread_memory::merge(
        &llm,
        "Sir Evrard",
        "The scene opened at dawn.",
        &messages,
        4000,
        Mode::Roleplay,
        "REAL-USER FACT: user lives in Seattle.",
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
    let llm = FakeLlm::new("ok"); // 2 chars.
    let messages = vec![msg(Role::User, "hi", "2026-04-22T10:00:00.000000Z")];
    let existing = "x".repeat(200);
    let got = thread_memory::merge(&llm, "Clara", &existing, &messages, 4000, Mode::Chatbot, "").await;
    assert!(got.is_none());
}

#[tokio::test]
async fn merge_short_response_accepted_when_no_existing_summary() {
    let llm = FakeLlm::new("tiny"); // 4 chars but no existing memory.
    let messages = vec![msg(Role::User, "hi", "2026-04-22T10:00:00.000000Z")];
    let got = thread_memory::merge(&llm, "Clara", "", &messages, 4000, Mode::Chatbot, "").await;
    assert_eq!(got.as_deref(), Some("tiny"));
}

#[tokio::test]
async fn merge_propagates_llm_error_as_none() {
    let messages = vec![msg(Role::User, "hi", "2026-04-22T10:00:00.000000Z")];
    let got = thread_memory::merge(
        &FailingLlm,
        "Clara",
        "prior",
        &messages,
        4000,
        Mode::Chatbot,
        "",
    )
    .await;
    assert!(got.is_none());
}

#[tokio::test]
async fn merge_empty_existing_uses_start_of_thread_placeholder() {
    let llm = FakeLlm::new("first entry");
    let messages = vec![msg(Role::User, "hi", "2026-04-22T10:00:00.000000Z")];
    thread_memory::merge(&llm, "Clara", "", &messages, 4000, Mode::Chatbot, "").await;

    let p = llm.last_prompt();
    assert!(p.contains("No summary yet. This is the start of the thread."));
}

#[tokio::test]
async fn merge_size_limit_zero_omits_size_target() {
    let llm = FakeLlm::new("body");
    let messages = vec![msg(Role::User, "hi", "2026-04-22T10:00:00.000000Z")];
    thread_memory::merge(&llm, "Clara", "", &messages, 0, Mode::Chatbot, "").await;
    assert!(!llm.last_prompt().contains("SIZE TARGET"));

    let llm = FakeLlm::new("body");
    thread_memory::merge(&llm, "Clara", "", &messages, 2500, Mode::Chatbot, "").await;
    assert!(llm.last_prompt().contains("SIZE TARGET: Aim for roughly 2500"));
}
