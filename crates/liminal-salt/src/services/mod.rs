pub mod chat;
pub mod config;
pub mod context_files;
pub mod fs;
pub mod llm;
pub mod local_context;
pub mod memory;
pub mod memory_worker;
pub mod persona;
pub mod prompt;
pub mod providers;
pub mod session;
pub mod summarizer;
pub mod themes;
pub mod thread_memory;

// Transition re-export so existing `use crate::services::openrouter::*` call
// sites keep compiling while commit 5 migrates them onto `Provider` methods.
pub use providers::openrouter;
