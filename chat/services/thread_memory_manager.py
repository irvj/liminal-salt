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


def filter_new_messages(messages, updated_at):
    """
    Return messages that haven't been merged into the current summary.

    If `updated_at` is empty, all messages are considered new (first run).
    Otherwise, only messages with a timestamp strictly greater than
    `updated_at` are returned. Messages missing a timestamp are skipped
    on non-first runs (conservative — avoid re-summarizing old content).
    """
    if not updated_at:
        return list(messages)
    return [m for m in messages if m.get('timestamp', '') > updated_at]


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
            "You are maintaining a running summary of a conversation thread between a user\n"
            f"and {persona_display_name}. The summary captures what has happened in THIS\n"
            "specific thread — decisions made, topics discussed, state changes, emotional\n"
            "beats, commitments, promises, and any context worth carrying forward as the\n"
            "conversation grows. It is not a log of every exchange; it is the narrative\n"
            "shape of the thread.\n\n"

            "--- CURRENT THREAD SUMMARY ---\n"
            f"{existing_block}\n\n"

            "--- NEW MESSAGES (since last update) ---\n"
            f"{transcript}\n\n"

            "--- INSTRUCTIONS ---\n\n"

            "Produce an updated summary that:\n"
            "- MERGES the new events into the existing summary; don't rewrite from scratch.\n"
            "- COMPRESSES repeated or similar events into single observations.\n"
            "- PRESERVES specific details: names, decisions, key quotes, turning points,\n"
            "  anything that changes the trajectory.\n"
            "- LETS minor or superseded details fade as the thread progresses.\n"
            "- IS written in neutral narrative prose using standard capitalization and\n"
            "  punctuation. Not bullet-point logs, not a transcript.\n"
            f"- REFERS to participants as \"the user\" and \"{persona_display_name}\" (or\n"
            "  character names if clear from the conversation).\n"
            "- AVOIDS meta-commentary about the update process or timestamps.\n\n"

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
