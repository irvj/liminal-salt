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
    ctx.insert("grouped_sessions", &Vec::<serde_json::Value>::new());
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
    ctx.insert("grouped_sessions", &Vec::<serde_json::Value>::new());

    let out = tera.render("chat/chat.html", &ctx).expect("render main");
    assert!(out.contains("Evening chat"));
    assert!(out.contains("<strong>there</strong>"), "markdown filter applied to user message");
    assert!(out.contains("hello!"));
    assert!(out.contains("session_20260421_150000.json"));
    // Chat mode is chatbot → fork-to-roleplay button rendered, scenario button not.
    assert!(out.contains("/session/fork-to-roleplay/"));

    // Phase 5 modals must be present: stripping them silently breaks the
    // brain-cog icon's click handler (dispatches an event that nothing
    // listens for). Guard against regressing the Phase 3b omission.
    assert!(
        out.contains(r#"x-data="threadMemoryModal""#),
        "thread memory modal must be registered in chat.html"
    );
    assert!(
        out.contains(r#"data-update-url="/session/thread-memory/update/""#),
        "thread memory modal wiring must point at the Phase 5 endpoint"
    );
    assert!(
        out.contains(r#"data-settings-save-url="/session/thread-memory/settings/save/""#),
        "thread memory modal must wire the settings-save endpoint"
    );
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
    ctx.insert("grouped_sessions", &Vec::<serde_json::Value>::new());

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
    ctx.insert("grouped_sessions", &Vec::<serde_json::Value>::new());

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
    ctx.insert(
        "grouped_sessions",
        &json!([
            {
                "persona": "assistant",
                "sessions": [{
                    "id": "session_20260421_140000.json",
                    "title": "Earlier session",
                    "persona": "assistant",
                    "mode": "chatbot"
                }]
            }
        ]),
    );

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

#[test]
fn persona_page_renders_with_personas() {
    let tera = build_tera();
    let mut ctx = base_context();
    ctx.insert("page", "persona");
    ctx.insert("show_home", &false);
    ctx.insert(
        "personas",
        &vec!["assistant".to_string(), "riddler".to_string()],
    );
    ctx.insert("default_persona", "assistant");
    ctx.insert("selected_persona", "assistant");
    ctx.insert("persona_preview", "# Assistant\n\nYou help with tasks.");
    ctx.insert("model", "anthropic/claude-opus-4-7");
    ctx.insert("persona_model", "");
    ctx.insert("pinned_sessions", &Vec::<serde_json::Value>::new());
    ctx.insert("grouped_sessions", &Vec::<serde_json::Value>::new());

    let out = tera.render("chat/chat.html", &ctx).expect("render persona");
    assert!(out.contains("Persona Settings"));
    assert!(out.contains("Set as Default"));
    assert!(out.contains("assistant"));
    // Persona modals present.
    assert!(out.contains("editPersonaModal"));
    assert!(out.contains("editPersonaModelModal"));
    assert!(out.contains("open-persona-context-files-modal"));
}

#[test]
fn memory_page_renders_with_empty_memory() {
    let tera = build_tera();
    let mut ctx = base_context();
    ctx.insert("page", "memory");
    ctx.insert("show_home", &false);
    ctx.insert("selected_persona", "assistant");
    ctx.insert(
        "available_personas",
        &vec!["assistant".to_string(), "riddler".to_string()],
    );
    ctx.insert("model", "anthropic/claude-opus-4-7");
    ctx.insert("memory_content", "");
    ctx.insert("pinned_sessions", &Vec::<serde_json::Value>::new());
    ctx.insert("grouped_sessions", &Vec::<serde_json::Value>::new());

    let out = tera.render("chat/chat.html", &ctx).expect("render memory");
    assert!(out.contains("Assistant's Memory"));
    assert!(out.contains("Update Memory"));
    assert!(out.contains("Seed Memory"));
    assert!(out.contains("Wipe Memory"));
    assert!(out.contains("No memory yet for Assistant"));
    // Context file modals reachable.
    assert!(out.contains("context-files-data") || out.contains("persona-context-files-data"));
}

#[test]
fn memory_page_renders_with_content() {
    let tera = build_tera();
    let mut ctx = base_context();
    ctx.insert("page", "memory");
    ctx.insert("show_home", &false);
    ctx.insert("selected_persona", "assistant");
    ctx.insert("available_personas", &vec!["assistant".to_string()]);
    ctx.insert("model", "m/id");
    ctx.insert("memory_content", "- prefers pineapple on pizza");
    ctx.insert("pinned_sessions", &Vec::<serde_json::Value>::new());
    ctx.insert("grouped_sessions", &Vec::<serde_json::Value>::new());

    let out = tera.render("chat/chat.html", &ctx).expect("render memory");
    assert!(out.contains("pineapple on pizza"));
    // Body rendered via markdown filter — should contain a list item.
    assert!(out.contains("<li>"));
}

#[test]
fn context_files_modal_renders_title_and_description() {
    let tera = build_tera();
    let mut ctx = base_context();
    ctx.insert("title", "Global context");
    ctx.insert("description", "test description");
    ctx.insert("empty_text", "nothing here");
    let out = tera
        .render("chat/context_files_modal.html", &ctx)
        .expect("modal render");
    assert!(out.contains("Global context"));
    assert!(out.contains("test description"));
    assert!(out.contains("nothing here"));
    assert!(out.contains("Drag & drop"));
    assert!(out.contains("Uploaded Files"));
    assert!(out.contains("Local Directory"));
    // Nested modals present.
    assert!(out.contains("Browse Directories"));
}

// =============================================================================
// Setup wizard (Phase 6b)
// =============================================================================

#[test]
fn setup_step1_renders_provider_picker() {
    let tera = build_tera();
    let mut ctx = base_context();
    ctx.insert(
        "providers",
        &json!([{
            "id": "openrouter",
            "name": "OpenRouter",
            "api_key_url": "https://openrouter.ai/keys",
            "api_key_placeholder": "sk-or-v1-..."
        }]),
    );
    ctx.insert("selected_provider", "openrouter");
    ctx.insert("api_key", "");

    let out = tera.render("setup/step1.html", &ctx).expect("render step1");
    assert!(out.contains("Step 1: Connect Your Provider"));
    assert!(out.contains("OpenRouter"));
    assert!(out.contains("sk-or-v1-..."));
    assert!(out.contains("Validate & Continue"));
    // CSRF token field + meta both wired.
    assert!(out.contains(r#"name="csrfmiddlewaretoken""#));
    assert!(out.contains("test_csrf_abcd1234"));
}

#[test]
fn setup_step1_renders_error_banner() {
    let tera = build_tera();
    let mut ctx = base_context();
    ctx.insert("providers", &Vec::<serde_json::Value>::new());
    ctx.insert("selected_provider", "openrouter");
    ctx.insert("api_key", "");
    ctx.insert("error", "Invalid API key. Please check your key and try again.");

    let out = tera.render("setup/step1.html", &ctx).expect("render step1 error");
    assert!(out.contains("Invalid API key"));
}

#[test]
fn setup_step2_renders_model_and_theme_pickers() {
    let tera = build_tera();
    let mut ctx = base_context();
    ctx.insert(
        "available_models",
        &json!([
            { "id": "anthropic/claude", "display": "Anthropic: Claude - $3.00/$15.00 per 1M" },
            { "id": "openai/gpt-4", "display": "Openai: GPT-4 - Free" }
        ]),
    );
    ctx.insert(
        "available_models_json",
        r#"[{"id":"anthropic/claude","display":"Claude"}]"#,
    );
    ctx.insert("model_count", &2);
    ctx.insert("selected_model", "");
    ctx.insert(
        "themes",
        &json!([
            { "id": "liminal-salt", "name": "Liminal Salt" },
            { "id": "dracula", "name": "Dracula" }
        ]),
    );
    ctx.insert(
        "themes_json",
        r#"[{"id":"liminal-salt","name":"Liminal Salt"}]"#,
    );
    ctx.insert("selected_theme", "liminal-salt");
    ctx.insert("selected_mode", "dark");

    let out = tera.render("setup/step2.html", &ctx).expect("render step2");
    assert!(out.contains("Step 2: Choose Your Preferences"));
    assert!(out.contains("Select a model"));
    // Theme selector dropdowns present.
    assert!(out.contains("Select a theme..."));
    // Dark/Light mode buttons render via the icons macro (no import bleed).
    assert!(out.contains("Dark"));
    assert!(out.contains("Light"));
    // Back button present so step 2 can walk back to step 1.
    assert!(out.contains(r#"value="back""#));
}

#[test]
fn setup_step3_renders_agreement_and_accept() {
    let tera = build_tera();
    let mut ctx = base_context();
    ctx.insert("agreement_version", "1.1");
    ctx.insert(
        "agreement_body",
        "# Liminal Salt\n\nThis is **bold** in the agreement body.",
    );
    ctx.insert("can_go_back", &true);

    let out = tera.render("setup/step3.html", &ctx).expect("render step3");
    assert!(out.contains("Version 1.1"));
    // Markdown filter applied to agreement body.
    assert!(out.contains("<strong>bold</strong>"));
    assert!(out.contains("I Agree"));
    // Back button visible only when can_go_back is true.
    assert!(out.contains(r#"value="back""#));
}

#[test]
fn setup_step3_hides_back_when_agreement_reprompt() {
    let tera = build_tera();
    let mut ctx = base_context();
    ctx.insert("agreement_version", "1.1");
    ctx.insert("agreement_body", "body");
    ctx.insert("can_go_back", &false);

    let out = tera.render("setup/step3.html", &ctx).expect("render step3 no-back");
    // Accept still renders; back button absent.
    assert!(out.contains("I Agree"));
    assert!(!out.contains(r#"value="back""#));
}
