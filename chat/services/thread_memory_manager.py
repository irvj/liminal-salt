"""
ThreadMemoryManager service — per-session summary of what has happened
in a single thread, maintained by LLM merge.

Storage lives inline on the session JSON (fields `thread_memory` and
`thread_memory_updated_at`). The manager owns the LLM-driven merge that
incorporates new messages into the existing summary.

Distinct from persona memory (MemoryManager) which is cross-thread and
written in the persona's voice about the user.
"""
import logging

from .llm_client import call_llm, LLMError

logger = logging.getLogger(__name__)


DEFAULT_THREAD_MEMORY_SIZE = 4000
# 0 = auto-update disabled by default. Matches persona memory's
# `auto_memory_interval` default — users opt in explicitly via the UI.
DEFAULT_THREAD_MEMORY_INTERVAL_MINUTES = 0
DEFAULT_THREAD_MEMORY_MESSAGE_FLOOR = 4


def resolve_persona_thread_memory_defaults(persona_config):
    """
    Return the persona's effective thread-memory defaults: persona config
    values merged with global fallback. Used by the persona settings UI
    to prefill form fields.
    """
    result = {
        'interval_minutes': DEFAULT_THREAD_MEMORY_INTERVAL_MINUTES,
        'message_floor': DEFAULT_THREAD_MEMORY_MESSAGE_FLOOR,
        'size_limit': DEFAULT_THREAD_MEMORY_SIZE,
    }
    persona_defaults = (persona_config or {}).get('default_thread_memory_settings') or {}
    for key in list(result.keys()):
        if key in persona_defaults:
            result[key] = persona_defaults[key]
    return result


def resolve_thread_memory_settings(session_data, persona_config):
    """
    Merge effective thread-memory settings: per-thread override (in session
    JSON under `thread_memory_settings`) → persona default (in persona
    config under `default_thread_memory_settings`) → global fallback.

    Returns a dict with `interval_minutes`, `message_floor`, `size_limit`.
    """
    result = {
        'interval_minutes': DEFAULT_THREAD_MEMORY_INTERVAL_MINUTES,
        'message_floor': DEFAULT_THREAD_MEMORY_MESSAGE_FLOOR,
        'size_limit': DEFAULT_THREAD_MEMORY_SIZE,
    }

    persona_default = (persona_config or {}).get('default_thread_memory_settings') or {}
    thread_override = (session_data or {}).get('thread_memory_settings') or {}

    for key in list(result.keys()):
        if key in persona_default:
            result[key] = persona_default[key]
        if key in thread_override:
            result[key] = thread_override[key]

    return result


def filter_new_messages(messages, updated_at):
    """
    Return messages that haven't been merged into the current summary.

    If `updated_at` is empty, all messages are considered new (first run).
    Otherwise, only messages with a timestamp strictly greater than
    `updated_at` are returned. A message missing a timestamp is anomalous
    (every write path sets one) — include it and log, rather than dropping
    content silently.
    """
    if not updated_at:
        return list(messages)
    result = []
    for m in messages:
        ts = m.get('timestamp', '')
        if not ts:
            logger.warning("filter_new_messages: message without timestamp, including as new")
            result.append(m)
        elif ts > updated_at:
            result.append(m)
    return result


def _format_messages_for_prompt(messages, persona_display_name):
    """Render messages as a plain transcript for the summarizer."""
    lines = []
    for msg in messages:
        role = msg.get('role')
        content = msg.get('content', '')
        if role == 'user':
            lines.append(f"User: {content}")
        else:
            lines.append(f"{persona_display_name}: {content}")
    return "\n\n".join(lines)


class ThreadMemoryManager:
    def __init__(self, api_key, model):
        self.api_key = api_key
        self.model = model

    def merge(self, persona_display_name, existing_memory, new_messages,
              size_limit=DEFAULT_THREAD_MEMORY_SIZE, mode="chatbot"):
        """
        Merge new messages into the existing thread summary via LLM.

        Args:
            persona_display_name: Pretty name of the persona (for role labels)
            existing_memory: Current thread memory string (may be empty)
            new_messages: List of message dicts (each with role/content/timestamp)
            size_limit: Target character count (0 = unlimited)
            mode: "chatbot" or "roleplay" — selects prompt variant

        Returns:
            Updated memory string on success, None on failure.
        """
        if not new_messages:
            return None

        transcript = _format_messages_for_prompt(new_messages, persona_display_name)

        size_instruction = ""
        if size_limit and size_limit > 0:
            size_instruction = (
                f"SIZE TARGET: Aim for roughly {size_limit} characters. Go over when\n"
                "the alternative is losing events — losing a topic entirely is never\n"
                "the right trade. If the memory won't fit, compress detail (the dish\n"
                "becomes \"steak and potatoes\"), don't drop what happened.\n\n"
            )

        existing_block = (
            existing_memory
            if existing_memory
            else "No summary yet. This is the start of the thread."
        )

        if mode == "roleplay":
            prompt = self._build_roleplay_prompt(
                persona_display_name, existing_block, transcript, size_instruction,
            )
        else:
            prompt = self._build_chatbot_prompt(
                persona_display_name, existing_block, transcript, size_instruction,
            )

        try:
            updated = call_llm(
                self.api_key, self.model,
                [{"role": "user", "content": prompt}],
                timeout=600,
            )

            if len(updated) < 10 and len(existing_memory) > 50:
                # Safety: reject suspiciously short replacements of substantial memory
                return None

            return updated

        except LLMError as e:
            logger.error(f"Thread memory merge failed: {e}")
            return None

    def _build_chatbot_prompt(self, persona_display_name, existing_block,
                              transcript, size_instruction):
        return (
            f"You are {persona_display_name}. Below is your working memory of a\n"
            "conversation thread with a user — what you remember of everything said\n"
            "so far, the way you'd remember a long catch-up with a friend the next\n"
            "day. You don't recall verbatim. You remember what they told you: the new\n"
            "job, the move they're planning, the story about their commute. The dish\n"
            "they ordered compresses to \"they had steak and potatoes\" — not the menu,\n"
            "the memory. The fact that they told you something stays. The exact\n"
            "wording doesn't.\n\n"

            "--- CURRENT THREAD SUMMARY ---\n"
            f"{existing_block}\n\n"

            "--- NEW MESSAGES (since last update) ---\n"
            f"{transcript}\n\n"

            "--- INSTRUCTIONS ---\n\n"

            "Update the summary so it reflects everything that has happened through\n"
            "the new messages. This is your working memory of the WHOLE conversation,\n"
            "not just the most recent exchanges. If someone asked you tomorrow \"did\n"
            "they mention X?\" for something that came up early in the thread, you\n"
            "should still remember it.\n\n"

            "MERGING:\n"
            "- The existing summary IS your memory of the thread so far. Treat it\n"
            "  as canonical and load-bearing. Every event already captured there\n"
            "  must still be represented in the updated summary — the new messages\n"
            "  add to that memory, they don't replace it.\n"
            "- MERGE the new messages into the existing summary; don't rewrite from\n"
            "  scratch.\n"
            "- ABSTRACT toward essence, don't drop events. \"They told a long story\n"
            "  about their commute\" is fine; dropping that they talked about the\n"
            "  commute at all is not. The goal isn't a shorter summary — it's a\n"
            "  memory.\n"
            "- PRESERVE the whole arc: the start, key turns, what got established\n"
            "  along the way, not only the latest exchanges. An early moment that\n"
            "  established something meaningful is as load-bearing as a recent one\n"
            "  — often more so, because it's had time to shape everything since.\n"
            "- BIAS HISTORICAL, NOT RECENT. When size pressure forces compression,\n"
            "  compress the new content first. Recent events haven't yet earned the\n"
            "  weight of events that have already survived into the summary; don't\n"
            "  let fresh detail crowd out what's established.\n"
            "- DETAIL settles to the level of natural memory. Exact quotes, long\n"
            "  verbatim passages, verbose descriptions → the gist. The gist of\n"
            "  every significant topic stays.\n"
            "- IF the existing summary is written in a different voice (e.g.\n"
            "  third-person narrator, \"the user discussed X with...\"), rewrite it\n"
            "  into the perspective below as you merge. The voice should be\n"
            "  consistent across the whole summary.\n\n"

            "PERSPECTIVE — apply to every sentence:\n"
            "- ALWAYS \"you\" for yourself: \"You walked them through...\", \"You agreed\n"
            "  to...\", \"You noticed he...\"\n"
            "- NEVER \"I\": not \"I explained...\", not \"I noticed...\"\n"
            "- ALWAYS third person for the user: \"he\", \"she\", \"they\" — infer from\n"
            "  context, default to \"they\" if unclear.\n"
            "- AVOID \"the user\" as a label; refer to them like a person whose\n"
            "  conversation you remember.\n\n"

            "FORMAT:\n"
            "- Write in standard prose with proper capitalization and punctuation,\n"
            "  regardless of your conversational style elsewhere. This is memory,\n"
            "  not dialogue.\n"
            "- No bullet-point log, no transcript, no timestamps, no meta-commentary\n"
            "  about the update process.\n\n"

            f"{size_instruction}"

            "Return ONLY the updated summary. No preamble, no explanation."
        )

    def _build_roleplay_prompt(self, persona_display_name, existing_block,
                               transcript, size_instruction):
        return (
            "You are maintaining a working memory of a ROLEPLAY thread — the whole\n"
            "scene as you'd remember a film or a chapter you finished reading. You\n"
            "don't replay every line. You remember what happened: where characters\n"
            "went, what they did and said to each other, what shifted between them,\n"
            "the beats that mattered. Exact dialogue compresses to the moment it\n"
            "captured. The arc stays; the word-for-word doesn't.\n\n"

            "--- CURRENT SCENE SUMMARY ---\n"
            f"{existing_block}\n\n"

            "--- NEW MESSAGES (since last update) ---\n"
            f"{transcript}\n\n"

            "--- INSTRUCTIONS ---\n\n"

            "Update the summary so it reflects everything that has happened through\n"
            "the new messages. This is a memory of the WHOLE scene so far, not only\n"
            "the most recent beats.\n\n"

            "MERGING:\n"
            "- The existing summary IS your memory of the scene so far. Treat it\n"
            "  as canonical and load-bearing. Every beat already captured there\n"
            "  must still be represented in the updated summary — the new messages\n"
            "  add to that memory, they don't replace it.\n"
            "- MERGE the new events into the existing summary; don't rewrite from\n"
            "  scratch.\n"
            "- ABSTRACT toward essence, don't drop events. A long back-and-forth\n"
            "  compresses to \"they argued about X and he finally agreed to Y\" —\n"
            "  that's memory. Dropping that the argument happened at all isn't.\n"
            "- PRESERVE the whole arc: where the scene opened, what got established,\n"
            "  the turns along the way, not just the latest beats. An early moment\n"
            "  that set the emotional stakes is as load-bearing as a late one.\n"
            "- BIAS HISTORICAL, NOT RECENT. When size pressure forces compression,\n"
            "  compress the new content first. Recent beats haven't yet earned the\n"
            "  weight of beats that have already survived into the summary; don't\n"
            "  let fresh detail crowd out what's established.\n"
            "- TRACK plot threads, promises made, secrets revealed, relationship\n"
            "  shifts — these define the scene and need to survive.\n"
            "- KEEP vivid anchors when they carry the scene: a line of dialogue\n"
            "  that turned things, a sensory detail that defined a place. Use them\n"
            "  sparingly — memory, not transcript.\n"
            "- USE character names (not \"the user\" and not the persona's raw name\n"
            "  if a character name is clear from context). Write in third-person\n"
            "  narrative prose, past tense. Not a script, not a log.\n"
            "- DO NOT extract the real user's biographical facts; this is fiction.\n"
            "- AVOID meta-commentary about the update process.\n\n"

            f"{size_instruction}"

            "Return ONLY the updated summary. No preamble, no explanation."
        )
