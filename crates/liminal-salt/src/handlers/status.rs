//! Per-error-type HTTP status mappers. Handlers delegate here so a given
//! service-error variant maps to the same HTTP status regardless of which
//! handler surfaces it. Canonical mapping is documented in CLAUDE.md.

use axum::http::StatusCode;

use crate::services::{
    chat::ChatError,
    context_files::ContextScopeError,
    local_context::ReadError,
    memory::MemoryError,
    persona::PersonaError,
    prompts::PromptError,
    session::SessionError,
};

pub fn session_status(err: &SessionError) -> StatusCode {
    match err {
        SessionError::InvalidId(_) | SessionError::InvalidState(_) => StatusCode::BAD_REQUEST,
        SessionError::NotFound(_) => StatusCode::NOT_FOUND,
        SessionError::Io(_) | SessionError::Corrupt(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

pub fn memory_status(err: &MemoryError) -> StatusCode {
    match err {
        MemoryError::InvalidPersonaName(_)
        | MemoryError::NoExistingMemory
        | MemoryError::NoThreads => StatusCode::BAD_REQUEST,
        MemoryError::UnusableResponse
        | MemoryError::Io(_)
        | MemoryError::Llm(_)
        | MemoryError::Prompt(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

pub fn persona_status(err: &PersonaError) -> StatusCode {
    match err {
        PersonaError::InvalidName => StatusCode::BAD_REQUEST,
        PersonaError::AlreadyExists => StatusCode::CONFLICT,
        PersonaError::NotFound => StatusCode::NOT_FOUND,
        PersonaError::Io(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

pub fn chat_status(err: &ChatError) -> StatusCode {
    match err {
        ChatError::SessionNotFound(_) => StatusCode::NOT_FOUND,
        ChatError::Session(inner) => session_status(inner),
        ChatError::LlmFailed(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

pub fn prompt_status(err: &PromptError) -> StatusCode {
    match err {
        PromptError::InvalidId(_) => StatusCode::BAD_REQUEST,
        PromptError::NotFound(_) => StatusCode::NOT_FOUND,
        PromptError::Io(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

pub fn context_scope_status(err: &ContextScopeError) -> StatusCode {
    match err {
        ContextScopeError::InvalidFilename | ContextScopeError::InvalidPath(_) => {
            StatusCode::BAD_REQUEST
        }
        ContextScopeError::NotTracked => StatusCode::NOT_FOUND,
        ContextScopeError::Read(ReadError::InvalidUtf8) => StatusCode::UNPROCESSABLE_ENTITY,
        ContextScopeError::Read(ReadError::Io(_)) => StatusCode::NOT_FOUND,
        ContextScopeError::Io(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}
