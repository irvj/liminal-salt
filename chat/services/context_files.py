"""
Unified Context File Manager

Handles uploading, storing, toggling, and loading context files for both
global (user) scope and per-persona scope. Replaces the near-identical
user_context.py and persona_context.py modules.
"""
import json
import os

from .local_context import (
    add_local_directory_to_config,
    remove_local_directory_from_config,
    list_local_directories_from_config,
    toggle_local_file_in_config,
    get_local_file_content_from_dir,
    refresh_local_directory_in_config,
    load_enabled_local_context,
)


class ContextFileManager:
    """
    Manages context files (uploaded .md/.txt) and local directory references
    for a given scope (global or per-persona).

    Args:
        base_dir: Path to the directory where files and config.json live
        scope_label: Label used in prompt headers (e.g. "USER" or "PERSONA")
        header_description: Description text included in the prompt header
    """

    def __init__(self, base_dir, scope_label="USER",
                 header_description="The following files were provided by the user as additional context."):
        self.base_dir = base_dir
        self.scope_label = scope_label
        self.header_description = header_description

    def _ensure_dir(self):
        """Ensure the base directory exists."""
        if not os.path.exists(self.base_dir):
            os.makedirs(self.base_dir)
        return self.base_dir

    def _config_path(self):
        return self._ensure_dir() / 'config.json'

    def get_config(self):
        """Load config.json, returning empty config if missing."""
        config_path = self._config_path()
        if os.path.exists(config_path):
            with open(config_path, 'r') as f:
                return json.load(f)
        return {"files": {}}

    def save_config(self, config):
        """Save config to config.json."""
        config_path = self._config_path()
        with open(config_path, 'w') as f:
            json.dump(config, f, indent=2)

    def list_files(self):
        """
        Get list of uploaded context files with their enabled status.

        Returns:
            List of dicts: [{"name": "file.md", "enabled": True}, ...]
        """
        config = self.get_config()
        context_dir = self._ensure_dir()

        files = []
        for filename in sorted(os.listdir(context_dir)):
            if filename.endswith(('.md', '.txt')) and filename != 'config.json':
                enabled = config.get("files", {}).get(filename, {}).get("enabled", True)
                files.append({"name": filename, "enabled": enabled})

        return files

    def upload_file(self, uploaded_file):
        """
        Save an uploaded file and add it to config as enabled.

        Args:
            uploaded_file: Django UploadedFile object

        Returns:
            str: Filename if successful, None if invalid file type
        """
        filename = uploaded_file.name

        if not filename.endswith(('.md', '.txt')):
            return None

        filename = os.path.basename(filename)

        context_dir = self._ensure_dir()
        filepath = context_dir / filename

        with open(filepath, 'wb') as f:
            for chunk in uploaded_file.chunks():
                f.write(chunk)

        config = self.get_config()
        if "files" not in config:
            config["files"] = {}
        config["files"][filename] = {"enabled": True}
        self.save_config(config)

        return filename

    def delete_file(self, filename):
        """
        Delete a context file and remove it from config.

        Returns:
            bool: True if deleted, False if not found
        """
        filename = os.path.basename(filename)
        context_dir = self._ensure_dir()
        filepath = context_dir / filename

        if os.path.exists(filepath):
            os.remove(filepath)

            config = self.get_config()
            if filename in config.get("files", {}):
                del config["files"][filename]
                self.save_config(config)

            return True

        return False

    def toggle_file(self, filename, enabled=None):
        """
        Toggle or set the enabled status of a file.

        Args:
            filename: Name of file to toggle
            enabled: If provided, set to this value. If None, toggle.

        Returns:
            bool: New enabled status
        """
        filename = os.path.basename(filename)
        config = self.get_config()

        if "files" not in config:
            config["files"] = {}

        if filename not in config["files"]:
            config["files"][filename] = {"enabled": True}

        if enabled is None:
            current = config["files"][filename].get("enabled", True)
            config["files"][filename]["enabled"] = not current
        else:
            config["files"][filename]["enabled"] = enabled

        self.save_config(config)
        return config["files"][filename]["enabled"]

    def get_file_content(self, filename):
        """
        Get the content of a context file.

        Returns:
            str: File content, or None if file doesn't exist
        """
        filename = os.path.basename(filename)
        filepath = self._ensure_dir() / filename

        if os.path.exists(filepath):
            with open(filepath, 'r') as f:
                return f.read()
        return None

    def save_file_content(self, filename, content):
        """
        Save content to a context file.

        Returns:
            bool: True if successful, False if file doesn't exist
        """
        filename = os.path.basename(filename)
        filepath = self._ensure_dir() / filename

        if os.path.exists(filepath):
            with open(filepath, 'w') as f:
                f.write(content)
            return True
        return False

    def load_enabled_context(self):
        """
        Load and concatenate content from all enabled context files,
        including both uploaded files and local directory references.

        Returns:
            str: Concatenated content with headers, or empty string if none
        """
        files = self.list_files()
        enabled_files = [f for f in files if f["enabled"]]

        context_dir = self._ensure_dir()
        content_parts = []

        if enabled_files:
            content_parts.append(f"--- {self.scope_label} CONTEXT FILES ---")
            content_parts.append(f"{self.header_description}\n")

            for file_info in enabled_files:
                filepath = context_dir / file_info["name"]
                if os.path.exists(filepath):
                    with open(filepath, 'r') as f:
                        content_parts.append(f"--- {file_info['name']} ---")
                        content_parts.append(f.read())
                        content_parts.append("")

        # Append local directory context
        config = self.get_config()
        local_context = load_enabled_local_context(config)
        if local_context:
            content_parts.append(local_context)

        return "\n".join(content_parts) if content_parts else ""

    # -----------------------------------------------------------------
    # Local directory wrappers
    # -----------------------------------------------------------------

    def add_local_directory(self, dir_path):
        return add_local_directory_to_config(dir_path, self.get_config, self.save_config)

    def remove_local_directory(self, dir_path):
        return remove_local_directory_from_config(dir_path, self.get_config, self.save_config)

    def list_local_directories(self):
        return list_local_directories_from_config(self.get_config)

    def toggle_local_file(self, dir_path, filename, enabled=None):
        return toggle_local_file_in_config(dir_path, filename, self.get_config, self.save_config, enabled)

    def get_local_file_content(self, dir_path, filename):
        return get_local_file_content_from_dir(dir_path, filename)

    def refresh_local_directory(self, dir_path):
        return refresh_local_directory_in_config(dir_path, self.get_config, self.save_config)
