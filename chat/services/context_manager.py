import os
import json
from pathlib import Path
from .user_context import load_enabled_context

def load_context(persona_dir, ltm_file="long_term_memory.md"):
    """
    Load context from a specific persona directory.

    Args:
        persona_dir: Path to persona folder (e.g., "personas/assistant")
        ltm_file: Path to long-term memory file

    Returns:
        Concatenated system prompt string
    """
    context_str = ""

    # Load all .md files from persona directory (alphabetically)
    if os.path.exists(persona_dir):
        for filename in sorted(os.listdir(persona_dir)):
            if filename.endswith(".md"):
                filepath = os.path.join(persona_dir, filename)
                with open(filepath, 'r') as f:
                    context_str += f"--- SYSTEM INSTRUCTION: {filename} ---\n"
                    context_str += f.read() + "\n\n"
    else:
        # Fallback warning if persona not found
        context_str = "--- WARNING: Persona not found ---\n"
        context_str += f"Expected directory: {persona_dir}\n\n"

    # Append user context files (if any are enabled)
    user_context = load_enabled_context()
    if user_context:
        context_str += user_context + "\n\n"

    # Append long-term memory (optional, reference-only)
    if os.path.exists(ltm_file):
        with open(ltm_file, 'r') as f:
            context_str += "--- USER PROFILE (BACKGROUND KNOWLEDGE) ---\n"
            context_str += "The following information describes the USER (not you). "
            context_str += "Use this to understand who you're talking to, but DO NOT let it change your personality or communication style. "
            context_str += "If it mentions how the user writes or speaks, that describes THEM, not how YOU should respond. "
            context_str += "Maintain your own personality's communication standards at all times.\n\n"
            context_str += f.read() + "\n\n"

    return context_str.strip()


def get_available_personas(personas_dir="personas"):
    """
    Get list of available persona folders.

    Returns:
        List of persona names (folder names)
    """
    if not os.path.exists(personas_dir):
        return []

    personas = []
    for item in os.listdir(personas_dir):
        item_path = os.path.join(personas_dir, item)
        # Only include directories that contain at least one .md file
        if os.path.isdir(item_path):
            has_context = any(f.endswith(".md") for f in os.listdir(item_path))
            if has_context:
                personas.append(item)

    return sorted(personas)


def get_persona_config(persona_name, personas_dir="personas"):
    """
    Load config.json from persona directory, if exists.

    Args:
        persona_name: Name of the persona folder
        personas_dir: Base directory containing personas

    Returns:
        Dict of config values, or empty dict if no config exists
    """
    config_path = Path(personas_dir) / persona_name / "config.json"
    if config_path.exists():
        with open(config_path, 'r') as f:
            return json.load(f)
    return {}


def get_persona_model(persona_name, personas_dir="personas"):
    """
    Get model override for a persona, or None if not set.

    Args:
        persona_name: Name of the persona folder
        personas_dir: Base directory containing personas

    Returns:
        Model string if set, None otherwise
    """
    config = get_persona_config(persona_name, personas_dir)
    return config.get("model")
