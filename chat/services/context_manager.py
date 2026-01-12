import os
import json
from pathlib import Path
from .user_context import load_enabled_context

def load_context(personality_dir, ltm_file="long_term_memory.md"):
    """
    Load context from a specific personality directory.

    Args:
        personality_dir: Path to personality folder (e.g., "personalities/assistant")
        ltm_file: Path to long-term memory file

    Returns:
        Concatenated system prompt string
    """
    context_str = ""

    # Load all .md files from personality directory (alphabetically)
    if os.path.exists(personality_dir):
        for filename in sorted(os.listdir(personality_dir)):
            if filename.endswith(".md"):
                filepath = os.path.join(personality_dir, filename)
                with open(filepath, 'r') as f:
                    context_str += f"--- SYSTEM INSTRUCTION: {filename} ---\n"
                    context_str += f.read() + "\n\n"
    else:
        # Fallback warning if personality not found
        context_str = "--- WARNING: Personality not found ---\n"
        context_str += f"Expected directory: {personality_dir}\n\n"

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


def get_available_personalities(personalities_dir="personalities"):
    """
    Get list of available personality folders.

    Returns:
        List of personality names (folder names)
    """
    if not os.path.exists(personalities_dir):
        return []

    personalities = []
    for item in os.listdir(personalities_dir):
        item_path = os.path.join(personalities_dir, item)
        # Only include directories that contain at least one .md file
        if os.path.isdir(item_path):
            has_context = any(f.endswith(".md") for f in os.listdir(item_path))
            if has_context:
                personalities.append(item)

    return sorted(personalities)


def get_personality_config(personality_name, personalities_dir="personalities"):
    """
    Load config.json from personality directory, if exists.

    Args:
        personality_name: Name of the personality folder
        personalities_dir: Base directory containing personalities

    Returns:
        Dict of config values, or empty dict if no config exists
    """
    config_path = Path(personalities_dir) / personality_name / "config.json"
    if config_path.exists():
        with open(config_path, 'r') as f:
            return json.load(f)
    return {}


def get_personality_model(personality_name, personalities_dir="personalities"):
    """
    Get model override for a personality, or None if not set.

    Args:
        personality_name: Name of the personality folder
        personalities_dir: Base directory containing personalities

    Returns:
        Model string if set, None otherwise
    """
    config = get_personality_config(personality_name, personalities_dir)
    return config.get("model")
