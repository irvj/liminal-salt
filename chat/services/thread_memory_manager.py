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
    def __init__(self, api_key, model, site_url=None, site_name=None):
        self.api_key = api_key
        self.model = model
        self.site_url = site_url
        self.site_name = site_name

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
                f"SIZE TARGET: Aim for roughly {size_limit} characters. Go over to preserve\n"
                "important events rather than lose them. Consolidate where you can.\n\n"
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
                site_url=self.site_url, site_name=self.site_name,
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
            f"You are {persona_display_name}. Below is your running summary of a\n"
            "conversation thread with a user — what has happened in THIS specific thread\n"
            "so far. It's your inner continuity for this conversation: decisions reached,\n"
            "topics covered, state shifts, commitments made, emotional beats, anything\n"
            "worth carrying forward as the thread grows. Not a log of every exchange —\n"
            "the shape of the thread, in your own head.\n\n"

            "--- CURRENT THREAD SUMMARY ---\n"
            f"{existing_block}\n\n"

            "--- NEW MESSAGES (since last update) ---\n"
            f"{transcript}\n\n"

            "--- INSTRUCTIONS ---\n\n"

            "Update the summary so it reflects everything that has happened through the\n"
            "new messages. This is YOUR inner continuity — notes you keep for yourself\n"
            "about this particular conversation.\n\n"

            "MERGING:\n"
            "- MERGE the new events into the existing summary; don't rewrite from scratch.\n"
            "- COMPRESS repeated or similar exchanges into a single observation.\n"
            "- PRESERVE specifics that matter: decisions, turning points, quotes that\n"
            "  stuck, commitments, problems worked through.\n"
            "- LET minor or superseded details fade as the thread moves on.\n"
            "- IF the existing summary is written in a different voice (e.g. third-person\n"
            "  narrator, \"the user discussed X with...\"), rewrite it into the perspective\n"
            "  below as you merge. The voice should be consistent across the whole summary.\n\n"

            "PERSPECTIVE — apply to every sentence:\n"
            "- ALWAYS \"you\" for yourself: \"You walked them through...\", \"You agreed to...\",\n"
            "  \"You noticed he...\"\n"
            "- NEVER \"I\": not \"I explained...\", not \"I noticed...\"\n"
            "- ALWAYS third person for the user: \"he\", \"she\", \"they\" — infer from\n"
            "  context, default to \"they\" if unclear.\n"
            "- AVOID \"the user\" as a label; refer to them like a person whose conversation\n"
            "  you remember.\n\n"

            "FORMAT:\n"
            "- Write in standard prose with proper capitalization and punctuation,\n"
            "  regardless of your conversational style elsewhere. This is memory, not\n"
            "  dialogue.\n"
            "- No bullet-point log, no transcript, no timestamps, no meta-commentary\n"
            "  about the update process.\n\n"

            f"{size_instruction}"

            "Return ONLY the updated summary. No preamble, no explanation."
        )

    def _build_roleplay_prompt(self, persona_display_name, existing_block,
                               transcript, size_instruction):
        return (
            "You are maintaining a running summary of a ROLEPLAY thread. Treat the\n"
            "exchange as fiction — a scene unfolding between characters. Your job is\n"
            "to preserve scene state, character positions, emotional beats, plot\n"
            "threads, unresolved tensions, and anything that needs to survive into\n"
            "future scenes as the rolling window trims older messages.\n\n"

            "--- CURRENT SCENE SUMMARY ---\n"
            f"{existing_block}\n\n"

            "--- NEW MESSAGES (since last update) ---\n"
            f"{transcript}\n\n"

            "--- INSTRUCTIONS ---\n\n"

            "Produce an updated summary that:\n"
            "- MERGES the new events into the existing summary; don't rewrite from scratch.\n"
            "- PRESERVES narrative continuity: where characters are, what they're doing,\n"
            "  what they're feeling, what was said, what was implied.\n"
            "- TRACKS plot threads, promises made, secrets revealed, relationship shifts.\n"
            "- KEEPS vivid moments: specific lines of dialogue, sensory details, emotional\n"
            "  turning points — these anchor the scene and shouldn't be flattened out.\n"
            "- COMPRESSES filler: long back-and-forth that doesn't advance the scene can\n"
            "  be summarized in a sentence.\n"
            "- LETS minor details fade once they're no longer relevant.\n"
            "- USES character names (not \"the user\" and not the persona's raw name if a\n"
            "  character name is clear from context) and writes in third-person narrative\n"
            "  prose, past tense. Not a script, not a log.\n"
            "- DOES NOT extract the real user's biographical facts; this is fiction.\n"
            "- AVOIDS meta-commentary about the update process.\n\n"

            f"{size_instruction}"

            "Return ONLY the updated summary. No preamble, no explanation."
        )
