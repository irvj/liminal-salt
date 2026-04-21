//! Template render smoke tests. Exercises every Phase 3 template with plausible
//! context so render-time errors (missing filters, wrong variable types, bad
//! syntax missed by the parser) surface before 3c's handlers try to use them.

use std::path::PathBuf;

use serde_json::json;
use tera::{Context, Tera};

fn build_tera() -> Tera {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let glob = manifest_dir
        .join("templates")
        .join("**")
        .join("*.html");
    let mut tera = Tera::new(glob.to_str().expect("utf-8 path")).expect("tera parses");
    liminal_salt::tera_extra::register(&mut tera);
    tera
}

fn base_context() -> Context {
    let mut ctx = Context::new();
    ctx.insert("csrf_token", "test_csrf_abcd1234");
    ctx.insert("theme_mode", "dark");
    ctx.insert("color_theme", "liminal-salt");
    ctx
}

#[test]
fn all_templates_parse() {
    // Tera::new() already ran in build_tera; this test documents that any
    // registered template is compile-time-checked.
    let _ = build_tera();
}

#[test]
fn base_renders_blocks() {
    let tera = build_tera();
    let ctx = base_context();
    let out = tera.render("base.html", &ctx).expect("render base");
    assert!(out.contains("<!DOCTYPE html>"));
    assert!(out.contains("csrf-token"));
    assert!(out.contains("test_csrf_abcd1234"));
    assert!(out.contains("toastContainer"));
}

#[test]
fn chat_home_renders_with_personas() {
    let tera = build_tera();
    let mut ctx = base_context();
    ctx.insert("show_home", &true);
    ctx.insert(
        "personas",
        &vec!["assistant".to_string(), "riddler".to_string()],
    );
    ctx.insert("default_persona", "assistant");
    ctx.insert("default_model", "anthropic/claude-opus-4-7");
    ctx.insert("pinned_sessions", &Vec::<serde_json::Value>::new());
    ctx.insert(
        "grouped_sessions",
        &serde_json::Map::<String, serde_json::Value>::new(),
    );
    let out = tera.render("chat/chat.html", &ctx).expect("render home");
    assert!(out.contains("Liminal Salt"));
    assert!(out.contains("Select persona"));
    assert!(out.contains("assistant"));
}

#[test]
fn chat_main_renders_session_with_messages() {
    let tera = build_tera();
    let mut ctx = base_context();
    ctx.insert("show_home", &false);
    ctx.insert("session_id", "session_20260421_150000.json");
    ctx.insert("title", "Evening chat");
    ctx.insert("persona", "riddler");
    ctx.insert("model", "anthropic/claude-opus-4-7");
    ctx.insert("mode", "chatbot");
    ctx.insert("draft", "");
    ctx.insert(
        "messages",
        &json!([
            { "role": "user", "content": "hi **there**", "timestamp": "2026-04-21T12:00:00.000000Z" },
            { "role": "assistant", "content": "hello!", "timestamp": "2026-04-21T12:00:01.000000Z" },
        ]),
    );
    ctx.insert("scenario", "");
    ctx.insert("thread_memory", "");
    ctx.insert("thread_memory_updated_at", "");
    ctx.insert("pinned_sessions", &Vec::<serde_json::Value>::new());
    ctx.insert(
        "grouped_sessions",
        &serde_json::Map::<String, serde_json::Value>::new(),
    );

    let out = tera.render("chat/chat.html", &ctx).expect("render main");
    assert!(out.contains("Evening chat"));
    assert!(out.contains("<strong>there</strong>"), "markdown filter applied to user message");
    assert!(out.contains("hello!"));
    assert!(out.contains("session_20260421_150000.json"));
    // Chat mode is chatbot → fork-to-roleplay button rendered, scenario button not.
    assert!(out.contains("/session/fork-to-roleplay/"));
}

#[test]
fn chat_main_renders_error_message_without_markdown() {
    let tera = build_tera();
    let mut ctx = base_context();
    ctx.insert("show_home", &false);
    ctx.insert("session_id", "session_20260421_150001.json");
    ctx.insert("title", "Oops");
    ctx.insert("persona", "assistant");
    ctx.insert("model", "model/id");
    ctx.insert("mode", "chatbot");
    ctx.insert("draft", "");
    ctx.insert(
        "messages",
        &json!([
            { "role": "user", "content": "hi", "timestamp": "2026-04-21T12:00:00.000000Z" },
            { "role": "assistant", "content": "ERROR: network down", "timestamp": "2026-04-21T12:00:01.000000Z" },
        ]),
    );
    ctx.insert("scenario", "");
    ctx.insert("thread_memory", "");
    ctx.insert("thread_memory_updated_at", "");
    ctx.insert("pinned_sessions", &Vec::<serde_json::Value>::new());
    ctx.insert(
        "grouped_sessions",
        &serde_json::Map::<String, serde_json::Value>::new(),
    );

    let out = tera.render("chat/chat.html", &ctx).expect("render error message");
    // "ERROR:" prefix triggers the error path — shows "Error:" heading and the
    // text in a <pre>, stripped of the leading "ERROR:".
    assert!(out.contains("Error:"));
    assert!(out.contains("<pre"));
    assert!(out.contains(" network down"));
}

#[test]
fn chat_main_roleplay_shows_scenario_button() {
    let tera = build_tera();
    let mut ctx = base_context();
    ctx.insert("show_home", &false);
    ctx.insert("session_id", "session_20260421_150002.json");
    ctx.insert("title", "The Gate");
    ctx.insert("persona", "riddler");
    ctx.insert("model", "m/id");
    ctx.insert("mode", "roleplay");
    ctx.insert("scenario", "a foggy graveyard");
    ctx.insert("draft", "");
    ctx.insert("messages", &Vec::<serde_json::Value>::new());
    ctx.insert("thread_memory", "");
    ctx.insert("thread_memory_updated_at", "");
    ctx.insert("pinned_sessions", &Vec::<serde_json::Value>::new());
    ctx.insert(
        "grouped_sessions",
        &serde_json::Map::<String, serde_json::Value>::new(),
    );

    let out = tera.render("chat/chat.html", &ctx).expect("render roleplay");
    // Roleplay mode: "Roleplay" label in header, scenario button instead of fork.
    assert!(out.contains("Roleplay"));
    assert!(out.contains("open-scenario-modal"));
    assert!(!out.contains("/session/fork-to-roleplay/"));
}

#[test]
fn sidebar_sessions_renders_pinned_and_grouped() {
    let tera = build_tera();
    let mut ctx = base_context();
    ctx.insert("current_session", "session_20260421_150000.json");
    ctx.insert(
        "pinned_sessions",
        &json!([
            {
                "id": "session_20260421_150000.json",
                "title": "Evening chat",
                "persona": "riddler",
                "mode": "chatbot"
            }
        ]),
    );
    // Tera's {% for persona, sessions in grouped %} needs a map.
    let mut grouped = serde_json::Map::new();
    grouped.insert(
        "assistant".to_string(),
        json!([{
            "id": "session_20260421_140000.json",
            "title": "Earlier session",
            "persona": "assistant",
            "mode": "chatbot"
        }]),
    );
    ctx.insert("grouped_sessions", &grouped);

    let out = tera
        .render("chat/sidebar_sessions.html", &ctx)
        .expect("render sidebar");
    assert!(out.contains("Evening chat"));
    assert!(out.contains("Earlier session"));
    assert!(out.contains("Pinned"));
    assert!(out.contains("Assistant")); // display_name filter
}

#[test]
fn assistant_fragment_renders_markdown_and_escapes() {
    let tera = build_tera();
    let mut ctx = base_context();
    ctx.insert("assistant_message", "**bold** \"quoted\"");
    ctx.insert("assistant_timestamp", "2026-04-21T12:00:00.000000Z");

    let out = tera
        .render("chat/assistant_fragment.html", &ctx)
        .expect("render fragment");
    assert!(out.contains("<strong>bold</strong>"));
    // The data-message attribute should be JS-escaped: " → "
    assert!(out.contains(r#""quoted""#));
}

#[test]
fn new_chat_button_renders_standalone() {
    let tera = build_tera();
    let ctx = base_context();
    let out = tera
        .render("chat/new_chat_button.html", &ctx)
        .expect("render button");
    assert!(out.contains("New Chat"));
    assert!(out.contains("/chat/new/"));
}
