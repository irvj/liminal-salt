//! Shared filesystem primitives for the service layer.
//!
//! `write_atomic` is the canonical "durably persist bytes to a path" helper.
//! Every service that writes to `data/` goes through it so a concurrent
//! lockless reader (e.g. `session::list_sessions` reading the sidebar while
//! a background write is in flight) never sees a truncated-zero file.

use std::path::{Path, PathBuf};

use tokio::io::AsyncWriteExt;

/// Durably write `bytes` to `path` in a way that's safe against concurrent
/// readers: write to `<path>.tmp`, fsync, then rename. POSIX `rename` is
/// atomic, so a reader racing the write sees either the pre-write file or
/// the post-write file — never an empty or half-written state.
///
/// Creates the parent directory if absent. Overwrites any stale `<path>.tmp`
/// left by a previous crashed write (the final `path` is what readers see).
pub async fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let tmp = tmp_path_for(path);
    let mut f = tokio::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&tmp)
        .await?;
    f.write_all(bytes).await?;
    f.sync_all().await?;
    drop(f);
    tokio::fs::rename(&tmp, path).await?;
    Ok(())
}

fn tmp_path_for(path: &Path) -> PathBuf {
    let mut s = path.as_os_str().to_os_string();
    s.push(".tmp");
    PathBuf::from(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn writes_bytes_and_cleans_up_tmp() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("payload.json");
        write_atomic(&path, b"hello").await.unwrap();
        assert_eq!(tokio::fs::read_to_string(&path).await.unwrap(), "hello");
        // The .tmp should have been renamed away; no stray file.
        let tmp_path = tmp_path_for(&path);
        assert!(!tmp_path.exists(), "tmp file should not remain after rename");
    }

    #[tokio::test]
    async fn overwrites_existing_target() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("overwrite.txt");
        write_atomic(&path, b"v1").await.unwrap();
        write_atomic(&path, b"v2").await.unwrap();
        assert_eq!(tokio::fs::read_to_string(&path).await.unwrap(), "v2");
    }

    #[tokio::test]
    async fn concurrent_read_never_sees_empty() {
        // Loop a reader + writer in parallel. The reader should never observe
        // an empty/partial file (what the old non-atomic write allowed).
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("race.json");
        write_atomic(&path, b"{\"v\":0}").await.unwrap();

        let writer_path = path.clone();
        let writer = tokio::spawn(async move {
            for i in 1..=50 {
                let body = format!("{{\"v\":{i}}}");
                write_atomic(&writer_path, body.as_bytes()).await.unwrap();
            }
        });

        let reader_path = path.clone();
        let reader = tokio::spawn(async move {
            for _ in 0..200 {
                let bytes = tokio::fs::read(&reader_path).await.unwrap();
                assert!(!bytes.is_empty(), "reader saw an empty file");
                assert!(
                    serde_json::from_slice::<serde_json::Value>(&bytes).is_ok(),
                    "reader saw malformed json: {:?}",
                    String::from_utf8_lossy(&bytes),
                );
            }
        });

        let _ = tokio::join!(writer, reader);
    }
}
