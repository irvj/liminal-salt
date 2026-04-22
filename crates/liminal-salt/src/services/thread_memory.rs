//! Per-session running summary — the "working memory" of a single thread,
//! maintained by LLM merge. Distinct from persona memory: thread memory lives
//! inline on the session JSON (`thread_memory`, `thread_memory_updated_at`),
//! is bounded by the session's lifetime, and is written in the persona's voice
//! about what happened in *this* thread only.
//!
//! This module returns the merged summary text; actually persisting it on the
//! session goes through `session::save_thread_memory` so the session's per-id
//! lock coordinates with other writers.

use crate::services::{
    llm::{ChatLlm, LlmMessage},
    persona::PersonaConfig,
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

/// Merge new messages into the existing thread summary via LLM. Returns the
/// updated summary on success, or `None` on LLM error / safety rejection.
///
/// `persona_memory` is the cross-thread persona memory used to color the
/// chatbot-variant merge (suppressed in roleplay for immersion). Pass `""`
/// if roleplay or if no memory file exists.
pub async fn merge<L: ChatLlm>(
    llm: &L,
    persona_display_name: &str,
    existing_memory: &str,
    new_messages: &[Message],
    size_limit: u32,
    mode: Mode,
    persona_memory: &str,
) -> Option<String> {
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

    let prompt = match mode {
        Mode::Roleplay => build_roleplay_prompt(&existing_block, &transcript, &size_instruction),
        Mode::Chatbot => build_chatbot_prompt(
            persona_display_name,
            &existing_block,
            &transcript,
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
    size_instruction: &str,
    persona_memory: &str,
) -> String {
    let trimmed_memory = persona_memory.trim();
    let (persona_memory_block, persona_memory_rule) = if trimmed_memory.is_empty() {
        (String::new(), "")
    } else {
        (
            format!(
                "--- WHAT YOU ALREADY KNOW ABOUT THIS PERSON ---\n\
                 {trimmed_memory}\n\n\
                 NOTE: The section above is your long-running memory about this\n\
                 person, carried in from other conversations. It tells you who they\n\
                 are to you — use it as the lens through which you read this thread.\n\
                 DO NOT copy facts from it into the summary below. The summary is a\n\
                 record of THIS thread only; what you already knew about them lives\n\
                 elsewhere and doesn't need repeating here.\n\n"
            ),
            "- DO NOT merge pre-existing knowledge about this person into the\n  \
             summary. Anything from \"WHAT YOU ALREADY KNOW\" above is background\n  \
             that colors how you read the thread — it is not content to summarize.\n  \
             The summary covers ONLY what happened in THIS thread.\n",
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
         Update the summary so it reflects everything that has happened through\n\
         the new messages. This is your working memory of the WHOLE conversation,\n\
         not just the most recent exchanges. If someone asked you tomorrow \"did\n\
         they mention X?\" for something that came up early in the thread, you\n\
         should still remember it.\n\n\
         MERGING:\n\
         - The existing summary IS your memory of the thread so far. Treat it\n  \
         as canonical and load-bearing. Every event already captured there\n  \
         must still be represented in the updated summary — the new messages\n  \
         add to that memory, they don't replace it.\n\
         - MERGE the new messages into the existing summary; don't rewrite from\n  \
         scratch.\n\
         {persona_memory_rule}\
         - ABSTRACT toward essence, don't drop events. \"They told a long story\n  \
         about their commute\" is fine; dropping that they talked about the\n  \
         commute at all is not. The goal isn't a shorter summary — it's a\n  \
         memory.\n\
         - PRESERVE the whole arc: the start, key turns, what got established\n  \
         along the way, not only the latest exchanges. An early moment that\n  \
         established something meaningful is as load-bearing as a recent one\n  \
         — often more so, because it's had time to shape everything since.\n\
         - BIAS HISTORICAL, NOT RECENT. When size pressure forces compression,\n  \
         compress the new content first. Recent events haven't yet earned the\n  \
         weight of events that have already survived into the summary; don't\n  \
         let fresh detail crowd out what's established.\n\
         - DETAIL settles to the level of natural memory. Exact quotes, long\n  \
         verbatim passages, verbose descriptions → the gist. The gist of\n  \
         every significant topic stays.\n\
         - IF the existing summary is written in a different voice (e.g.\n  \
         third-person narrator, \"the user discussed X with...\"), rewrite it\n  \
         into the perspective below as you merge. The voice should be\n  \
         consistent across the whole summary.\n\n\
         PERSPECTIVE — apply to every sentence:\n\
         - ALWAYS \"you\" for yourself: \"You walked them through...\", \"You agreed\n  \
         to...\", \"You noticed he...\"\n\
         - NEVER \"I\": not \"I explained...\", not \"I noticed...\"\n\
         - ALWAYS third person for the user: \"he\", \"she\", \"they\" — infer from\n  \
         context, default to \"they\" if unclear.\n\
         - AVOID \"the user\" as a label; refer to them like a person whose\n  \
         conversation you remember.\n\n\
         FORMAT:\n\
         - Write in standard prose with proper capitalization and punctuation,\n  \
         regardless of your conversational style elsewhere. This is memory,\n  \
         not dialogue.\n\
         - No bullet-point log, no transcript, no timestamps, no meta-commentary\n  \
         about the update process.\n\n\
         {size_instruction}\
         Return ONLY the updated summary. No preamble, no explanation."
    )
}

fn build_roleplay_prompt(
    existing_block: &str,
    transcript: &str,
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
         Update the summary so it reflects everything that has happened through\n\
         the new messages. This is a memory of the WHOLE scene so far, not only\n\
         the most recent beats.\n\n\
         MERGING:\n\
         - The existing summary IS your memory of the scene so far. Treat it\n  \
         as canonical and load-bearing. Every beat already captured there\n  \
         must still be represented in the updated summary — the new messages\n  \
         add to that memory, they don't replace it.\n\
         - MERGE the new events into the existing summary; don't rewrite from\n  \
         scratch.\n\
         - ABSTRACT toward essence, don't drop events. A long back-and-forth\n  \
         compresses to \"they argued about X and he finally agreed to Y\" —\n  \
         that's memory. Dropping that the argument happened at all isn't.\n\
         - PRESERVE the whole arc: where the scene opened, what got established,\n  \
         the turns along the way, not just the latest beats. An early moment\n  \
         that set the emotional stakes is as load-bearing as a late one.\n\
         - BIAS HISTORICAL, NOT RECENT. When size pressure forces compression,\n  \
         compress the new content first. Recent beats haven't yet earned the\n  \
         weight of beats that have already survived into the summary; don't\n  \
         let fresh detail crowd out what's established.\n\
         - TRACK plot threads, promises made, secrets revealed, relationship\n  \
         shifts — these define the scene and need to survive.\n\
         - KEEP vivid anchors when they carry the scene: a line of dialogue\n  \
         that turned things, a sensory detail that defined a place. Use them\n  \
         sparingly — memory, not transcript.\n\
         - USE character names (not \"the user\" and not the persona's raw name\n  \
         if a character name is clear from context). Write in third-person\n  \
         narrative prose, past tense. Not a script, not a log.\n\
         - DO NOT extract the real user's biographical facts; this is fiction.\n\
         - AVOID meta-commentary about the update process.\n\n\
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
        let p = build_chatbot_prompt("Clara", "prev", "User: hi", "", "");
        assert!(!p.contains("WHAT YOU ALREADY KNOW"));
        assert!(!p.contains("DO NOT merge pre-existing knowledge"));
    }

    #[test]
    fn chatbot_prompt_includes_persona_memory_when_present() {
        let p = build_chatbot_prompt("Clara", "prev", "User: hi", "", "knows their dog is named Max");
        assert!(p.contains("WHAT YOU ALREADY KNOW"));
        assert!(p.contains("knows their dog is named Max"));
        assert!(p.contains("DO NOT merge pre-existing knowledge"));
    }

    #[test]
    fn roleplay_prompt_omits_perspective_rules() {
        // Roleplay prompt is narrative third-person; should not contain the
        // chatbot-specific "YOU" perspective check.
        let p = build_roleplay_prompt("prev", "messages", "");
        assert!(p.contains("ROLEPLAY"));
        assert!(!p.contains("PERSPECTIVE — apply to every sentence"));
    }
}
