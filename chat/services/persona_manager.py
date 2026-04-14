"""
PersonaManager service — all persona filesystem I/O in one place.

Handles persona CRUD, identity file reading/writing, and orchestrates
rename/delete side effects (memory, sessions, context files, config).
"""
import logging
import os
import shutil

from django.conf import settings as django_settings

from .memory_manager import rename_memory, delete_memory
from .session_manager import update_persona_across_sessions

logger = logging.getLogger(__name__)


def _get_persona_path(persona_name):
    """Return the full path to a persona directory."""
    return os.path.join(str(django_settings.PERSONAS_DIR), persona_name)


def _get_persona_context_path(persona_name):
    """Return the path to a persona's context files directory."""
    return django_settings.DATA_DIR / 'user_context' / 'personas' / persona_name


def _validate_persona_name(name):
    """Check that a persona name contains only alphanumeric chars and underscores."""
    return bool(name) and all(c.isalnum() or c == '_' for c in name)


def persona_exists(persona_name):
    """Check if a persona directory exists."""
    return os.path.exists(_get_persona_path(persona_name))


def get_persona_preview(persona_name):
    """
    Read the identity content for a persona (first .md file).

    Returns the content string, or empty string if not found.
    """
    persona_path = _get_persona_path(persona_name)
    if not os.path.exists(persona_path):
        return ""

    md_files = sorted(f for f in os.listdir(persona_path) if f.endswith(".md"))
    if not md_files:
        return ""

    with open(os.path.join(persona_path, md_files[0]), 'r') as f:
        return f.read()


def save_persona_identity(persona_name, content):
    """
    Write identity content to a persona's .md file.

    Writes to the first existing .md file in the persona directory.
    Returns True on success, False if persona or .md file not found.
    """
    persona_path = _get_persona_path(persona_name)
    if not os.path.exists(persona_path):
        return False

    md_files = sorted(f for f in os.listdir(persona_path) if f.endswith(".md"))
    if not md_files:
        return False

    filepath = os.path.join(persona_path, md_files[0])
    with open(filepath, 'w') as f:
        f.write(content)
        f.flush()
        os.fsync(f.fileno())
    return True


def create_persona(name, identity_content=""):
    """
    Create a new persona directory with an identity.md file.

    Returns (success, error_message) tuple.
    """
    if not _validate_persona_name(name):
        return False, "Invalid persona name. Use only letters, numbers, and underscores."

    if persona_exists(name):
        return False, f"A persona named '{name}' already exists."

    persona_path = _get_persona_path(name)
    os.makedirs(persona_path)

    filepath = os.path.join(persona_path, 'identity.md')
    with open(filepath, 'w') as f:
        f.write(identity_content)
        f.flush()
        os.fsync(f.fileno())

    return True, None


def delete_persona(persona_name):
    """
    Delete a persona and all its associated data.

    Handles side effects:
    1. Delete persona directory
    2. Delete memory file
    3. Delete persona context files directory

    Returns True if deleted, False if not found.
    """
    persona_path = _get_persona_path(persona_name)
    if not os.path.exists(persona_path):
        return False

    # 1. Delete persona directory
    shutil.rmtree(persona_path)

    # 2. Delete memory file
    delete_memory(persona_name)

    # 3. Delete persona context files directory
    context_path = _get_persona_context_path(persona_name)
    if os.path.exists(context_path):
        shutil.rmtree(context_path)
        logger.info(f"Deleted persona context directory: {context_path}")

    return True


def rename_persona(old_name, new_name, config=None, save_config_fn=None):
    """
    Rename a persona and handle all side effects.

    Orchestrates:
    1. Rename persona directory
    2. Rename memory file
    3. Rename persona context files directory
    4. Update all session files
    5. Update default persona in config if needed

    Args:
        old_name: Current persona name
        new_name: New persona name
        config: App config dict (for checking DEFAULT_PERSONA)
        save_config_fn: Function to persist config changes

    Returns (success, error_message) tuple.
    """
    if not _validate_persona_name(new_name):
        return False, "Invalid persona name. Use only letters, numbers, and underscores."

    if persona_exists(new_name):
        return False, f"A persona named '{new_name}' already exists."

    old_path = _get_persona_path(old_name)
    if not os.path.exists(old_path):
        return False, "Original persona not found"

    new_path = _get_persona_path(new_name)

    # 1. Rename persona directory
    shutil.move(old_path, new_path)

    # 2. Rename memory file
    rename_memory(old_name, new_name)

    # 3. Rename persona context files directory
    old_context = _get_persona_context_path(old_name)
    if os.path.exists(old_context):
        new_context = _get_persona_context_path(new_name)
        shutil.move(str(old_context), str(new_context))
        logger.info(f"Renamed persona context directory: {old_name} -> {new_name}")

    # 4. Update all session files
    update_persona_across_sessions(old_name, new_name)

    # 5. Update default persona in config if needed
    if config and save_config_fn and config.get("DEFAULT_PERSONA") == old_name:
        config["DEFAULT_PERSONA"] = new_name
        save_config_fn(config)

    return True, None
