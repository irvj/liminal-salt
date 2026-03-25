"""
Local Directory Context File Management Service

Shared functions for managing local directory references in context file configs.
Both user_context and persona_context delegate to these generic functions
by passing their own get_config/save_config callables.
"""
import os
import logging

from django.conf import settings

logger = logging.getLogger(__name__)

MAX_FILES_PER_DIR = 200


def validate_directory_path(dir_path):
    """
    Validate and resolve a directory path.

    Returns:
        (bool, str): (success, resolved_path_or_error_message)
    """
    if not dir_path or not dir_path.strip():
        return False, "No directory path provided"

    resolved = os.path.realpath(dir_path)

    # Block paths inside app's DATA_DIR
    data_dir = os.path.realpath(str(settings.DATA_DIR))
    if resolved == data_dir or resolved.startswith(data_dir + os.sep):
        return False, "Cannot reference directories inside the app's data directory"

    if not os.path.isdir(resolved):
        return False, f"Directory not found: {resolved}"

    if not os.access(resolved, os.R_OK):
        return False, f"Directory not readable: {resolved}"

    return True, resolved


def list_directory_files(dir_path):
    """
    List .md and .txt files in a directory (non-recursive).

    Returns:
        list[dict]: [{"name": "file.md", "exists": True}, ...]
    """
    files = []
    if not os.path.isdir(dir_path):
        return files

    count = 0
    for filename in sorted(os.listdir(dir_path)):
        if filename.endswith(('.md', '.txt')):
            files.append({
                "name": filename,
                "exists": os.path.isfile(os.path.join(dir_path, filename))
            })
            count += 1
            if count >= MAX_FILES_PER_DIR:
                break

    return files


def load_enabled_local_context(config):
    """
    Read all enabled files from all configured local directories.

    Returns:
        str: Formatted context string, or empty string if none
    """
    local_dirs = config.get("local_directories", {})
    if not local_dirs:
        return ""

    content_parts = []
    content_parts.append("--- LOCAL CONTEXT FILES ---")
    content_parts.append("The following files are referenced from local directories.\n")

    found_any = False
    for dir_path, dir_config in local_dirs.items():
        resolved = os.path.realpath(dir_path)
        if not os.path.isdir(resolved):
            continue

        for filename, file_config in dir_config.get("files", {}).items():
            if not file_config.get("enabled", False):
                continue

            safe_name = os.path.basename(filename)
            filepath = os.path.join(resolved, safe_name)
            if not os.path.isfile(filepath):
                logger.warning(f"Local context file missing: {filepath}")
                continue

            try:
                with open(filepath, 'r', errors='replace') as f:
                    content_parts.append(f"--- {safe_name} (from {dir_path}) ---")
                    content_parts.append(f.read())
                    content_parts.append("")
                    found_any = True
            except Exception as e:
                logger.warning(f"Failed to read local context file {filepath}: {e}")

    if not found_any:
        return ""

    return "\n".join(content_parts)


# =============================================================================
# Generic config-backed functions
# =============================================================================

def add_local_directory_to_config(dir_path, get_config_fn, save_config_fn):
    """
    Add a local directory to the config.

    Returns:
        (bool, str, list): (success, resolved_path_or_error, files_list)
    """
    valid, result = validate_directory_path(dir_path)
    if not valid:
        return False, result, []

    resolved = result
    config = get_config_fn()

    if "local_directories" not in config:
        config["local_directories"] = {}

    if resolved in config["local_directories"]:
        return False, "Directory already added", []

    # Scan for files and enable them by default
    files = list_directory_files(resolved)
    config["local_directories"][resolved] = {
        "files": {f["name"]: {"enabled": True} for f in files}
    }

    save_config_fn(config)
    return True, resolved, files


def remove_local_directory_from_config(dir_path, get_config_fn, save_config_fn):
    """Remove a local directory from the config."""
    resolved = os.path.realpath(dir_path)
    config = get_config_fn()

    local_dirs = config.get("local_directories", {})
    if resolved in local_dirs:
        del local_dirs[resolved]
        config["local_directories"] = local_dirs
        save_config_fn(config)
        return True

    return False


def list_local_directories_from_config(get_config_fn):
    """
    List all configured local directories with their files.

    Returns:
        list[dict]: [{"path": "/abs/path", "exists": True, "files": [...]}, ...]
    """
    config = get_config_fn()
    local_dirs = config.get("local_directories", {})
    result = []

    for dir_path, dir_config in local_dirs.items():
        exists = os.path.isdir(dir_path)
        files = []

        if exists:
            # Get actual files on disk
            disk_files = {f["name"]: f["exists"] for f in list_directory_files(dir_path)}
            config_files = dir_config.get("files", {})

            for filename in sorted(set(list(disk_files.keys()) + list(config_files.keys()))):
                files.append({
                    "name": filename,
                    "enabled": config_files.get(filename, {}).get("enabled", False),
                    "exists": disk_files.get(filename, False)
                })
        else:
            # Directory missing - show config files as missing
            for filename, file_config in dir_config.get("files", {}).items():
                files.append({
                    "name": filename,
                    "enabled": file_config.get("enabled", False),
                    "exists": False
                })

        result.append({
            "path": dir_path,
            "exists": exists,
            "files": files
        })

    return result


def toggle_local_file_in_config(dir_path, filename, get_config_fn, save_config_fn, enabled=None):
    """Toggle or set the enabled status of a file in a local directory."""
    resolved = os.path.realpath(dir_path)
    safe_name = os.path.basename(filename)

    config = get_config_fn()
    local_dirs = config.get("local_directories", {})

    if resolved not in local_dirs:
        return False

    files = local_dirs[resolved].get("files", {})
    if safe_name not in files:
        files[safe_name] = {"enabled": True}

    if enabled is None:
        files[safe_name]["enabled"] = not files[safe_name].get("enabled", True)
    else:
        files[safe_name]["enabled"] = enabled

    local_dirs[resolved]["files"] = files
    config["local_directories"] = local_dirs
    save_config_fn(config)
    return True


def get_local_file_content_from_dir(dir_path, filename):
    """
    Read a file from a local directory.

    Returns:
        str or None: File content, or None if not found/readable
    """
    resolved = os.path.realpath(dir_path)
    safe_name = os.path.basename(filename)
    filepath = os.path.join(resolved, safe_name)

    if not os.path.isfile(filepath):
        return None

    try:
        with open(filepath, 'r', errors='replace') as f:
            return f.read()
    except Exception:
        return None


def refresh_local_directory_in_config(dir_path, get_config_fn, save_config_fn):
    """Re-scan a directory and update config with any new/removed files."""
    resolved = os.path.realpath(dir_path)
    config = get_config_fn()
    local_dirs = config.get("local_directories", {})

    if resolved not in local_dirs:
        return False

    if not os.path.isdir(resolved):
        return False

    disk_files = list_directory_files(resolved)
    disk_names = {f["name"] for f in disk_files}
    existing_config = local_dirs[resolved].get("files", {})

    # Keep enabled status for existing files, enable new files by default
    new_files = {}
    for name in sorted(disk_names):
        if name in existing_config:
            new_files[name] = existing_config[name]
        else:
            new_files[name] = {"enabled": True}

    local_dirs[resolved]["files"] = new_files
    config["local_directories"] = local_dirs
    save_config_fn(config)
    return True


def browse_directory(path, show_hidden=False):
    """
    Browse a directory for subdirectories.

    Returns:
        dict: {"current": str, "parent": str|None, "dirs": list, "has_context_files": bool, "error": str}
    """
    result = {
        "current": "",
        "parent": None,
        "dirs": [],
        "has_context_files": False,
        "error": ""
    }

    if not path:
        path = os.path.expanduser("~")

    resolved = os.path.realpath(path)
    result["current"] = resolved

    if not os.path.isdir(resolved):
        result["error"] = f"Not a directory: {resolved}"
        return result

    # Parent directory
    parent = os.path.dirname(resolved)
    if parent != resolved:
        result["parent"] = parent

    # Check for context files in current directory
    try:
        for f in os.listdir(resolved):
            if f.endswith(('.md', '.txt')) and os.path.isfile(os.path.join(resolved, f)):
                result["has_context_files"] = True
                break
    except PermissionError:
        result["error"] = "Permission denied"
        return result

    # List subdirectories
    try:
        for name in sorted(os.listdir(resolved)):
            if not show_hidden and name.startswith('.'):
                continue
            full_path = os.path.join(resolved, name)
            if os.path.isdir(full_path):
                result["dirs"].append({
                    "name": name,
                    "path": full_path
                })
    except PermissionError:
        result["error"] = "Permission denied"

    return result
