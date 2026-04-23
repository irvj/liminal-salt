//! Integration tests for the full system-prompt assembly. Verifies CLAUDE.md
//! context order, and that roleplay threads suppress persona memory.

use liminal_salt::services::{
    context_files::ContextScope,
    persona,
    prompt::build_system_prompt,
    session::{Message, Mode, Role, Session, now_timestamp},
};

fn blank_session(persona_name: &str, mode: Mode) -> Session {
    Session {
        title: "t".into(),
        title_locked: None,
        persona: persona_name.into(),
        mode,
        messages: Vec::new(),
        draft: None,
        pinned: None,
        scenario: None,
        thread_memory: String::new(),
        thread_memory_updated_at: String::new(),
        thread_memory_settings: None,
    }
}

async fn seed_persona(data_dir: &std::path::Path, name: &str, identity: &str) {
    persona::create_persona(data_dir, name, identity).await.unwrap();
}

fn index_of(haystack: &str, needle: &str) -> Option<usize> {
    haystack.find(needle)
}

#[tokio::test]
async fn chatbot_assembly_has_identity_contexts_memory_in_order() {
    let tmp = tempfile::tempdir().unwrap();
    seed_persona(tmp.path(), "coach", "# Coach identity\nBe supportive.").await;

    // Persona-scoped context file.
    let persona_scope = ContextScope::persona(tmp.path(), "coach");
    persona_scope
        .upload_file("style.md", b"speaks in short sentences")
        .await
        .unwrap();

    // Global context file.
    let global_scope = ContextScope::global(tmp.path());
    global_scope
        .upload_file("prefs.md", b"prefers plain language")
        .await
        .unwrap();

    // Persona memory.
    let memory_path = tmp.path().join("memory").join("coach.md");
    tokio::fs::create_dir_all(memory_path.parent().unwrap()).await.unwrap();
    tokio::fs::write(&memory_path, "They go by Sam and run a bakery.")
        .await
        .unwrap();

    let mut session = blank_session("coach", Mode::Chatbot);
    session.thread_memory = "Earlier, Sam asked about staffing.".to_string();

    let prompt = build_system_prompt(tmp.path(), &session).await;

    // All six sections present (no scenario in chatbot mode).
    let identity_pos = index_of(&prompt, "--- SYSTEM INSTRUCTION:").expect("identity");
    let persona_ctx_pos = index_of(&prompt, "--- PERSONA CONTEXT FILES ---").expect("persona ctx");
    let global_ctx_pos = index_of(&prompt, "--- USER CONTEXT FILES ---").expect("global ctx");
    let thread_pos = index_of(&prompt, "--- THREAD SUMMARY ---").expect("thread summary");
    let memory_pos = index_of(&prompt, "--- YOUR MEMORY ABOUT THIS USER ---").expect("memory");

    // CLAUDE.md order.
    assert!(identity_pos < persona_ctx_pos);
    assert!(persona_ctx_pos < global_ctx_pos);
    assert!(global_ctx_pos < thread_pos);
    assert!(thread_pos < memory_pos);

    assert!(prompt.contains("speaks in short sentences"));
    assert!(prompt.contains("prefers plain language"));
    assert!(prompt.contains("Sam asked about staffing"));
    assert!(prompt.contains("They go by Sam and run a bakery."));
}

#[tokio::test]
async fn roleplay_assembly_includes_scenario_and_suppresses_persona_memory() {
    let tmp = tempfile::tempdir().unwrap();
    seed_persona(tmp.path(), "bard", "Speaks in prose.").await;

    let memory_path = tmp.path().join("memory").join("bard.md");
    tokio::fs::create_dir_all(memory_path.parent().unwrap()).await.unwrap();
    tokio::fs::write(&memory_path, "REAL-USER SECRET BIOGRAPHY")
        .await
        .unwrap();

    let mut session = blank_session("bard", Mode::Roleplay);
    session.scenario = Some("A dim tavern at the edge of a forgotten kingdom.".into());
    session.thread_memory = "The stranger offered a riddle.".into();
    // Add a message to make the session realistic.
    session.messages.push(Message {
        role: Role::User,
        content: "Who are you?".into(),
        timestamp: now_timestamp(),
    });

    let prompt = build_system_prompt(tmp.path(), &session).await;

    let identity_pos = index_of(&prompt, "--- SYSTEM INSTRUCTION:").unwrap();
    let scenario_pos = index_of(&prompt, "--- SCENARIO ---").expect("scenario emitted");
    let thread_pos = index_of(&prompt, "--- THREAD SUMMARY ---").unwrap();

    assert!(identity_pos < scenario_pos);
    assert!(scenario_pos < thread_pos);

    // Persona memory MUST be suppressed for roleplay.
    assert!(
        !prompt.contains("--- YOUR MEMORY ABOUT THIS USER ---"),
        "persona memory header leaked into roleplay prompt",
    );
    assert!(
        !prompt.contains("REAL-USER SECRET BIOGRAPHY"),
        "persona memory body leaked into roleplay prompt",
    );

    assert!(prompt.contains("dim tavern"));
    assert!(prompt.contains("stranger offered a riddle"));
}

#[tokio::test]
async fn chatbot_without_memory_file_omits_section() {
    let tmp = tempfile::tempdir().unwrap();
    seed_persona(tmp.path(), "quiet", "You are quiet.").await;

    let session = blank_session("quiet", Mode::Chatbot);
    let prompt = build_system_prompt(tmp.path(), &session).await;

    assert!(prompt.contains("--- SYSTEM INSTRUCTION:"));
    assert!(!prompt.contains("--- YOUR MEMORY ABOUT THIS USER ---"));
    assert!(!prompt.contains("--- THREAD SUMMARY ---"));
    assert!(!prompt.contains("--- SCENARIO ---"));
}

#[tokio::test]
async fn missing_persona_emits_warning_sentinel() {
    let tmp = tempfile::tempdir().unwrap();
    let session = blank_session("ghost", Mode::Chatbot);
    let prompt = build_system_prompt(tmp.path(), &session).await;
    assert!(prompt.contains("--- WARNING: Persona not found ---"));
}

#[tokio::test]
async fn empty_scenario_is_not_emitted_in_roleplay() {
    let tmp = tempfile::tempdir().unwrap();
    seed_persona(tmp.path(), "bard", "Speaks in prose.").await;
    let mut session = blank_session("bard", Mode::Roleplay);
    session.scenario = Some(String::new());
    let prompt = build_system_prompt(tmp.path(), &session).await;
    assert!(!prompt.contains("--- SCENARIO ---"));
}
