"""
Persona-Specific Context File Management Service

Handles uploading, storing, and loading context files that are scoped
to individual personas. These files are included in prompts only for
chats using the associated persona.
"""
import os
import json
from django.conf import settings


def get_persona_context_dir(persona_name):
    """
    Get the persona-specific context directory path, creating it if needed.

    Args:
        persona_name: Name of the persona (e.g., 'assistant')

    Returns:
        Path: Directory path for this persona's context files
    """
    # Sanitize persona name
    persona_name = os.path.basename(persona_name)

    context_dir = settings.DATA_DIR / 'user_context' / 'personas' / persona_name
    if not os.path.exists(context_dir):
        os.makedirs(context_dir)
    return context_dir


def get_config_path(persona_name):
    """Get the path to the persona's config.json file."""
    return get_persona_context_dir(persona_name) / 'config.json'


def get_config(persona_name):
    """Load persona config.json, creating empty config if missing."""
    config_path = get_config_path(persona_name)
    if os.path.exists(config_path):
        with open(config_path, 'r') as f:
            return json.load(f)
    return {"files": {}}


def save_config(persona_name, config):
    """Save config to persona's config.json."""
    config_path = get_config_path(persona_name)
    with open(config_path, 'w') as f:
        json.dump(config, f, indent=2)


def list_files(persona_name):
    """
    Get list of uploaded context files for a persona with their enabled status.

    Args:
        persona_name: Name of the persona

    Returns:
        List of dicts: [{"name": "file.md", "enabled": True}, ...]
    """
    config = get_config(persona_name)
    context_dir = get_persona_context_dir(persona_name)

    files = []
    for filename in sorted(os.listdir(context_dir)):
        if filename.endswith(('.md', '.txt')) and filename != 'config.json':
            enabled = config.get("files", {}).get(filename, {}).get("enabled", True)
            files.append({"name": filename, "enabled": enabled})

    return files


def upload_file(persona_name, uploaded_file):
    """
    Save an uploaded file for a persona and add it to config as enabled.

    Args:
        persona_name: Name of the persona
        uploaded_file: Django UploadedFile object

    Returns:
        str: Filename if successful, None if invalid file type
    """
    filename = uploaded_file.name

    # Validate file extension
    if not filename.endswith(('.md', '.txt')):
        return None

    # Sanitize filename (remove path components)
    filename = os.path.basename(filename)

    # Save the file
    context_dir = get_persona_context_dir(persona_name)
    filepath = context_dir / filename

    with open(filepath, 'wb') as f:
        for chunk in uploaded_file.chunks():
            f.write(chunk)

    # Add to config as enabled
    config = get_config(persona_name)
    if "files" not in config:
        config["files"] = {}
    config["files"][filename] = {"enabled": True}
    save_config(persona_name, config)

    return filename


def delete_file(persona_name, filename):
    """
    Delete a context file from a persona and remove it from config.

    Args:
        persona_name: Name of the persona
        filename: Name of file to delete

    Returns:
        bool: True if deleted, False if not found
    """
    # Sanitize filename
    filename = os.path.basename(filename)

    context_dir = get_persona_context_dir(persona_name)
    filepath = context_dir / filename

    if os.path.exists(filepath):
        os.remove(filepath)

        # Remove from config
        config = get_config(persona_name)
        if filename in config.get("files", {}):
            del config["files"][filename]
            save_config(persona_name, config)

        return True

    return False


def toggle_file(persona_name, filename, enabled=None):
    """
    Toggle or set the enabled status of a persona's context file.

    Args:
        persona_name: Name of the persona
        filename: Name of file to toggle
        enabled: If provided, set to this value. If None, toggle current value.

    Returns:
        bool: New enabled status
    """
    filename = os.path.basename(filename)
    config = get_config(persona_name)

    if "files" not in config:
        config["files"] = {}

    if filename not in config["files"]:
        config["files"][filename] = {"enabled": True}

    if enabled is None:
        # Toggle
        current = config["files"][filename].get("enabled", True)
        config["files"][filename]["enabled"] = not current
    else:
        # Set to specified value
        config["files"][filename]["enabled"] = enabled

    save_config(persona_name, config)
    return config["files"][filename]["enabled"]


def get_file_content(persona_name, filename):
    """
    Get the content of a persona's context file.

    Args:
        persona_name: Name of the persona
        filename: Name of file to read

    Returns:
        str: File content, or None if file doesn't exist
    """
    filename = os.path.basename(filename)
    context_dir = get_persona_context_dir(persona_name)
    filepath = context_dir / filename

    if os.path.exists(filepath):
        with open(filepath, 'r') as f:
            return f.read()
    return None


def save_file_content(persona_name, filename, content):
    """
    Save content to a persona's context file.

    Args:
        persona_name: Name of the persona
        filename: Name of file to write
        content: Content to write

    Returns:
        bool: True if successful
    """
    filename = os.path.basename(filename)
    context_dir = get_persona_context_dir(persona_name)
    filepath = context_dir / filename

    if os.path.exists(filepath):
        with open(filepath, 'w') as f:
            f.write(content)
        return True
    return False


def load_enabled_context(persona_name):
    """
    Load and concatenate content from all enabled context files for a persona.

    Args:
        persona_name: Name of the persona

    Returns:
        str: Concatenated content with file headers, or empty string if none
    """
    files = list_files(persona_name)
    enabled_files = [f for f in files if f["enabled"]]

    if not enabled_files:
        return ""

    context_dir = get_persona_context_dir(persona_name)
    content_parts = []

    content_parts.append("--- PERSONA CONTEXT FILES ---")
    content_parts.append("The following files provide additional context for this persona.\n")

    for file_info in enabled_files:
        filepath = context_dir / file_info["name"]
        if os.path.exists(filepath):
            with open(filepath, 'r') as f:
                content_parts.append(f"--- {file_info['name']} ---")
                content_parts.append(f.read())
                content_parts.append("")  # Empty line between files

    return "\n".join(content_parts)
