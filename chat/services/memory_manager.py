import os
import shutil
from pathlib import Path

from django.conf import settings as django_settings

from .llm_client import call_llm, LLMError
from .context_manager import get_persona_model


# =============================================================================
# Module-level file I/O functions (no API key needed)
# =============================================================================

def _safe_persona_name(persona_name):
    """Sanitize persona name to prevent path traversal."""
    # Strip any path components — only keep the base name
    safe = os.path.basename(persona_name)
    # Remove anything that isn't alphanumeric or underscore
    safe = ''.join(c for c in safe if c.isalnum() or c == '_')
    return safe or 'assistant'


def get_memory_file(persona_name):
    """Return Path to a persona's memory file, creating directory if needed."""
    memory_dir = django_settings.MEMORY_DIR
    os.makedirs(memory_dir, exist_ok=True)
    return memory_dir / f"{_safe_persona_name(persona_name)}.md"


def get_memory_content(persona_name):
    """Read a persona's memory, returning empty string if not found."""
    filepath = get_memory_file(persona_name)
    if filepath.exists():
        with open(filepath, 'r') as f:
            return f.read()
    return ""


def save_memory_content(persona_name, content):
    """Write memory with atomic write pattern (flush + fsync)."""
    filepath = get_memory_file(persona_name)
    with open(filepath, 'w') as f:
        f.write(content)
        f.flush()
        os.fsync(f.fileno())


def delete_memory(persona_name):
    """Delete a persona's memory file."""
    filepath = get_memory_file(persona_name)
    if filepath.exists():
        os.remove(filepath)


def rename_memory(old_name, new_name):
    """Rename a persona's memory file when persona is renamed."""
    old_path = get_memory_file(old_name)
    if old_path.exists():
        new_path = get_memory_file(new_name)
        shutil.move(str(old_path), str(new_path))


def list_persona_memories():
    """Return list of persona names that have memory files."""
    memory_dir = django_settings.MEMORY_DIR
    if not os.path.exists(memory_dir):
        return []
    return sorted(
        Path(f).stem for f in os.listdir(memory_dir)
        if f.endswith('.md')
    )


def get_memory_model(config, persona_name, personas_dir):
    """Get model for memory generation: MEMORY_MODEL -> persona model -> default."""
    return (
        config.get("MEMORY_MODEL")
        or get_persona_model(persona_name, personas_dir)
        or config.get("MODEL")
    )


# =============================================================================
# MemoryManager class (LLM-dependent operations)
# =============================================================================

class MemoryManager:
    def __init__(self, api_key, model, site_url=None, site_name=None):
        self.api_key = api_key
        self.model = model
        self.site_url = site_url
        self.site_name = site_name

    def _merge_memory(self, persona_name, persona_identity, new_data_label, new_data_content,
                      instructions_opener, size_limit=8000, extra_sections=""):
        """
        Core memory merge: existing memory + new data -> updated memory via LLM.

        Args:
            persona_name: Name of the persona
            persona_identity: Raw identity content from persona .md files
            new_data_label: Section header for the new data (e.g. "RECENT CONVERSATIONS")
            new_data_content: The new data text to merge in
            instructions_opener: First sentence(s) of the instructions block
            size_limit: Target character count (0 = unlimited)
            extra_sections: Additional prompt sections (e.g. roleplay awareness)

        Returns:
            True if memory was updated, False on failure
        """
        existing_memory = get_memory_content(persona_name)
        persona_display_name = persona_name.replace('_', ' ').title()

        size_instruction = ""
        if size_limit and size_limit > 0:
            size_instruction = (
                f"SIZE TARGET: Aim for roughly {size_limit} characters. You can go over rather than\n"
                "lose something important, but consolidate where you can. Quality over quantity.\n\n"
            )

        prompt = (
            f"You are {persona_display_name}. Below is your identity — who you are, how you\n"
            "think, how you talk.\n\n"

            "--- YOUR IDENTITY ---\n"
            f"{persona_identity}\n\n"

            "--- YOUR EXISTING MEMORY ABOUT THE USER ---\n"
            f"{existing_memory if existing_memory else 'You do not have any memories yet. This is the beginning.'}\n\n"

            f"--- {new_data_label} ---\n"
            f"{new_data_content}\n\n"

            "--- INSTRUCTIONS ---\n\n"

            f"{instructions_opener}\n\n"

            "This is not a clinical profile. It's what stuck. The things worth holding onto.\n"
            "Write with your personality, your observations, your feelings about what matters.\n\n"

            "MERGING RULES:\n"
            "- READ your existing memory carefully. Most of it should survive.\n"
            "- ADD new details, observations, and developments from the new information.\n"
            "- REVISE entries that have been updated or corrected (e.g., they got a new job,\n"
            "  changed an opinion, finished a project).\n"
            "- COMPRESS patterns: if something has come up many times, consolidate it into\n"
            "  a confident observation rather than listing each instance.\n"
            "- LET STALE DETAILS FADE: if something minor hasn't come up in a while and\n"
            "  isn't anchored by emotional weight, it's okay to drop it.\n"
            "- KEEP VIVID ANCHORS: specific quotes, memorable moments, things said with\n"
            "  emotional weight — these survive even if old.\n"
            "- NEVER remove core identity facts (name, family, career, values) unless\n"
            "  explicitly contradicted.\n\n"

            "SECTIONS:\n"
            "Use markdown ## headers for each section. Let sections emerge organically from what\n"
            "you know about this person. Don't force a rigid template. Some natural sections\n"
            "might include things like:\n"
            "- How you two work together / your dynamic\n"
            "- What's going on in their life\n"
            "- Patterns you've noticed about them\n"
            "- Things they've said that stuck with you\n"
            "- People in their life\n"
            "- Ongoing threads you're tracking\n\n"

            "But these are suggestions, not requirements. Use whatever sections feel right for\n"
            "what you actually know. If this is the first memory, start with what you learned.\n"
            "If you've been talking a while, the structure will reflect the depth.\n\n"

            f"{extra_sections}"

            "FORMAT:\n"
            "- Write in standard, properly capitalized prose and markdown, using ## headers\n"
            "  for sections. Do NOT adopt the persona's speaking style for the memory itself.\n"
            "- Second person addressed to yourself (\"You've noticed...\", \"You two talk about...\",\n"
            "  \"They told you...\", \"You feel like...\")\n"
            "- Be specific — names, details, quotes, not vague summaries\n"
            "- No timestamps or meta-commentary about the update process\n"
            "- No bullet-point databases — write like a person remembering, not a system logging\n\n"

            f"{size_instruction}"

            "Return ONLY the updated memory content. No preamble, no explanation."
        )

        try:
            updated_memory = call_llm(
                self.api_key, self.model,
                [{"role": "user", "content": prompt}],
                site_url=self.site_url, site_name=self.site_name,
                timeout=600
            )

            # Safety check: don't replace substantial memory with suspiciously short output
            if len(updated_memory) < 10 and len(existing_memory) > 50:
                return False

            save_memory_content(persona_name, updated_memory)
            return True

        except Exception as e:
            print(f"Error updating memory for {persona_name}: {e}")
            return False

    def update_persona_memory(self, persona_name, persona_identity, threads, size_limit=8000):
        """
        Incremental merge: read existing memory + new conversation threads -> updated memory.

        Args:
            persona_name: Name of the persona
            persona_identity: Raw identity content from persona .md files
            threads: List of thread dicts with 'title', 'persona', 'messages' keys
            size_limit: Target character count (0 = unlimited)

        Returns:
            True if memory was updated, False on failure
        """
        if not threads:
            return False

        persona_display_name = persona_name.replace('_', ' ').title()

        # Format threads as full conversations (both sides)
        threads_text = ""
        for i, thread in enumerate(threads, 1):
            title = thread.get("title", "Untitled")
            messages = thread.get("messages", [])

            if messages:
                threads_text += f"=== THREAD {i}: {title} ===\n"
                for msg in messages:
                    role_label = "User" if msg.get('role') == 'user' else persona_display_name
                    threads_text += f"{role_label}: {msg.get('content', '')}\n"
                threads_text += "\n"

        roleplay_section = (
            "ROLEPLAY AWARENESS:\n"
            "Some conversations may be roleplay or creative writing. Signs include: the persona\n"
            "name suggests a character, thread titles suggest fiction, messages are written in\n"
            "character. For roleplay threads:\n"
            "- Do NOT extract character traits as real user traits\n"
            "- Instead, note what kind of stories/scenarios they enjoy\n"
            "- The creative interests are real even if the content is fictional\n\n"
        )

        return self._merge_memory(
            persona_name, persona_identity,
            new_data_label="RECENT CONVERSATIONS",
            new_data_content=threads_text,
            instructions_opener=(
                "You are updating your personal memory about the user you talk to. This memory\n"
                "is written in second person — addressed to you, as things you know, feel, and\n"
                "have observed. When you read it back, it becomes your own inner knowledge."
            ),
            size_limit=size_limit,
            extra_sections=roleplay_section,
        )

    def seed_memory(self, persona_name, persona_identity, seed_content, size_limit=8000):
        """
        Merge uploaded file content into a persona's existing memory.

        The seed content is woven into the memory organically — not appended
        verbatim. The file is not saved; this is a one-time injection.

        Args:
            persona_name: Name of the persona
            persona_identity: Raw identity content from persona .md files
            seed_content: Text content from the uploaded file
            size_limit: Target character count (0 = unlimited)

        Returns:
            True if memory was updated, False on failure
        """
        return self._merge_memory(
            persona_name, persona_identity,
            new_data_label="NEW INFORMATION FROM THE USER",
            new_data_content=seed_content,
            instructions_opener=(
                "You are updating your personal memory about the user you talk to. They've provided\n"
                "additional information they want you to know. This memory is written in second\n"
                "person — addressed to you, as things you know, feel, and have observed. When you\n"
                "read it back, it becomes your own inner knowledge."
            ),
            size_limit=size_limit,
        )

    def modify_memory_with_command(self, persona_name, persona_identity, command, size_limit=8000):
        """
        Apply a user's natural language command to modify a persona's memory.

        Args:
            persona_name: Name of the persona
            persona_identity: Raw identity content from persona .md files
            command: User's instruction (e.g., "Forget my brother Tom's name")
            size_limit: Target character count (0 = unlimited)

        Returns:
            True if memory was updated, False on failure
        """
        if not get_memory_content(persona_name):
            return False

        return self._merge_memory(
            persona_name, persona_identity,
            new_data_label="USER'S COMMAND",
            new_data_content=command,
            instructions_opener=(
                "The user has asked you to modify your memory. Apply their request. If they ask\n"
                "to forget something, remove it. If they ask to add or change something, do so.\n"
                "This memory is written in second person — addressed to you, as things you know,\n"
                "feel, and have observed. When you read it back, it becomes your own inner knowledge."
            ),
            size_limit=size_limit,
        )
