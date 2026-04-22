//! Integration tests for `ContextScope` — uploads, enabled toggle, listing,
//! and the concatenated `load_enabled_context` used by the system prompt.

use liminal_salt::services::context_files::ContextScope;

#[tokio::test]
async fn upload_list_toggle_delete_uploaded_file() {
    let tmp = tempfile::tempdir().unwrap();
    let scope = ContextScope::global(tmp.path());

    let saved = scope
        .upload_file("notes.md", b"hello world")
        .await
        .expect("upload");
    assert_eq!(saved, "notes.md");

    let files = scope.list_files().await;
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].name, "notes.md");
    assert!(files[0].enabled);

    assert_eq!(scope.toggle_file("notes.md", None).await, Some(false));
    assert_eq!(scope.toggle_file("notes.md", Some(true)).await, Some(true));

    assert!(scope.delete_file("notes.md").await);
    assert!(scope.list_files().await.is_empty());
}

#[tokio::test]
async fn upload_sanitizes_traversal_attempts() {
    let tmp = tempfile::tempdir().unwrap();
    let scope = ContextScope::global(tmp.path());

    let saved = scope
        .upload_file("../../passwd", b"nope")
        .await
        .expect("upload");
    assert_eq!(saved, "passwd");
    // File landed inside the scope dir, not outside.
    let inside = scope.base_dir().join("passwd");
    assert!(tokio::fs::try_exists(&inside).await.unwrap());
}

#[tokio::test]
async fn load_enabled_context_emits_scope_header_and_file_bodies() {
    let tmp = tempfile::tempdir().unwrap();
    let scope = ContextScope::global(tmp.path());

    scope.upload_file("one.md", b"first").await.unwrap();
    scope.upload_file("two.md", b"second").await.unwrap();

    let out = scope.load_enabled_context().await;
    assert!(out.contains("--- USER CONTEXT FILES ---"));
    assert!(out.contains("--- one.md ---"));
    assert!(out.contains("first"));
    assert!(out.contains("--- two.md ---"));
    assert!(out.contains("second"));
}

#[tokio::test]
async fn disabled_files_are_excluded_from_enabled_context() {
    let tmp = tempfile::tempdir().unwrap();
    let scope = ContextScope::global(tmp.path());

    scope.upload_file("keep.md", b"present").await.unwrap();
    scope.upload_file("skip.md", b"hidden").await.unwrap();
    scope.toggle_file("skip.md", Some(false)).await.unwrap();

    let out = scope.load_enabled_context().await;
    assert!(out.contains("keep.md"));
    assert!(out.contains("present"));
    assert!(!out.contains("skip.md"));
    assert!(!out.contains("hidden"));
}

#[tokio::test]
async fn persona_scope_uses_persona_header() {
    let tmp = tempfile::tempdir().unwrap();
    let scope = ContextScope::persona(tmp.path(), "coach");
    scope.upload_file("style.md", b"firm but fair").await.unwrap();
    let out = scope.load_enabled_context().await;
    assert!(out.contains("--- PERSONA CONTEXT FILES ---"));
    assert!(out.contains("firm but fair"));
}

#[tokio::test]
async fn empty_scope_renders_empty_string() {
    let tmp = tempfile::tempdir().unwrap();
    let scope = ContextScope::global(tmp.path());
    let out = scope.load_enabled_context().await;
    assert!(out.is_empty());
}

#[tokio::test]
async fn save_and_get_file_content_roundtrip() {
    let tmp = tempfile::tempdir().unwrap();
    let scope = ContextScope::global(tmp.path());
    scope.upload_file("memo.md", b"v1").await.unwrap();
    assert!(scope.save_file_content("memo.md", "v2 rewritten").await);
    assert_eq!(
        scope.get_file_content("memo.md").await.as_deref(),
        Some("v2 rewritten")
    );
}

#[tokio::test]
async fn add_and_remove_local_directory() {
    let tmp = tempfile::tempdir().unwrap();
    // External directory we reference from the scope.
    let external = tempfile::tempdir().unwrap();
    tokio::fs::write(external.path().join("a.md"), "alpha")
        .await
        .unwrap();
    tokio::fs::write(external.path().join("b.md"), "beta")
        .await
        .unwrap();
    tokio::fs::write(external.path().join("ignored.py"), "x")
        .await
        .unwrap();

    let scope = ContextScope::global(tmp.path());
    let ext_str = external.path().to_string_lossy().to_string();
    let (resolved, files) = scope
        .add_local_directory(&ext_str)
        .await
        .expect("add local dir");
    assert!(resolved.contains(external.path().file_name().unwrap().to_str().unwrap()));
    let names: Vec<&str> = files.iter().map(|f| f.name.as_str()).collect();
    assert!(names.contains(&"a.md"));
    assert!(names.contains(&"b.md"));
    assert!(!names.contains(&"ignored.py")); // non-md/txt filtered

    let dirs = scope.list_local_directories().await;
    assert_eq!(dirs.len(), 1);
    assert!(dirs[0].exists);

    assert!(scope.remove_local_directory(&resolved).await);
    assert!(scope.list_local_directories().await.is_empty());
}

#[tokio::test]
async fn local_directory_content_appears_in_enabled_context() {
    let tmp = tempfile::tempdir().unwrap();
    let external = tempfile::tempdir().unwrap();
    tokio::fs::write(external.path().join("facts.md"), "external fact body")
        .await
        .unwrap();

    let scope = ContextScope::global(tmp.path());
    let (resolved, _) = scope
        .add_local_directory(&external.path().to_string_lossy())
        .await
        .unwrap();

    let out = scope.load_enabled_context().await;
    assert!(out.contains("--- LOCAL CONTEXT FILES ---"));
    assert!(out.contains(&format!("facts.md (from {resolved})")));
    assert!(out.contains("external fact body"));
}

#[tokio::test]
async fn toggled_off_local_file_is_excluded() {
    let tmp = tempfile::tempdir().unwrap();
    let external = tempfile::tempdir().unwrap();
    tokio::fs::write(external.path().join("keep.md"), "keep body")
        .await
        .unwrap();
    tokio::fs::write(external.path().join("skip.md"), "skip body")
        .await
        .unwrap();

    let scope = ContextScope::global(tmp.path());
    let (resolved, _) = scope
        .add_local_directory(&external.path().to_string_lossy())
        .await
        .unwrap();

    scope
        .toggle_local_file(&resolved, "skip.md", Some(false))
        .await
        .expect("toggle");

    let out = scope.load_enabled_context().await;
    assert!(out.contains("keep body"));
    assert!(!out.contains("skip body"));
}
