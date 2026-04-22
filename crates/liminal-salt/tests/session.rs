//! Integration tests for the session service. Each test writes to its own
//! tempdir; session IDs differ per test so the global per-session lock map
//! doesn't cause incidental serialization between parallel tests.

use liminal_salt::services::session::{
    self, Message, Mode, Role, SessionError, ThreadMemorySettings, valid_session_id,
};

fn msg(role: Role, content: &str) -> Message {
    Message {
        role,
        content: content.to_string(),
        timestamp: session::now_timestamp(),
    }
}

async fn read_raw_json(path: &std::path::Path) -> serde_json::Value {
    let bytes = tokio::fs::read(path).await.expect("read session file");
    serde_json::from_slice(&bytes).expect("valid json")
}

#[tokio::test]
async fn valid_session_id_accepts_canonical_forms() {
    assert!(valid_session_id("session_20260421_120000.json"));
    assert!(valid_session_id("session_20260421_120000_1.json"));
    assert!(valid_session_id("session_20260421_120000_42.json"));
}

#[tokio::test]
async fn valid_session_id_rejects_traversal_and_garbage() {
    assert!(!valid_session_id("session_20260421_120000.txt"));
    assert!(!valid_session_id("../session_20260421_120000.json"));
    assert!(!valid_session_id("session_20260421_120000.json/../etc"));
    assert!(!valid_session_id("session_2026_120000.json"));
    assert!(!valid_session_id(""));
    assert!(!valid_session_id("session_abc.json"));
}

#[tokio::test]
async fn create_then_load_round_trips_core_fields() {
    let tmp = tempfile::tempdir().unwrap();
    let id = "session_20260421_120001.json";
    let written = session::create_session(
        tmp.path(),
        id,
        "riddler",
        "New Chat",
        Mode::Chatbot,
        vec![],
    )
    .await
    .expect("create");

    let loaded = session::load_session(tmp.path(), id)
        .await
        .expect("load");
    assert_eq!(loaded.title, written.title);
    assert_eq!(loaded.persona, "riddler");
    assert_eq!(loaded.mode, Mode::Chatbot);
    assert!(loaded.messages.is_empty());
    // Optional fields stay absent on create.
    assert!(loaded.title_locked.is_none());
    assert!(loaded.pinned.is_none());
    assert!(loaded.draft.is_none());
}

#[tokio::test]
async fn create_omits_optional_fields_from_disk() {
    let tmp = tempfile::tempdir().unwrap();
    let id = "session_20260421_120002.json";
    session::create_session(tmp.path(), id, "assistant", "New Chat", Mode::Chatbot, vec![])
        .await
        .unwrap();

    let json = read_raw_json(&tmp.path().join(id)).await;
    let obj = json.as_object().expect("object");
    assert!(!obj.contains_key("title_locked"));
    assert!(!obj.contains_key("draft"));
    assert!(!obj.contains_key("pinned"));
    assert!(!obj.contains_key("scenario"));
    assert!(!obj.contains_key("thread_memory_settings"));
    assert_eq!(obj.get("mode").and_then(|v| v.as_str()), Some("chatbot"));
}

#[tokio::test]
async fn save_chat_history_preserves_non_chat_fields() {
    let tmp = tempfile::tempdir().unwrap();
    let id = "session_20260421_120003.json";
    session::create_session(tmp.path(), id, "assistant", "New Chat", Mode::Roleplay, vec![])
        .await
        .unwrap();

    // Side-channel state that save_chat_history must preserve.
    session::save_scenario(tmp.path(), id, "dim tavern")
        .await
        .unwrap();
    session::save_thread_memory(
        tmp.path(),
        id,
        "they spoke of stars",
        "2026-04-21T12:00:01.000000Z",
    )
    .await
    .unwrap();
    assert!(session::toggle_pin(tmp.path(), id).await.unwrap());
    session::save_draft(tmp.path(), id, "half-written")
        .await
        .unwrap();

    let messages = vec![msg(Role::User, "hello"), msg(Role::Assistant, "hi")];
    session::save_chat_history(
        tmp.path(),
        id,
        "Tavern tales",
        "assistant",
        messages.clone(),
        Some(true),
    )
    .await
    .unwrap();

    let loaded = session::load_session(tmp.path(), id).await.unwrap();
    assert_eq!(loaded.title, "Tavern tales");
    assert_eq!(loaded.title_locked, Some(true));
    assert_eq!(loaded.messages.len(), 2);
    assert_eq!(loaded.mode, Mode::Roleplay, "mode must survive RMW");
    assert_eq!(loaded.scenario.as_deref(), Some("dim tavern"));
    assert_eq!(loaded.thread_memory, "they spoke of stars");
    assert_eq!(loaded.thread_memory_updated_at, "2026-04-21T12:00:01.000000Z");
    assert_eq!(loaded.pinned, Some(true));
    assert_eq!(loaded.draft.as_deref(), Some("half-written"));
}

#[tokio::test]
async fn invalid_session_id_short_circuits_every_writer() {
    let tmp = tempfile::tempdir().unwrap();
    let bogus = "../etc/passwd";

    // Every public entry point must reject the invalid id with InvalidId.
    fn assert_invalid<T: std::fmt::Debug>(result: Result<T, SessionError>) {
        match result {
            Err(SessionError::InvalidId(_)) => {}
            other => panic!("expected InvalidId, got {other:?}"),
        }
    }

    assert_invalid(
        session::create_session(tmp.path(), bogus, "x", "x", Mode::Chatbot, vec![]).await,
    );
    assert_invalid(session::delete_session(tmp.path(), bogus).await);
    assert_invalid(session::save_chat_history(tmp.path(), bogus, "t", "p", vec![], None).await);
    assert_invalid(session::toggle_pin(tmp.path(), bogus).await);
    assert_invalid(session::rename_session(tmp.path(), bogus, "new").await);
    assert_invalid(session::save_draft(tmp.path(), bogus, "x").await);
    assert_invalid(session::save_scenario(tmp.path(), bogus, "x").await);
    assert_invalid(
        session::save_thread_memory(tmp.path(), bogus, "x", "2026-04-21T00:00:00.000000Z").await,
    );
    assert_invalid(session::fork_to_roleplay(tmp.path(), bogus).await);
    assert_invalid(session::load_session(tmp.path(), bogus).await);
    assert_invalid(session::remove_last_assistant_message(tmp.path(), bogus).await);
    assert_invalid(session::update_last_user_message(tmp.path(), bogus, "x").await);
}

#[tokio::test]
async fn rename_session_locks_title() {
    let tmp = tempfile::tempdir().unwrap();
    let id = "session_20260421_120004.json";
    session::create_session(tmp.path(), id, "assistant", "New Chat", Mode::Chatbot, vec![])
        .await
        .unwrap();

    session::rename_session(tmp.path(), id, "My Thread")
        .await
        .unwrap();
    let loaded = session::load_session(tmp.path(), id).await.unwrap();
    assert_eq!(loaded.title, "My Thread");
    assert_eq!(loaded.title_locked, Some(true));
}

#[tokio::test]
async fn toggle_pin_flips_and_returns_new_state() {
    let tmp = tempfile::tempdir().unwrap();
    let id = "session_20260421_120005.json";
    session::create_session(tmp.path(), id, "assistant", "New Chat", Mode::Chatbot, vec![])
        .await
        .unwrap();

    assert!(session::toggle_pin(tmp.path(), id).await.unwrap());
    assert!(!session::toggle_pin(tmp.path(), id).await.unwrap());
}

#[tokio::test]
async fn delete_session_removes_file_and_drops_lock() {
    let tmp = tempfile::tempdir().unwrap();
    let id = "session_20260421_120006.json";
    session::create_session(tmp.path(), id, "assistant", "New Chat", Mode::Chatbot, vec![])
        .await
        .unwrap();

    session::delete_session(tmp.path(), id).await.unwrap();
    assert!(matches!(
        session::load_session(tmp.path(), id).await,
        Err(SessionError::NotFound(_))
    ));
    // Second delete: file already gone, returns NotFound without panicking.
    assert!(matches!(
        session::delete_session(tmp.path(), id).await,
        Err(SessionError::NotFound(_))
    ));
}

#[tokio::test]
async fn remove_last_assistant_returns_user_content() {
    let tmp = tempfile::tempdir().unwrap();
    let id = "session_20260421_120007.json";
    session::create_session(
        tmp.path(),
        id,
        "assistant",
        "t",
        Mode::Chatbot,
        vec![msg(Role::User, "question"), msg(Role::Assistant, "answer")],
    )
    .await
    .unwrap();

    let (last_user, new_state) = session::remove_last_assistant_message(tmp.path(), id)
        .await
        .expect("removal");
    assert_eq!(last_user, "question");
    assert_eq!(new_state.messages.len(), 1);
    assert_eq!(new_state.messages[0].role, Role::User);
}

#[tokio::test]
async fn update_last_user_rewrites_final_user_message() {
    let tmp = tempfile::tempdir().unwrap();
    let id = "session_20260421_120008.json";
    session::create_session(
        tmp.path(),
        id,
        "assistant",
        "t",
        Mode::Chatbot,
        vec![msg(Role::User, "draft v1")],
    )
    .await
    .unwrap();

    session::update_last_user_message(tmp.path(), id, "draft v2")
        .await
        .unwrap();
    let loaded = session::load_session(tmp.path(), id).await.unwrap();
    assert_eq!(loaded.messages[0].content, "draft v2");
}

#[tokio::test]
async fn thread_memory_settings_override_merges_and_clears() {
    let tmp = tempfile::tempdir().unwrap();
    let id = "session_20260421_120009.json";
    session::create_session(tmp.path(), id, "assistant", "t", Mode::Chatbot, vec![])
        .await
        .unwrap();

    // Partial write: only interval_minutes is set.
    let patch1 = ThreadMemorySettings {
        interval_minutes: Some(15),
        message_floor: None,
        size_limit: None,
    };
    session::save_thread_memory_settings_override(tmp.path(), id, patch1)
        .await
        .unwrap();

    // Second partial: only message_floor. interval_minutes should persist.
    let patch2 = ThreadMemorySettings {
        interval_minutes: None,
        message_floor: Some(3),
        size_limit: None,
    };
    session::save_thread_memory_settings_override(tmp.path(), id, patch2)
        .await
        .unwrap();

    let loaded = session::load_session(tmp.path(), id).await.unwrap();
    let settings = loaded.thread_memory_settings.expect("merged settings");
    assert_eq!(settings.interval_minutes, Some(15));
    assert_eq!(settings.message_floor, Some(3));
    assert_eq!(settings.size_limit, None);

    // Clear: override disappears entirely.
    session::clear_thread_memory_settings_override(tmp.path(), id)
        .await
        .unwrap();
    let loaded = session::load_session(tmp.path(), id).await.unwrap();
    assert!(loaded.thread_memory_settings.is_none());
}

#[tokio::test]
async fn fork_to_roleplay_copies_thread_and_resets_mode() {
    let tmp = tempfile::tempdir().unwrap();
    let source = "session_20260421_120010.json";
    let messages = vec![msg(Role::User, "hello"), msg(Role::Assistant, "hi")];
    session::create_session(
        tmp.path(),
        source,
        "assistant",
        "Origin",
        Mode::Chatbot,
        messages.clone(),
    )
    .await
    .unwrap();
    session::save_thread_memory(
        tmp.path(),
        source,
        "brief exchange of greetings",
        "2026-04-21T12:00:10.000000Z",
    )
    .await
    .unwrap();
    session::toggle_pin(tmp.path(), source).await.unwrap();

    let new_id = session::fork_to_roleplay(tmp.path(), source)
        .await
        .expect("fork");

    // New session: roleplay mode, same messages and thread memory, reset
    // pinned/draft/scenario/title.
    let forked = session::load_session(tmp.path(), &new_id).await.unwrap();
    assert_eq!(forked.mode, Mode::Roleplay);
    assert_eq!(forked.title, "New Chat");
    assert_eq!(forked.persona, "assistant");
    assert_eq!(forked.messages.len(), 2);
    assert_eq!(forked.thread_memory, "brief exchange of greetings");
    assert_eq!(
        forked.thread_memory_updated_at,
        "2026-04-21T12:00:10.000000Z"
    );
    assert!(forked.pinned.is_none());
    assert!(forked.draft.is_none());
    assert!(forked.scenario.is_none());

    // Origin untouched.
    let origin = session::load_session(tmp.path(), source).await.unwrap();
    assert_eq!(origin.mode, Mode::Chatbot);
    assert_eq!(origin.title, "Origin");
    assert_eq!(origin.pinned, Some(true));
}

#[tokio::test]
async fn update_persona_across_sessions_rewrites_matching_sessions() {
    let tmp = tempfile::tempdir().unwrap();
    let a = "session_20260421_120011.json";
    let b = "session_20260421_120012.json";
    let c = "session_20260421_120013.json";
    session::create_session(tmp.path(), a, "sage", "t", Mode::Chatbot, vec![])
        .await
        .unwrap();
    session::create_session(tmp.path(), b, "sage", "t", Mode::Chatbot, vec![])
        .await
        .unwrap();
    session::create_session(tmp.path(), c, "other", "t", Mode::Chatbot, vec![])
        .await
        .unwrap();

    session::update_persona_across_sessions(tmp.path(), "sage", "oracle").await;

    assert_eq!(
        session::load_session(tmp.path(), a).await.unwrap().persona,
        "oracle"
    );
    assert_eq!(
        session::load_session(tmp.path(), b).await.unwrap().persona,
        "oracle"
    );
    assert_eq!(
        session::load_session(tmp.path(), c).await.unwrap().persona,
        "other"
    );
}

#[tokio::test]
async fn concurrent_save_chat_history_produces_valid_json() {
    use tokio::task::JoinSet;

    let tmp = tempfile::tempdir().unwrap();
    let id = "session_20260421_120014.json";
    session::create_session(tmp.path(), id, "assistant", "t", Mode::Chatbot, vec![])
        .await
        .unwrap();

    let mut set = JoinSet::new();
    for i in 0..10u32 {
        let path = tmp.path().to_path_buf();
        let id = id.to_string();
        set.spawn(async move {
            let messages = (0..5)
                .map(|j| Message {
                    role: if j % 2 == 0 { Role::User } else { Role::Assistant },
                    content: format!("msg {i}-{j}"),
                    timestamp: session::now_timestamp(),
                })
                .collect();
            session::save_chat_history(&path, &id, &format!("title {i}"), "assistant", messages, None)
                .await
        });
    }
    while let Some(res) = set.join_next().await {
        res.expect("task panicked").expect("save ok");
    }

    // The final file must be valid JSON with exactly 5 messages (the
    // last writer wins; no torn / mixed state).
    let loaded = session::load_session(tmp.path(), id).await.unwrap();
    assert_eq!(loaded.messages.len(), 5);
    assert!(loaded.title.starts_with("title "));
}

#[tokio::test]
async fn list_sessions_sorts_newest_first_and_skips_non_json() {
    let tmp = tempfile::tempdir().unwrap();
    session::create_session(
        tmp.path(),
        "session_20260421_120015.json",
        "a",
        "t",
        Mode::Chatbot,
        vec![],
    )
    .await
    .unwrap();
    session::create_session(
        tmp.path(),
        "session_20260421_120016.json",
        "b",
        "t",
        Mode::Chatbot,
        vec![],
    )
    .await
    .unwrap();
    // Non-json entry must be ignored.
    tokio::fs::write(tmp.path().join("README.txt"), "ignore me")
        .await
        .unwrap();

    let list = session::list_sessions(tmp.path()).await;
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].id, "session_20260421_120016.json");
    assert_eq!(list[1].id, "session_20260421_120015.json");
}

// =============================================================================
// list_persona_threads
// =============================================================================

async fn seed_session(
    dir: &std::path::Path,
    id: &str,
    persona: &str,
    mode: Mode,
    messages: Vec<Message>,
) {
    session::create_session(dir, id, persona, "t", mode, messages)
        .await
        .expect("create");
}

#[tokio::test]
async fn list_persona_threads_filters_persona_and_skips_roleplay() {
    let tmp = tempfile::tempdir().unwrap();
    seed_session(
        tmp.path(),
        "session_20260421_130001.json",
        "riddler",
        Mode::Chatbot,
        vec![msg(Role::User, "match")],
    )
    .await;
    seed_session(
        tmp.path(),
        "session_20260421_130002.json",
        "other",
        Mode::Chatbot,
        vec![msg(Role::User, "skip - wrong persona")],
    )
    .await;
    seed_session(
        tmp.path(),
        "session_20260421_130003.json",
        "riddler",
        Mode::Roleplay,
        vec![msg(Role::User, "skip - roleplay")],
    )
    .await;
    // Empty thread — should be skipped even though persona matches.
    seed_session(
        tmp.path(),
        "session_20260421_130004.json",
        "riddler",
        Mode::Chatbot,
        vec![],
    )
    .await;

    let threads = session::list_persona_threads(tmp.path(), "riddler", None, None).await;
    assert_eq!(threads.len(), 1);
    assert_eq!(threads[0].persona, "riddler");
    assert_eq!(threads[0].messages[0].content, "match");
}

#[tokio::test]
async fn list_persona_threads_respects_per_thread_message_cap() {
    let tmp = tempfile::tempdir().unwrap();
    let msgs = vec![
        msg(Role::User, "m1"),
        msg(Role::Assistant, "m2"),
        msg(Role::User, "m3"),
        msg(Role::Assistant, "m4"),
        msg(Role::User, "m5"),
    ];
    seed_session(
        tmp.path(),
        "session_20260421_140001.json",
        "r",
        Mode::Chatbot,
        msgs,
    )
    .await;

    let threads = session::list_persona_threads(tmp.path(), "r", None, Some(2)).await;
    assert_eq!(threads.len(), 1);
    assert_eq!(threads[0].messages.len(), 2);
    // Cap keeps the most recent messages.
    assert_eq!(threads[0].messages[0].content, "m4");
    assert_eq!(threads[0].messages[1].content, "m5");
}

#[tokio::test]
async fn list_persona_threads_caps_total_threads_newest_first() {
    let tmp = tempfile::tempdir().unwrap();
    for i in 1..=3u32 {
        seed_session(
            tmp.path(),
            &format!("session_20260421_15000{i}.json"),
            "r",
            Mode::Chatbot,
            vec![msg(Role::User, &format!("thread {i}"))],
        )
        .await;
        // Space out mtimes so the sort is deterministic even on fast filesystems.
        tokio::time::sleep(std::time::Duration::from_millis(15)).await;
    }

    let threads = session::list_persona_threads(tmp.path(), "r", Some(2), None).await;
    assert_eq!(threads.len(), 2);
    // Newest-mtime first: thread 3, then thread 2.
    assert_eq!(threads[0].messages[0].content, "thread 3");
    assert_eq!(threads[1].messages[0].content, "thread 2");
}

#[tokio::test]
async fn list_persona_threads_missing_dir_returns_empty() {
    let tmp = tempfile::tempdir().unwrap();
    let nonexistent = tmp.path().join("never_created");
    let threads = session::list_persona_threads(&nonexistent, "r", None, None).await;
    assert!(threads.is_empty());
}
