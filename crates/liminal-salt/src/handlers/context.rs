//! Context-file endpoints — global (`/settings/context/*`) and per-persona
//! (`/persona/context/*`) uploaded files, plus the local-directory endpoints
//! (`/context/local/*`) that are shared across scopes via an optional
//! `persona` form / query parameter.

use std::path::PathBuf;

use axum::{
    Json,
    extract::{Multipart, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};

use crate::{
    AppState,
    services::{context_files::ContextScope, local_context},
};

// =============================================================================
// Scope construction from a `persona` form/query field
// =============================================================================

fn scope_for(state: &AppState, persona: &str) -> ContextScope {
    if persona.is_empty() {
        ContextScope::global(&state.data_dir)
    } else {
        ContextScope::persona(&state.data_dir, persona)
    }
}

// =============================================================================
// Uploaded files
// =============================================================================

/// Global-scope upload. POST FormData → `{files: [...]}`.
pub async fn upload_global(
    State(state): State<AppState>,
    multipart: Multipart,
) -> Response {
    upload_impl(state, multipart, None).await
}

/// Per-persona upload. Persona name comes from the `persona` form field.
pub async fn upload_persona(
    State(state): State<AppState>,
    multipart: Multipart,
) -> Response {
    upload_impl(state, multipart, Some(())).await
}

async fn upload_impl(
    state: AppState,
    mut multipart: Multipart,
    _is_persona_route: Option<()>,
) -> Response {
    let mut persona = String::new();
    let mut filename: Option<String> = None;
    let mut body: Vec<u8> = Vec::new();

    while let Ok(Some(mut field)) = multipart.next_field().await {
        match field.name() {
            Some("persona") => persona = field.text().await.unwrap_or_default(),
            Some("file") => {
                filename = field
                    .file_name()
                    .map(|s| s.to_string());
                while let Ok(Some(chunk)) = field.chunk().await {
                    body.extend_from_slice(&chunk);
                }
            }
            _ => {}
        }
    }

    let Some(name) = filename else {
        return (StatusCode::BAD_REQUEST, "no file").into_response();
    };
    let scope = scope_for(&state, &persona);
    if scope.upload_file(&name, &body).await.is_none() {
        return (StatusCode::INTERNAL_SERVER_ERROR, "upload failed").into_response();
    }
    let files = scope.list_files().await;
    Json(FilesResponse { files }).into_response()
}

#[derive(Serialize)]
struct FilesResponse {
    files: Vec<crate::services::context_files::ContextFileEntry>,
}

async fn mutate_file_impl<F, Fut>(
    state: AppState,
    mut multipart: Multipart,
    op: F,
) -> Response
where
    F: FnOnce(ContextScope, String) -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    let mut persona = String::new();
    let mut filename: Option<String> = None;
    while let Ok(Some(field)) = multipart.next_field().await {
        match field.name() {
            Some("persona") => persona = field.text().await.unwrap_or_default(),
            Some("filename") => filename = field.text().await.ok(),
            _ => {}
        }
    }
    let Some(fname) = filename else {
        return (StatusCode::BAD_REQUEST, "filename required").into_response();
    };
    let scope = scope_for(&state, &persona);
    let ok = op(scope, fname).await;
    if !ok {
        return (StatusCode::BAD_REQUEST, "operation failed").into_response();
    }
    // Re-list and return.
    let scope = scope_for(&state, &persona);
    Json(FilesResponse {
        files: scope.list_files().await,
    })
    .into_response()
}

pub async fn toggle_file_global(
    State(state): State<AppState>,
    multipart: Multipart,
) -> Response {
    mutate_file_impl(state, multipart, |scope, name| async move {
        scope.toggle_file(&name, None).await.is_some()
    })
    .await
}

pub async fn toggle_file_persona(
    State(state): State<AppState>,
    multipart: Multipart,
) -> Response {
    toggle_file_global(State(state), multipart).await
}

pub async fn delete_file_global(
    State(state): State<AppState>,
    multipart: Multipart,
) -> Response {
    mutate_file_impl(state, multipart, |scope, name| async move {
        scope.delete_file(&name).await
    })
    .await
}

pub async fn delete_file_persona(
    State(state): State<AppState>,
    multipart: Multipart,
) -> Response {
    delete_file_global(State(state), multipart).await
}

// --- get file content (GET with query) ---

#[derive(Deserialize)]
pub struct FileContentQuery {
    #[serde(default)]
    pub persona: String,
    pub filename: String,
}

#[derive(Serialize)]
struct ContentResponse {
    content: String,
}

pub async fn get_file_content(
    State(state): State<AppState>,
    Query(q): Query<FileContentQuery>,
) -> Response {
    let scope = scope_for(&state, &q.persona);
    match scope.get_file_content(&q.filename).await {
        Some(content) => Json(ContentResponse { content }).into_response(),
        None => (StatusCode::NOT_FOUND, "not found").into_response(),
    }
}

// --- save edited content ---

pub async fn save_file_content(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Response {
    let mut persona = String::new();
    let mut filename: Option<String> = None;
    let mut content: Option<String> = None;
    while let Ok(Some(field)) = multipart.next_field().await {
        match field.name() {
            Some("persona") => persona = field.text().await.unwrap_or_default(),
            Some("filename") => filename = field.text().await.ok(),
            Some("content") => content = field.text().await.ok(),
            _ => {}
        }
    }
    let (Some(fname), Some(body)) = (filename, content) else {
        return (StatusCode::BAD_REQUEST, "filename + content required").into_response();
    };
    let scope = scope_for(&state, &persona);
    if !scope.save_file_content(&fname, &body).await {
        return (StatusCode::INTERNAL_SERVER_ERROR, "save failed").into_response();
    }
    StatusCode::NO_CONTENT.into_response()
}

// =============================================================================
// Local directories
// =============================================================================

#[derive(Serialize)]
struct DirectoriesResponse {
    directories: Vec<crate::services::context_files::LocalDirectoryEntry>,
}

#[derive(Deserialize)]
pub struct BrowseQuery {
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub show_hidden: String,
}

#[derive(Serialize)]
struct BrowseResponse {
    current: String,
    parent: Option<String>,
    dirs: Vec<BrowseDirEntry>,
    has_context_files: bool,
    context_files: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Serialize)]
struct BrowseDirEntry {
    name: String,
    path: String,
}

pub async fn browse_directory(
    Query(q): Query<BrowseQuery>,
) -> Response {
    let show_hidden = matches!(q.show_hidden.as_str(), "1" | "true");
    let start: PathBuf = if q.path.is_empty() {
        // Default to home dir for a friendly starting point.
        dirs_home().unwrap_or_else(|| PathBuf::from("/"))
    } else {
        PathBuf::from(&q.path)
    };

    match local_context::browse_directory(&start, show_hidden).await {
        Some(result) => {
            let (dirs, files) = split_browse_entries(result.entries);
            let has_any = !files.is_empty();
            Json(BrowseResponse {
                current: result.path,
                parent: result.parent,
                dirs: dirs
                    .into_iter()
                    .map(|e| BrowseDirEntry {
                        name: e.name,
                        path: e.path,
                    })
                    .collect(),
                has_context_files: has_any,
                context_files: files.into_iter().map(|e| e.name).collect(),
                error: None,
            })
            .into_response()
        }
        None => Json(BrowseResponse {
            current: start.to_string_lossy().to_string(),
            parent: None,
            dirs: Vec::new(),
            has_context_files: false,
            context_files: Vec::new(),
            error: Some("could not browse directory".to_string()),
        })
        .into_response(),
    }
}

fn split_browse_entries(
    entries: Vec<local_context::BrowseEntry>,
) -> (Vec<local_context::BrowseEntry>, Vec<local_context::BrowseEntry>) {
    let (dirs, files): (Vec<_>, Vec<_>) = entries.into_iter().partition(|e| e.is_dir);
    (dirs, files)
}

/// Minimal home-dir resolver (avoids pulling `dirs` crate for one use site).
fn dirs_home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

/// Multipart body parser — extract the 3 string fields the JS sends.
async fn parse_local_fields(
    mut multipart: Multipart,
) -> (String, Option<String>, Option<String>) {
    let mut persona = String::new();
    let mut dir_path: Option<String> = None;
    let mut filename: Option<String> = None;
    while let Ok(Some(field)) = multipart.next_field().await {
        match field.name() {
            Some("persona") => persona = field.text().await.unwrap_or_default(),
            Some("dir_path") => dir_path = field.text().await.ok(),
            Some("filename") => filename = field.text().await.ok(),
            _ => {}
        }
    }
    (persona, dir_path, filename)
}

pub async fn add_directory(
    State(state): State<AppState>,
    multipart: Multipart,
) -> Response {
    let (persona, dir_path, _) = parse_local_fields(multipart).await;
    let Some(path) = dir_path else {
        return (StatusCode::BAD_REQUEST, "dir_path required").into_response();
    };
    let scope = scope_for(&state, &persona);
    match scope.add_local_directory(&path).await {
        Ok((_, _)) => Json(DirectoriesResponse {
            directories: scope_for(&state, &persona).list_local_directories().await,
        })
        .into_response(),
        Err(err) => (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse { error: err }),
        )
            .into_response(),
    }
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

pub async fn remove_directory(
    State(state): State<AppState>,
    multipart: Multipart,
) -> Response {
    let (persona, dir_path, _) = parse_local_fields(multipart).await;
    let Some(path) = dir_path else {
        return (StatusCode::BAD_REQUEST, "dir_path required").into_response();
    };
    let scope = scope_for(&state, &persona);
    scope.remove_local_directory(&path).await;
    Json(DirectoriesResponse {
        directories: scope.list_local_directories().await,
    })
    .into_response()
}

pub async fn toggle_local_file(
    State(state): State<AppState>,
    multipart: Multipart,
) -> Response {
    let (persona, dir_path, filename) = parse_local_fields(multipart).await;
    let (Some(path), Some(fname)) = (dir_path, filename) else {
        return (StatusCode::BAD_REQUEST, "dir_path + filename required").into_response();
    };
    let scope = scope_for(&state, &persona);
    scope.toggle_local_file(&path, &fname, None).await;
    Json(DirectoriesResponse {
        directories: scope.list_local_directories().await,
    })
    .into_response()
}

pub async fn refresh_local_dir(
    State(state): State<AppState>,
    multipart: Multipart,
) -> Response {
    let (persona, dir_path, _) = parse_local_fields(multipart).await;
    let Some(path) = dir_path else {
        return (StatusCode::BAD_REQUEST, "dir_path required").into_response();
    };
    let scope = scope_for(&state, &persona);
    scope.refresh_local_directory(&path).await;
    Json(DirectoriesResponse {
        directories: scope.list_local_directories().await,
    })
    .into_response()
}

#[derive(Deserialize)]
pub struct LocalContentQuery {
    #[serde(default)]
    pub persona: String,
    pub dir_path: String,
    pub filename: String,
}

pub async fn get_local_file_content(
    State(state): State<AppState>,
    Query(q): Query<LocalContentQuery>,
) -> Response {
    use crate::services::{
        context_files::LocalContentError,
        local_context::ReadError,
    };
    let scope = scope_for(&state, &q.persona);
    match scope.get_local_file_content(&q.dir_path, &q.filename).await {
        Ok(content) => Json(ContentResponse { content }).into_response(),
        Err(LocalContentError::InvalidFilename | LocalContentError::DirMissing) => {
            (StatusCode::NOT_FOUND, "not found").into_response()
        }
        Err(LocalContentError::Read(ReadError::Io(_))) => {
            (StatusCode::NOT_FOUND, "not found").into_response()
        }
        Err(LocalContentError::Read(ReadError::InvalidUtf8)) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            "file is not valid UTF-8",
        )
            .into_response(),
    }
}

