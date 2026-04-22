//! Integration tests for the persona service: CRUD, identity I/O, config
//! roundtrip, and the rename cascade across sessions.

use liminal_salt::services::{
    persona::{
        self, PersonaConfig, ThreadMemoryDefaults, create_persona, delete_persona, list_personas,
        load_identity, load_persona_config, rename_persona, save_persona_config,
        valid_persona_name,
    },
    session::{self, Mode},
};

#[test]
fn name_validation() {
    assert!(valid_persona_name("assistant"));
    assert!(valid_persona_name("riddler_2"));
    assert!(!valid_persona_name(""));
    assert!(!valid_persona_name("with space"));
    assert!(!valid_persona_name("../escape"));
    assert!(!valid_persona_name("dash-ok")); // hyphens rejected — matches Python
}

#[tokio::test]
async fn create_list_delete_roundtrip() {
    let tmp = tempfile::tempdir().unwrap();
    create_persona(tmp.path(), "coach", "# Identity\n\nYou are a coach.")
        .await
        .expect("create");
    let personas = list_personas(tmp.path()).await;
    assert!(personas.contains(&"coach".to_string()));
    assert_eq!(
        load_identity(tmp.path(), "coach").await.trim(),
        "# Identity\n\nYou are a coach."
    );
    delete_persona(tmp.path(), "coach").await.expect("delete");
    assert!(!list_personas(tmp.path()).await.contains(&"coach".to_string()));
}

#[tokio::test]
async fn create_rejects_duplicate() {
    let tmp = tempfile::tempdir().unwrap();
    create_persona(tmp.path(), "dup", "body").await.unwrap();
    let err = create_persona(tmp.path(), "dup", "body").await;
    assert!(matches!(err, Err(persona::PersonaError::AlreadyExists)));
}

#[tokio::test]
async fn delete_cascades_memory_and_context() {
    let tmp = tempfile::tempdir().unwrap();
    create_persona(tmp.path(), "vera", "body").await.unwrap();

    // Seed sibling state the cascade must clean up.
    let memory = tmp.path().join("memory").join("vera.md");
    tokio::fs::create_dir_all(memory.parent().unwrap()).await.unwrap();
    tokio::fs::write(&memory, "some memory").await.unwrap();

    let persona_ctx = tmp
        .path()
        .join("user_context")
        .join("personas")
        .join("vera");
    tokio::fs::create_dir_all(&persona_ctx).await.unwrap();
    tokio::fs::write(persona_ctx.join("notes.md"), "note").await.unwrap();

    delete_persona(tmp.path(), "vera").await.unwrap();

    assert!(!tokio::fs::try_exists(&memory).await.unwrap());
    assert!(!tokio::fs::try_exists(&persona_ctx).await.unwrap());
}

#[tokio::test]
async fn rename_updates_directory_memory_context_and_sessions() {
    let tmp = tempfile::tempdir().unwrap();
    create_persona(tmp.path(), "old_name", "body").await.unwrap();

    let memory_old = tmp.path().join("memory").join("old_name.md");
    tokio::fs::create_dir_all(memory_old.parent().unwrap()).await.unwrap();
    tokio::fs::write(&memory_old, "mem").await.unwrap();

    let ctx_old = tmp.path().join("user_context").join("personas").join("old_name");
    tokio::fs::create_dir_all(&ctx_old).await.unwrap();
    tokio::fs::write(ctx_old.join("f.md"), "ctx").await.unwrap();

    let sessions = tmp.path().join("sessions");
    tokio::fs::create_dir_all(&sessions).await.unwrap();
    session::create_session(
        &sessions,
        "session_20260421_160000.json",
        "old_name",
        "t",
        Mode::Chatbot,
        vec![],
    )
    .await
    .unwrap();
    session::create_session(
        &sessions,
        "session_20260421_160001.json",
        "unrelated",
        "t",
        Mode::Chatbot,
        vec![],
    )
    .await
    .unwrap();

    rename_persona(tmp.path(), "old_name", "new_name").await.unwrap();

    // Directory moved.
    assert!(
        tokio::fs::try_exists(persona::persona_dir(tmp.path(), "new_name"))
            .await
            .unwrap()
    );
    assert!(
        !tokio::fs::try_exists(persona::persona_dir(tmp.path(), "old_name"))
            .await
            .unwrap()
    );
    // Memory file moved.
    let memory_new = tmp.path().join("memory").join("new_name.md");
    assert!(tokio::fs::try_exists(&memory_new).await.unwrap());
    assert!(!tokio::fs::try_exists(&memory_old).await.unwrap());
    // Persona context dir moved.
    let ctx_new = tmp.path().join("user_context").join("personas").join("new_name");
    assert!(tokio::fs::try_exists(&ctx_new).await.unwrap());
    // Sessions updated — matching persona renamed, unrelated untouched.
    let updated = session::load_session(&sessions, "session_20260421_160000.json")
        .await
        .unwrap();
    assert_eq!(updated.persona, "new_name");
    let unrelated = session::load_session(&sessions, "session_20260421_160001.json")
        .await
        .unwrap();
    assert_eq!(unrelated.persona, "unrelated");
}

#[tokio::test]
async fn config_roundtrip_omits_none_fields() {
    let tmp = tempfile::tempdir().unwrap();
    create_persona(tmp.path(), "alex", "body").await.unwrap();

    // Empty config → only baseline fields on disk (none serialized).
    let cfg = PersonaConfig {
        model: Some("anthropic/claude-sonnet-4-6".to_string()),
        default_mode: Some("roleplay".to_string()),
        default_thread_memory_settings: Some(ThreadMemoryDefaults {
            interval_minutes: Some(20),
            message_floor: None,
            size_limit: None,
        }),
        ..Default::default()
    };
    save_persona_config(tmp.path(), "alex", &cfg).await.unwrap();

    // Inspect raw JSON: verify absent fields are truly absent (no empty
    // objects, no nulls).
    let raw = tokio::fs::read(persona::config_file(tmp.path(), "alex"))
        .await
        .unwrap();
    let value: serde_json::Value = serde_json::from_slice(&raw).unwrap();
    let obj = value.as_object().unwrap();
    assert!(obj.contains_key("model"));
    assert!(obj.contains_key("default_mode"));
    assert!(obj.contains_key("default_thread_memory_settings"));
    assert!(!obj.contains_key("user_history_max_threads"));
    assert!(!obj.contains_key("auto_memory_interval"));

    let tmd = obj["default_thread_memory_settings"].as_object().unwrap();
    assert!(tmd.contains_key("interval_minutes"));
    assert!(!tmd.contains_key("message_floor"));
    assert!(!tmd.contains_key("size_limit"));

    // Roundtrip: load → identical shape.
    let loaded = load_persona_config(tmp.path(), "alex").await;
    assert_eq!(loaded.model.as_deref(), Some("anthropic/claude-sonnet-4-6"));
    assert_eq!(loaded.default_mode.as_deref(), Some("roleplay"));
    assert_eq!(
        loaded.default_thread_memory_settings.unwrap().interval_minutes,
        Some(20)
    );
}

#[tokio::test]
async fn config_preserves_unknown_keys() {
    let tmp = tempfile::tempdir().unwrap();
    create_persona(tmp.path(), "future", "body").await.unwrap();

    // Write a config with a forward-compatible key the current struct doesn't know.
    let path = persona::config_file(tmp.path(), "future");
    tokio::fs::create_dir_all(path.parent().unwrap()).await.unwrap();
    tokio::fs::write(
        &path,
        r#"{"model":"m","future_key":{"nested":true}}"#,
    )
    .await
    .unwrap();

    let loaded = load_persona_config(tmp.path(), "future").await;
    assert_eq!(loaded.model.as_deref(), Some("m"));
    assert!(loaded.extras.contains_key("future_key"));

    // Re-save should preserve the unknown key.
    save_persona_config(tmp.path(), "future", &loaded).await.unwrap();
    let raw = tokio::fs::read_to_string(&path).await.unwrap();
    assert!(raw.contains("future_key"));
    assert!(raw.contains("nested"));
}

#[tokio::test]
async fn rename_to_same_name_is_noop() {
    let tmp = tempfile::tempdir().unwrap();
    create_persona(tmp.path(), "steady", "body").await.unwrap();
    rename_persona(tmp.path(), "steady", "steady").await.unwrap();
    assert!(
        tokio::fs::try_exists(persona::persona_dir(tmp.path(), "steady"))
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn rename_rejects_target_collision() {
    let tmp = tempfile::tempdir().unwrap();
    create_persona(tmp.path(), "one", "body").await.unwrap();
    create_persona(tmp.path(), "two", "body").await.unwrap();
    let err = rename_persona(tmp.path(), "one", "two").await;
    assert!(matches!(err, Err(persona::PersonaError::AlreadyExists)));
}
