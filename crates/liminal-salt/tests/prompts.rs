//! Integration tests for `services::prompts`.
//!
//! Bundled defaults are embedded at compile time via `crate::assets::DefaultPrompts`,
//! so these tests assert against the real shipped content rather than fixtures.
//! User state lives in tempdirs.

use liminal_salt::services::prompts::{self, PROMPTS, PromptError};

#[test]
fn registry_size_locked() {
    // M3 v1 ships 5 user-editable prompts; lock that in so a registry edit
    // is a deliberate decision, not a drift.
    assert_eq!(PROMPTS.len(), 5, "prompt registry size changed");

    // No duplicate IDs.
    let mut ids: Vec<&str> = PROMPTS.iter().map(|p| p.id).collect();
    ids.sort();
    let len_before = ids.len();
    ids.dedup();
    assert_eq!(ids.len(), len_before, "duplicate prompt id in registry");
}

#[test]
fn list_returns_full_registry() {
    let listed = prompts::list();
    assert_eq!(listed.len(), PROMPTS.len());
}

#[test]
fn every_registered_prompt_has_a_bundled_default() {
    // Catches registry/disk drift at test time: a registered ID with no
    // shipped `.md` would be a runtime warning + broken Reset.
    for meta in PROMPTS {
        let content = prompts::load_default(meta.id)
            .unwrap_or_else(|err| panic!("bundled default missing for {}: {err:?}", meta.id));
        assert!(!content.is_empty(), "bundled default empty for {}", meta.id);
    }
}

#[tokio::test]
async fn seed_writes_every_registered_prompt_into_data_dir() {
    let tmp = tempfile::tempdir().unwrap();
    let data_dir = tmp.path().join("data");

    prompts::seed_default_prompts(&data_dir).await;

    for meta in PROMPTS {
        let target = data_dir.join("prompts").join(format!("{}.md", meta.id));
        let body = tokio::fs::read_to_string(&target).await.unwrap_or_else(|err| {
            panic!("seeded file missing for {}: {err}", meta.id);
        });
        let bundled = prompts::load_default(meta.id).unwrap();
        assert_eq!(body, bundled, "seeded content differs from bundled default for {}", meta.id);
    }
}

#[tokio::test]
async fn seed_does_not_overwrite_user_edits() {
    let tmp = tempfile::tempdir().unwrap();
    let data_dir = tmp.path().join("data");
    let id = PROMPTS[0].id;

    let user_path = data_dir.join("prompts").join(format!("{id}.md"));
    tokio::fs::create_dir_all(user_path.parent().unwrap()).await.unwrap();
    tokio::fs::write(&user_path, "user's edit").await.unwrap();

    prompts::seed_default_prompts(&data_dir).await;

    let body = tokio::fs::read_to_string(&user_path).await.unwrap();
    assert_eq!(body, "user's edit", "seed must not overwrite user edits");
}

#[tokio::test]
async fn save_then_load_round_trips() {
    let tmp = tempfile::tempdir().unwrap();
    let data_dir = tmp.path().join("data");
    let id = PROMPTS[0].id;

    prompts::save(&data_dir, id, "edited content").await.unwrap();
    let loaded = prompts::load(&data_dir, id).await.unwrap();
    assert_eq!(loaded, "edited content");
}

#[tokio::test]
async fn load_falls_back_to_bundled_when_user_file_missing() {
    let tmp = tempfile::tempdir().unwrap();
    let data_dir = tmp.path().join("data");
    let id = PROMPTS[0].id;

    let loaded = prompts::load(&data_dir, id).await.unwrap();
    let bundled = prompts::load_default(id).unwrap();
    assert_eq!(loaded, bundled, "load must fall back to bundled default if user file absent");
}

#[tokio::test]
async fn load_user_copy_wins_over_bundled() {
    let tmp = tempfile::tempdir().unwrap();
    let data_dir = tmp.path().join("data");
    let id = PROMPTS[0].id;

    prompts::save(&data_dir, id, "user override").await.unwrap();

    let loaded = prompts::load(&data_dir, id).await.unwrap();
    assert_eq!(loaded, "user override");
}

#[tokio::test]
async fn reset_overwrites_user_with_bundled() {
    let tmp = tempfile::tempdir().unwrap();
    let data_dir = tmp.path().join("data");
    let id = PROMPTS[0].id;

    prompts::save(&data_dir, id, "user override").await.unwrap();
    prompts::reset(&data_dir, id).await.unwrap();

    let loaded = prompts::load(&data_dir, id).await.unwrap();
    let bundled = prompts::load_default(id).unwrap();
    assert_eq!(loaded, bundled);
}

#[tokio::test]
async fn invalid_id_rejected_by_save() {
    let tmp = tempfile::tempdir().unwrap();
    let err = prompts::save(tmp.path(), "not_in_registry", "x").await.unwrap_err();
    assert!(matches!(err, PromptError::InvalidId(_)));
}

#[tokio::test]
async fn invalid_id_rejected_by_load() {
    let tmp = tempfile::tempdir().unwrap();
    let err = prompts::load(tmp.path(), "not_in_registry").await.unwrap_err();
    assert!(matches!(err, PromptError::InvalidId(_)));
}

#[tokio::test]
async fn invalid_id_rejected_by_reset() {
    let tmp = tempfile::tempdir().unwrap();
    let err = prompts::reset(tmp.path(), "not_in_registry").await.unwrap_err();
    assert!(matches!(err, PromptError::InvalidId(_)));
}

#[test]
fn invalid_id_rejected_by_load_default() {
    let err = prompts::load_default("not_in_registry").unwrap_err();
    assert!(matches!(err, PromptError::InvalidId(_)));
}

#[tokio::test]
async fn save_creates_prompts_dir_via_atomic_write() {
    let tmp = tempfile::tempdir().unwrap();
    let data_dir = tmp.path().join("nested").join("data");
    let id = PROMPTS[0].id;

    // The data dir doesn't exist yet; atomic write should create it.
    prompts::save(&data_dir, id, "content").await.unwrap();
    let body = tokio::fs::read_to_string(data_dir.join("prompts").join(format!("{id}.md")))
        .await
        .unwrap();
    assert_eq!(body, "content");
}
