//! Integration tests for `services::prompts`. All filesystem state lives in
//! tempdirs; both the user `data/prompts/` and the bundled `default_prompts/`
//! are pointed at fixtures so the real crate's bundled defaults are not
//! required (Phase 1 ships them as empty placeholders anyway).

use std::path::Path;

use liminal_salt::services::prompts::{self, PROMPTS, PromptError};

/// Write a fake bundled default. Mirrors what `crates/liminal-salt/default_prompts/`
/// will contain in Phase 2.
async fn write_bundled(bundled_dir: &Path, id: &str, content: &str) {
    tokio::fs::create_dir_all(bundled_dir).await.unwrap();
    tokio::fs::write(bundled_dir.join(format!("{id}.md")), content)
        .await
        .unwrap();
}

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

#[tokio::test]
async fn seed_copies_bundled_files_when_missing() {
    let tmp = tempfile::tempdir().unwrap();
    let data_dir = tmp.path().join("data");
    let bundled_dir = tmp.path().join("bundled");

    // Provide bundled content for one registered prompt; leave the rest absent
    // (seed should warn-and-continue, not panic).
    let id = PROMPTS[0].id;
    write_bundled(&bundled_dir, id, "bundled body").await;

    prompts::seed_default_prompts(&data_dir, &bundled_dir).await;

    let target = data_dir.join("prompts").join(format!("{id}.md"));
    let body = tokio::fs::read_to_string(&target).await.unwrap();
    assert_eq!(body, "bundled body");
}

#[tokio::test]
async fn seed_does_not_overwrite_user_edits() {
    let tmp = tempfile::tempdir().unwrap();
    let data_dir = tmp.path().join("data");
    let bundled_dir = tmp.path().join("bundled");
    let id = PROMPTS[0].id;

    // User has an existing edited copy. Bundled default is different.
    let user_path = data_dir.join("prompts").join(format!("{id}.md"));
    tokio::fs::create_dir_all(user_path.parent().unwrap())
        .await
        .unwrap();
    tokio::fs::write(&user_path, "user's edit").await.unwrap();
    write_bundled(&bundled_dir, id, "bundled body").await;

    prompts::seed_default_prompts(&data_dir, &bundled_dir).await;

    let body = tokio::fs::read_to_string(&user_path).await.unwrap();
    assert_eq!(body, "user's edit", "seed must not overwrite user edits");
}

#[tokio::test]
async fn save_then_load_round_trips() {
    let tmp = tempfile::tempdir().unwrap();
    let data_dir = tmp.path().join("data");
    let bundled_dir = tmp.path().join("bundled");
    let id = PROMPTS[0].id;

    prompts::save(&data_dir, id, "edited content").await.unwrap();
    let loaded = prompts::load(&data_dir, &bundled_dir, id).await.unwrap();
    assert_eq!(loaded, "edited content");
}

#[tokio::test]
async fn load_falls_back_to_bundled_when_user_file_missing() {
    let tmp = tempfile::tempdir().unwrap();
    let data_dir = tmp.path().join("data");
    let bundled_dir = tmp.path().join("bundled");
    let id = PROMPTS[0].id;

    write_bundled(&bundled_dir, id, "default body").await;

    let loaded = prompts::load(&data_dir, &bundled_dir, id).await.unwrap();
    assert_eq!(
        loaded, "default body",
        "load must fall back to bundled default if user file absent"
    );
}

#[tokio::test]
async fn load_user_copy_wins_over_bundled() {
    let tmp = tempfile::tempdir().unwrap();
    let data_dir = tmp.path().join("data");
    let bundled_dir = tmp.path().join("bundled");
    let id = PROMPTS[0].id;

    write_bundled(&bundled_dir, id, "default body").await;
    prompts::save(&data_dir, id, "user override").await.unwrap();

    let loaded = prompts::load(&data_dir, &bundled_dir, id).await.unwrap();
    assert_eq!(loaded, "user override");
}

#[tokio::test]
async fn reset_overwrites_user_with_bundled() {
    let tmp = tempfile::tempdir().unwrap();
    let data_dir = tmp.path().join("data");
    let bundled_dir = tmp.path().join("bundled");
    let id = PROMPTS[0].id;

    write_bundled(&bundled_dir, id, "default body").await;
    prompts::save(&data_dir, id, "user override").await.unwrap();

    prompts::reset(&data_dir, &bundled_dir, id).await.unwrap();
    let loaded = prompts::load(&data_dir, &bundled_dir, id).await.unwrap();
    assert_eq!(loaded, "default body");
}

#[tokio::test]
async fn reset_returns_not_found_when_bundled_missing() {
    let tmp = tempfile::tempdir().unwrap();
    let data_dir = tmp.path().join("data");
    let bundled_dir = tmp.path().join("bundled");
    let id = PROMPTS[0].id;

    // Deliberately do NOT write a bundled default.
    let err = prompts::reset(&data_dir, &bundled_dir, id).await.unwrap_err();
    assert!(matches!(err, PromptError::NotFound(_)), "got {err:?}");
}

#[tokio::test]
async fn invalid_id_rejected_by_save() {
    let tmp = tempfile::tempdir().unwrap();
    let err = prompts::save(tmp.path(), "not_in_registry", "x")
        .await
        .unwrap_err();
    assert!(matches!(err, PromptError::InvalidId(_)));
}

#[tokio::test]
async fn invalid_id_rejected_by_load() {
    let tmp = tempfile::tempdir().unwrap();
    let bundled = tmp.path().join("bundled");
    let err = prompts::load(tmp.path(), &bundled, "not_in_registry")
        .await
        .unwrap_err();
    assert!(matches!(err, PromptError::InvalidId(_)));
}

#[tokio::test]
async fn invalid_id_rejected_by_reset() {
    let tmp = tempfile::tempdir().unwrap();
    let bundled = tmp.path().join("bundled");
    let err = prompts::reset(tmp.path(), &bundled, "not_in_registry")
        .await
        .unwrap_err();
    assert!(matches!(err, PromptError::InvalidId(_)));
}

#[tokio::test]
async fn load_default_returns_not_found_for_registered_but_missing() {
    let tmp = tempfile::tempdir().unwrap();
    let bundled = tmp.path().join("bundled");
    let id = PROMPTS[0].id;
    let err = prompts::load_default(&bundled, id).await.unwrap_err();
    assert!(matches!(err, PromptError::NotFound(_)));
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
