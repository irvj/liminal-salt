//! ChatCore preservation tests. Uses a fake LLM so nothing hits the network.
//! The roadmap's Phase 3 unit spec:
//! > `ChatCore::send` preserves `scenario`, `thread_memory`, `pinned`, `draft`
//! > through the RMW.

use std::path::Path;

use liminal_salt::services::chat::{SendContext, send_message};
use liminal_salt::services::llm::{ChatLlm, LlmError, LlmMessage};
use liminal_salt::services::session::{self, Mode, Role};

struct FakeLlm {
    response: String,
}

impl ChatLlm for FakeLlm {
    async fn complete(&self, _messages: &[LlmMessage]) -> Result<String, LlmError> {
        Ok(self.response.clone())
    }
}

struct FailingLlm;

impl ChatLlm for FailingLlm {
    async fn complete(&self, _messages: &[LlmMessage]) -> Result<String, LlmError> {
        Err(LlmError::BadResponse("simulated".into()))
    }
}

fn ctx<'a>(sessions_dir: &'a Path, session_id: &'a str, system_prompt: &'a str) -> SendContext<'a> {
    SendContext {
        sessions_dir,
        session_id,
        system_prompt,
        user_timezone: "UTC",
        assistant_timezone: None,
        context_history_limit: 50,
    }
}

#[tokio::test]
async fn send_preserves_scenario_memory_pinned_draft() {
    let tmp = tempfile::tempdir().unwrap();
    let id = "session_20260421_130001.json";

    session::create_session(tmp.path(), id, "assistant", "New Chat", Mode::Roleplay, vec![])
        .await
        .unwrap();

    // Non-chat state that must survive the RMW.
    session::save_scenario(tmp.path(), id, "ominous clearing at dusk").await;
    session::save_thread_memory(
        tmp.path(),
        id,
        "they discovered a locked gate",
        "2026-04-21T12:00:00.000000Z",
    )
    .await;
    session::toggle_pin(tmp.path(), id).await;
    session::save_draft(tmp.path(), id, "partially typed").await;

    let llm = FakeLlm {
        response: "The gate creaks open.".to_string(),
    };
    let outcome = send_message(
        &ctx(tmp.path(), id, "you are a narrator"),
        &llm,
        "I try the handle.",
        false,
    )
    .await;

    assert!(!outcome.is_error);
    assert_eq!(outcome.response, "The gate creaks open.");

    let loaded = session::load_session(tmp.path(), id).await.unwrap();
    assert_eq!(loaded.scenario.as_deref(), Some("ominous clearing at dusk"));
    assert_eq!(loaded.thread_memory, "they discovered a locked gate");
    assert_eq!(loaded.thread_memory_updated_at, "2026-04-21T12:00:00.000000Z");
    assert_eq!(loaded.pinned, Some(true));
    assert_eq!(loaded.draft.as_deref(), Some("partially typed"));
    assert_eq!(loaded.mode, Mode::Roleplay);

    // Turn should have appended user + assistant message.
    assert_eq!(loaded.messages.len(), 2);
    assert_eq!(loaded.messages[0].role, Role::User);
    assert_eq!(loaded.messages[0].content, "I try the handle.");
    assert_eq!(loaded.messages[1].role, Role::Assistant);
    assert_eq!(loaded.messages[1].content, "The gate creaks open.");

    // send_message must not lock the title — summarizer will do that in a
    // separate step.
    assert!(loaded.title_locked.is_none());
}

#[tokio::test]
async fn skip_user_save_does_not_duplicate_message() {
    let tmp = tempfile::tempdir().unwrap();
    let id = "session_20260421_130002.json";

    // Initial message is already persisted — simulates start_chat flow.
    let existing = vec![session::Message {
        role: Role::User,
        content: "pre-saved".to_string(),
        timestamp: session::now_timestamp(),
    }];
    session::create_session(tmp.path(), id, "assistant", "New Chat", Mode::Chatbot, existing)
        .await
        .unwrap();

    let llm = FakeLlm {
        response: "ack".to_string(),
    };
    let outcome = send_message(
        &ctx(tmp.path(), id, "be brief"),
        &llm,
        "pre-saved",
        true,
    )
    .await;
    assert!(!outcome.is_error);

    let loaded = session::load_session(tmp.path(), id).await.unwrap();
    assert_eq!(loaded.messages.len(), 2); // user + assistant, not duplicated
    assert_eq!(loaded.messages[0].content, "pre-saved");
    assert_eq!(loaded.messages[1].role, Role::Assistant);
}

#[tokio::test]
async fn llm_failure_surfaces_as_error_string() {
    let tmp = tempfile::tempdir().unwrap();
    let id = "session_20260421_130003.json";
    session::create_session(tmp.path(), id, "assistant", "t", Mode::Chatbot, vec![])
        .await
        .unwrap();

    let outcome = send_message(
        &ctx(tmp.path(), id, "sys"),
        &FailingLlm,
        "hello",
        false,
    )
    .await;
    assert!(outcome.is_error);
    assert!(outcome.response.starts_with("ERROR:"));

    // On error we do NOT save the assistant message. The user message we
    // appended is also not persisted (the save path is after the LLM call).
    let loaded = session::load_session(tmp.path(), id).await.unwrap();
    assert!(
        loaded.messages.is_empty(),
        "failed turn must not leave partial state on disk",
    );
}

#[tokio::test]
async fn missing_session_returns_error_without_panic() {
    let tmp = tempfile::tempdir().unwrap();
    let id = "session_20260421_130004.json";
    // Don't create the session — load_session will return None.

    let outcome = send_message(
        &ctx(tmp.path(), id, "sys"),
        &FakeLlm {
            response: "unreachable".into(),
        },
        "hi",
        false,
    )
    .await;
    assert!(outcome.is_error);
    assert!(outcome.response.contains("session not found"));
}
