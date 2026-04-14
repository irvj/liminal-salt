import os
import json
import logging
import shutil
from pathlib import Path
from .user_context import load_enabled_context
from .persona_context import load_enabled_context as load_enabled_persona_context

logger = logging.getLogger(__name__)

DEFAULT_PERSONAS_DIR = Path(__file__).resolve().parent.parent / 'default_personas'


def ensure_default_personas(personas_dir):
    """
    Copy bundled default personas into the data personas directory
    if they don't already exist. Called on app startup.
    """
    personas_dir = Path(personas_dir)
    personas_dir.mkdir(parents=True, exist_ok=True)

    if not DEFAULT_PERSONAS_DIR.exists():
        return

    for persona in DEFAULT_PERSONAS_DIR.iterdir():
        if not persona.is_dir():
            continue
        target = personas_dir / persona.name
        if not target.exists():
            shutil.copytree(persona, target)
            logger.info(f"Seeded default persona: {persona.name}")

def load_context(persona_dir, persona_name=None):
    """
    Load context from a specific persona directory.

    Args:
        persona_dir: Path to persona folder (e.g., "personas/assistant")
        persona_name: Persona name for loading per-persona memory

    Returns:
        Concatenated system prompt string
    """
    from .memory_manager import get_memory_content

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

    # Append persona-specific context files (if any are enabled)
    resolved_persona = persona_name or os.path.basename(persona_dir)
    persona_context = load_enabled_persona_context(resolved_persona)
    if persona_context:
        context_str += persona_context + "\n\n"

    # Append global user context files (if any are enabled)
    user_context = load_enabled_context()
    if user_context:
        context_str += user_context + "\n\n"

    # Append per-persona memory
    if persona_name:
        memory_content = get_memory_content(persona_name)
        if memory_content:
            context_str += "--- YOUR MEMORY ABOUT THIS USER ---\n"
            context_str += "The following is your memory about the person you're talking to. "
            context_str += "It is written to you, about them — these are things you know, "
            context_str += "have observed, and carry from previous conversations.\n\n"
            context_str += memory_content + "\n\n"

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


def get_persona_identity(persona_dir):
    """
    Load raw persona identity content (all .md files concatenated, no headers).

    Args:
        persona_dir: Path to persona folder (e.g., "personas/assistant")

    Returns:
        Concatenated identity content string, or empty string if not found
    """
    identity = ""
    if os.path.exists(persona_dir):
        for filename in sorted(os.listdir(persona_dir)):
            if filename.endswith(".md"):
                filepath = os.path.join(persona_dir, filename)
                with open(filepath, 'r') as f:
                    identity += f.read() + "\n"
    return identity.strip()


def save_persona_config(persona_name, config_data, personas_dir="personas"):
    """
    Save config.json for a persona directory.

    Args:
        persona_name: Name of the persona folder
        config_data: Dict of config values to save
        personas_dir: Base directory containing personas
    """
    config_path = Path(personas_dir) / persona_name / "config.json"
    config_path.parent.mkdir(parents=True, exist_ok=True)
    with open(config_path, 'w') as f:
        json.dump(config_data, f, indent=4)
        f.flush()
        os.fsync(f.fileno())


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
