"""
User Context File Management Service

Handles uploading, storing, and loading user-provided context files
that are included in prompts but NOT used for memory generation.
"""
import os
import json
from django.conf import settings


def get_user_context_dir():
    """Get the user context directory path, creating it if needed."""
    context_dir = settings.DATA_DIR / 'user_context'
    if not os.path.exists(context_dir):
        os.makedirs(context_dir)
    return context_dir


def get_config_path():
    """Get the path to the config.json file."""
    return get_user_context_dir() / 'config.json'


def get_config():
    """Load config.json, creating empty config if missing."""
    config_path = get_config_path()
    if os.path.exists(config_path):
        with open(config_path, 'r') as f:
            return json.load(f)
    return {"files": {}}


def save_config(config):
    """Save config to config.json."""
    config_path = get_config_path()
    with open(config_path, 'w') as f:
        json.dump(config, f, indent=2)


def list_files():
    """
    Get list of uploaded context files with their enabled status.

    Returns:
        List of dicts: [{"name": "file.md", "enabled": True}, ...]
    """
    config = get_config()
    context_dir = get_user_context_dir()

    files = []
    for filename in sorted(os.listdir(context_dir)):
        if filename.endswith(('.md', '.txt')) and filename != 'config.json':
            enabled = config.get("files", {}).get(filename, {}).get("enabled", True)
            files.append({"name": filename, "enabled": enabled})

    return files


def upload_file(uploaded_file):
    """
    Save an uploaded file and add it to config as enabled.

    Args:
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
    context_dir = get_user_context_dir()
    filepath = context_dir / filename

    with open(filepath, 'wb') as f:
        for chunk in uploaded_file.chunks():
            f.write(chunk)

    # Add to config as enabled
    config = get_config()
    if "files" not in config:
        config["files"] = {}
    config["files"][filename] = {"enabled": True}
    save_config(config)

    return filename


def delete_file(filename):
    """
    Delete a context file and remove it from config.

    Args:
        filename: Name of file to delete

    Returns:
        bool: True if deleted, False if not found
    """
    # Sanitize filename
    filename = os.path.basename(filename)

    context_dir = get_user_context_dir()
    filepath = context_dir / filename

    if os.path.exists(filepath):
        os.remove(filepath)

        # Remove from config
        config = get_config()
        if filename in config.get("files", {}):
            del config["files"][filename]
            save_config(config)

        return True

    return False


def toggle_file(filename, enabled=None):
    """
    Toggle or set the enabled status of a file.

    Args:
        filename: Name of file to toggle
        enabled: If provided, set to this value. If None, toggle current value.

    Returns:
        bool: New enabled status
    """
    filename = os.path.basename(filename)
    config = get_config()

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

    save_config(config)
    return config["files"][filename]["enabled"]


def load_enabled_context():
    """
    Load and concatenate content from all enabled context files.

    Returns:
        str: Concatenated content with file headers, or empty string if none
    """
    files = list_files()
    enabled_files = [f for f in files if f["enabled"]]

    if not enabled_files:
        return ""

    context_dir = get_user_context_dir()
    content_parts = []

    content_parts.append("--- USER CONTEXT FILES ---")
    content_parts.append("The following files were provided by the user as additional context.\n")

    for file_info in enabled_files:
        filepath = context_dir / file_info["name"]
        if os.path.exists(filepath):
            with open(filepath, 'r') as f:
                content_parts.append(f"--- {file_info['name']} ---")
                content_parts.append(f.read())
                content_parts.append("")  # Empty line between files

    return "\n".join(content_parts)
