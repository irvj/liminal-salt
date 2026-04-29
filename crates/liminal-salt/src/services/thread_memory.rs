//! Per-session running summary — the "working memory" of a single thread,
//! maintained by LLM merge. Distinct from persona memory: thread memory lives
//! inline on the session JSON (`thread_memory`, `thread_memory_updated_at`),
//! is bounded by the session's lifetime, and is written in the persona's voice
//! about what happened in *this* thread only.
//!
//! This module returns the merged summary text; actually persisting it on the
//! session goes through `session::save_thread_memory` so the session's per-id
//! lock coordinates with other writers.

use std::path::Path;

use crate::services::{
    llm::{ChatLlm, LlmMessage},
    persona::PersonaConfig,
    prompts,
    session::{Message, Mode, Role, Session},
};

pub const DEFAULT_THREAD_MEMORY_SIZE: u32 = 4000;

/// `0` means auto-update is disabled by default. Matches persona memory's
/// `auto_memory_interval` default — users opt in via the UI.
pub const DEFAULT_THREAD_MEMORY_INTERVAL_MINUTES: u32 = 0;
pub const DEFAULT_THREAD_MEMORY_MESSAGE_FLOOR: u32 = 4;

/// Fully resolved settings after walking per-thread override → persona default →
/// global fallback. Everything's concrete (no `Option`) so callers don't need
/// to re-apply defaults.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub struct EffectiveThreadMemorySettings {
    pub interval_minutes: u32,
    pub message_floor: u32,
    pub size_limit: u32,
}

impl Default for EffectiveThreadMemorySettings {
    fn default() -> Self {
        Self {
            interval_minutes: DEFAULT_THREAD_MEMORY_INTERVAL_MINUTES,
            message_floor: DEFAULT_THREAD_MEMORY_MESSAGE_FLOOR,
            size_limit: DEFAULT_THREAD_MEMORY_SIZE,
        }
    }
}

/// Walk persona config + global fallback to compute a persona's effective
/// thread-memory defaults (no per-thread override applied). Used to prefill
/// the persona settings form.
pub fn resolve_persona_defaults(persona_cfg: &PersonaConfig) -> EffectiveThreadMemorySettings {
    let mut out = EffectiveThreadMemorySettings::default();
    if let Some(def) = persona_cfg.default_thread_memory_settings.as_ref() {
        if let Some(v) = def.interval_minutes {
            out.interval_minutes = v;
        }
        if let Some(v) = def.message_floor {
            out.message_floor = v;
        }
        if let Some(v) = def.size_limit {
            out.size_limit = v;
        }
    }
    out
}

/// Walk per-thread override → persona default → global fallback. If `session`
/// is `None` (e.g. a fresh session being previewed in the UI) only the last
/// two layers apply.
pub fn resolve_settings(
    session: Option<&Session>,
    persona_cfg: &PersonaConfig,
) -> EffectiveThreadMemorySettings {
    let mut out = resolve_persona_defaults(persona_cfg);
    if let Some(s) = session
        && let Some(ov) = s.thread_memory_settings.as_ref()
    {
        if let Some(v) = ov.interval_minutes {
            out.interval_minutes = v;
        }
        if let Some(v) = ov.message_floor {
            out.message_floor = v;
        }
        if let Some(v) = ov.size_limit {
            out.size_limit = v;
        }
    }
    out
}

/// Messages that aren't covered by the current summary. When `updated_at` is
/// empty (first run, or thread memory was wiped) every message is new.
///
/// A message without a timestamp is anomalous — every write path sets one —
/// but we include it and log rather than silently drop content.
pub fn filter_new_messages(messages: &[Message], updated_at: &str) -> Vec<Message> {
    if updated_at.is_empty() {
        return messages.to_vec();
    }
    let mut out = Vec::new();
    for m in messages {
        if m.timestamp.is_empty() {
            tracing::warn!("filter_new_messages: message without timestamp, including as new");
            out.push(m.clone());
        } else if m.timestamp.as_str() > updated_at {
            out.push(m.clone());
        }
    }
    out
}

/// Inputs to a single thread-memory merge.
///
/// `persona_memory` is the cross-thread persona memory used to color the
/// chatbot-variant merge (suppressed in roleplay for immersion). Set to `""`
/// in roleplay mode or if no memory file exists.
pub struct MergeRequest<'a> {
    pub data_dir: &'a Path,
    pub bundled_prompts_dir: &'a Path,
    pub persona_display_name: &'a str,
    pub persona_memory: &'a str,
    pub existing_memory: &'a str,
    pub new_messages: &'a [Message],
    pub size_limit: u32,
    pub mode: Mode,
}

/// Merge new messages into the existing thread summary via LLM. Returns the
/// updated summary on success, or `None` on LLM error / safety rejection /
/// prompt-load failure.
pub async fn merge<L: ChatLlm>(llm: &L, req: MergeRequest<'_>) -> Option<String> {
    let MergeRequest {
        data_dir,
        bundled_prompts_dir,
        persona_display_name,
        persona_memory,
        existing_memory,
        new_messages,
        size_limit,
        mode,
    } = req;

    if new_messages.is_empty() {
        return None;
    }

    let transcript = format_transcript(new_messages, persona_display_name);

    let size_instruction = if size_limit > 0 {
        format!(
            "SIZE TARGET: Aim for roughly {size_limit} characters. Go over when\n\
             the alternative is losing events — losing a topic entirely is never\n\
             the right trade. If the memory won't fit, compress detail (the dish\n\
             becomes \"steak and potatoes\"), don't drop what happened.\n\n"
        )
    } else {
        String::new()
    };

    let existing_block = if existing_memory.is_empty() {
        "No summary yet. This is the start of the thread.".to_string()
    } else {
        existing_memory.to_string()
    };

    let prompt_id = match mode {
        Mode::Roleplay => "thread_memory_merge_roleplay",
        Mode::Chatbot => "thread_memory_merge_chatbot",
    };
    let instructions = match prompts::load(data_dir, bundled_prompts_dir, prompt_id).await {
        Ok(s) => s,
        Err(err) => {
            tracing::error!(prompt = prompt_id, error = %err, "thread memory prompt load failed");
            return None;
        }
    };
    let instructions = instructions.trim_end();

    let prompt = match mode {
        Mode::Roleplay => build_roleplay_prompt(
            &existing_block,
            &transcript,
            instructions,
            &size_instruction,
        ),
        Mode::Chatbot => build_chatbot_prompt(
            persona_display_name,
            &existing_block,
            &transcript,
            instructions,
            &size_instruction,
            persona_memory,
        ),
    };

    let response = match llm
        .complete(&[LlmMessage::new(Role::User, prompt)])
        .await
    {
        Ok(r) => r,
        Err(err) => {
            tracing::error!(error = %err, "thread memory merge LLM call failed");
            return None;
        }
    };

    // Safety: don't replace substantial memory with a suspiciously short output.
    if response.len() < 10 && existing_memory.len() > 50 {
        tracing::warn!(
            response_len = response.len(),
            "thread memory merge rejected: response too short"
        );
        return None;
    }

    Some(response)
}

// =============================================================================
// Helpers
// =============================================================================

fn format_transcript(messages: &[Message], persona_display_name: &str) -> String {
    let mut out = String::new();
    for (i, msg) in messages.iter().enumerate() {
        if i > 0 {
            out.push_str("\n\n");
        }
        let label = match msg.role {
            Role::User => "User",
            _ => persona_display_name,
        };
        out.push_str(label);
        out.push_str(": ");
        out.push_str(&msg.content);
    }
    out
}

fn build_chatbot_prompt(
    persona_display_name: &str,
    existing_block: &str,
    transcript: &str,
    instructions: &str,
    size_instruction: &str,
    persona_memory: &str,
) -> String {
    let trimmed_memory = persona_memory.trim();
    let persona_memory_block = if trimmed_memory.is_empty() {
        String::new()
    } else {
        format!(
            "--- WHAT YOU ALREADY KNOW ABOUT THIS PERSON ---\n\
             {trimmed_memory}\n\n\
             NOTE: The section above is your long-running memory about this\n\
             person, carried in from other conversations. It tells you who they\n\
             are to you — use it as the lens through which you read this thread.\n\
             DO NOT copy facts from it into the summary below. The summary is a\n\
             record of THIS thread only; what you already knew about them lives\n\
             elsewhere and doesn't need repeating here.\n\n"
        )
    };

    format!(
        "You are {persona_display_name}. Below is your working memory of a\n\
         conversation thread with this person — what you remember of everything\n\
         said so far. This is memory, not transcript. You don't recall verbatim.\n\
         You remember what happened: what they told you, what you worked through\n\
         together, the shape of the exchange. A long back-and-forth compresses\n\
         to its essence — \"they walked through the three options and picked the\n\
         middle one\" — not the full dialogue. What got said stays; the exact\n\
         wording doesn't.\n\n\
         Write in the register that actually fits THIS thread and THIS person.\n\
         A technical working session reads differently than a late-night vent;\n\
         a one-off question reads differently than a conversation with someone\n\
         you've known a long time. Read the thread and, if present, your prior\n\
         memory of them — and let what's there decide the voice. Don't impose\n\
         a tone that isn't earned by the material.\n\n\
         {persona_memory_block}\
         --- CURRENT THREAD SUMMARY ---\n\
         {existing_block}\n\n\
         --- NEW MESSAGES (since last update) ---\n\
         {transcript}\n\n\
         --- INSTRUCTIONS ---\n\n\
         {instructions}\n\n\
         {size_instruction}\
         Return ONLY the updated summary. No preamble, no explanation."
    )
}

fn build_roleplay_prompt(
    existing_block: &str,
    transcript: &str,
    instructions: &str,
    size_instruction: &str,
) -> String {
    format!(
        "You are maintaining a working memory of a ROLEPLAY thread — the whole\n\
         scene as you'd remember a film or a chapter you finished reading. You\n\
         don't replay every line. You remember what happened: where characters\n\
         went, what they did and said to each other, what shifted between them,\n\
         the beats that mattered. Exact dialogue compresses to the moment it\n\
         captured. The arc stays; the word-for-word doesn't.\n\n\
         --- CURRENT SCENE SUMMARY ---\n\
         {existing_block}\n\n\
         --- NEW MESSAGES (since last update) ---\n\
         {transcript}\n\n\
         --- INSTRUCTIONS ---\n\n\
         {instructions}\n\n\
         {size_instruction}\
         Return ONLY the updated summary. No preamble, no explanation."
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::persona::ThreadMemoryDefaults;
    use crate::services::session::ThreadMemorySettings;

    fn msg(role: Role, content: &str, ts: &str) -> Message {
        Message {
            role,
            content: content.to_string(),
            timestamp: ts.to_string(),
        }
    }

    #[test]
    fn resolve_settings_walks_three_tiers() {
        // Global fallback only.
        let cfg = PersonaConfig::default();
        assert_eq!(
            resolve_settings(None, &cfg),
            EffectiveThreadMemorySettings::default()
        );

        // Persona default overrides interval only.
        let cfg = PersonaConfig {
            default_thread_memory_settings: Some(ThreadMemoryDefaults {
                interval_minutes: Some(60),
                message_floor: None,
                size_limit: None,
            }),
            ..Default::default()
        };
        let resolved = resolve_settings(None, &cfg);
        assert_eq!(resolved.interval_minutes, 60);
        assert_eq!(resolved.message_floor, DEFAULT_THREAD_MEMORY_MESSAGE_FLOOR);
        assert_eq!(resolved.size_limit, DEFAULT_THREAD_MEMORY_SIZE);

        // Per-thread override wins over persona default.
        let mut session = crate::services::session::Session {
            title: String::new(),
            title_locked: None,
            persona: "assistant".to_string(),
            mode: Mode::Chatbot,
            messages: vec![],
            draft: None,
            pinned: None,
            scenario: None,
            thread_memory: String::new(),
            thread_memory_updated_at: String::new(),
            thread_memory_settings: Some(ThreadMemorySettings {
                interval_minutes: Some(30),
                message_floor: Some(10),
                size_limit: None,
            }),
        };
        let resolved = resolve_settings(Some(&session), &cfg);
        assert_eq!(resolved.interval_minutes, 30);
        assert_eq!(resolved.message_floor, 10);
        // size_limit falls through to global since neither override nor persona set it.
        assert_eq!(resolved.size_limit, DEFAULT_THREAD_MEMORY_SIZE);

        // Override present but with all None fields is a no-op (falls through).
        session.thread_memory_settings = Some(ThreadMemorySettings::default());
        let resolved = resolve_settings(Some(&session), &cfg);
        assert_eq!(resolved.interval_minutes, 60);
    }

    #[test]
    fn filter_new_messages_respects_cutoff() {
        let messages = vec![
            msg(Role::User, "old", "2026-04-01T10:00:00.000000Z"),
            msg(Role::Assistant, "old reply", "2026-04-01T10:00:01.000000Z"),
            msg(Role::User, "new", "2026-04-10T10:00:00.000000Z"),
        ];
        let got = filter_new_messages(&messages, "2026-04-05T00:00:00.000000Z");
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].content, "new");

        // Empty cutoff returns everything.
        let got = filter_new_messages(&messages, "");
        assert_eq!(got.len(), 3);

        // Missing-timestamp messages are included (and logged).
        let mut missing = messages.clone();
        missing.push(msg(Role::User, "no ts", ""));
        let got = filter_new_messages(&missing, "2026-04-05T00:00:00.000000Z");
        assert_eq!(got.len(), 2);
        assert!(got.iter().any(|m| m.content == "no ts"));
    }

    #[test]
    fn format_transcript_labels_roles() {
        let messages = vec![
            msg(Role::User, "hi", "t1"),
            msg(Role::Assistant, "hello back", "t2"),
        ];
        let out = format_transcript(&messages, "Clara");
        assert_eq!(out, "User: hi\n\nClara: hello back");
    }

    #[test]
    fn chatbot_prompt_omits_persona_memory_when_absent() {
        // The data section is envelope-conditional on `persona_memory`; verify
        // it's omitted when none is supplied. The MERGING-list rule about
        // pre-existing knowledge is now part of the user-editable `.md`
        // (always present), so we don't assert on it here.
        let p = build_chatbot_prompt("Clara", "prev", "User: hi", "INSTRUCTIONS", "", "");
        assert!(!p.contains("--- WHAT YOU ALREADY KNOW ABOUT THIS PERSON ---"));
    }

    #[test]
    fn chatbot_prompt_includes_persona_memory_when_present() {
        let p = build_chatbot_prompt(
            "Clara",
            "prev",
            "User: hi",
            "INSTRUCTIONS",
            "",
            "knows their dog is named Max",
        );
        assert!(p.contains("--- WHAT YOU ALREADY KNOW ABOUT THIS PERSON ---"));
        assert!(p.contains("knows their dog is named Max"));
    }

    #[test]
    fn chatbot_prompt_inlines_loaded_instructions() {
        let p = build_chatbot_prompt("Clara", "prev", "User: hi", "BODY_OF_INSTRUCTIONS", "", "");
        assert!(p.contains("--- INSTRUCTIONS ---\n\nBODY_OF_INSTRUCTIONS"));
    }

    #[test]
    fn roleplay_prompt_uses_roleplay_envelope() {
        let p = build_roleplay_prompt("prev", "messages", "BODY_OF_INSTRUCTIONS", "");
        assert!(p.contains("ROLEPLAY"));
        assert!(p.contains("--- CURRENT SCENE SUMMARY ---"));
        assert!(p.contains("--- INSTRUCTIONS ---\n\nBODY_OF_INSTRUCTIONS"));
        // Roleplay envelope is distinct from chatbot's; the chatbot-specific
        // register paragraph must not leak into the roleplay path.
        assert!(!p.contains("Write in the register"));
    }
}
