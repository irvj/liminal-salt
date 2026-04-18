"""
SessionManager service — all session file I/O in one place.

Every read/write of session JSON files goes through this module.
Views never touch session files directly.
"""
import json
import logging
import os
from datetime import datetime, timezone
from zoneinfo import ZoneInfo

from django.conf import settings as django_settings

logger = logging.getLogger(__name__)


def get_session_path(session_id):
    """Return the full path for a session file."""
    return django_settings.SESSIONS_DIR / session_id


def _read_session(session_path):
    """Read and parse a session JSON file. Returns dict or None."""
    if not os.path.exists(session_path):
        return None
    try:
        with open(session_path, 'r') as f:
            data = json.load(f)
        if isinstance(data, dict):
            return data
        return None
    except (json.JSONDecodeError, IOError) as e:
        logger.error(f"Error reading session {session_path}: {e}")
        return None


def _write_session(session_path, data):
    """Write session data with flush + fsync for durability."""
    with open(session_path, 'w') as f:
        json.dump(data, f, indent=2)
        f.flush()
        os.fsync(f.fileno())


def load_session(session_id):
    """
    Load a session file and return its data.

    Returns dict with keys: title, persona, messages, draft, pinned
    Returns None if the session doesn't exist or can't be read.
    """
    session_path = get_session_path(session_id)
    return _read_session(session_path)


def create_session(session_id, persona, messages=None, title="New Chat", mode="chatbot"):
    """
    Create a new session file.

    Args:
        session_id: Filename like session_YYYYMMDD_HHMMSS.json
        persona: Persona name for this session
        messages: Initial message list (default empty)
        title: Session title (default "New Chat")
        mode: Thread mode, "chatbot" (default) or "roleplay". Immutable once set.

    Returns the session data dict that was written.
    """
    session_path = get_session_path(session_id)
    data = {
        "title": title,
        "persona": persona,
        "mode": mode,
        "messages": messages or [],
    }
    _write_session(session_path, data)
    return data


def delete_session(session_id):
    """Delete a session file. Returns True if deleted, False if not found."""
    session_path = get_session_path(session_id)
    if os.path.exists(session_path):
        os.remove(session_path)
        return True
    return False


def get_session_persona(session_id):
    """Get the persona name from a session file. Returns None if not found."""
    data = load_session(session_id)
    if data:
        return data.get("persona")
    return None


def get_session_draft(session_id):
    """Get the draft text from a session file. Returns empty string if not found."""
    data = load_session(session_id)
    if data:
        return data.get("draft", "")
    return ""


def toggle_pin(session_id):
    """
    Toggle the pinned status of a session.

    Returns the new pinned state, or None if the session doesn't exist.
    """
    session_path = get_session_path(session_id)
    data = _read_session(session_path)
    if data is None:
        return None

    data['pinned'] = not data.get('pinned', False)
    _write_session(session_path, data)
    return data['pinned']


def rename_session(session_id, new_title):
    """
    Update the title of a session.

    Returns True on success, False if the session doesn't exist.
    """
    session_path = get_session_path(session_id)
    data = _read_session(session_path)
    if data is None:
        return False

    data['title'] = new_title
    _write_session(session_path, data)
    return True


def save_draft(session_id, draft_text):
    """
    Save draft text to a session file.

    Returns True on success, False if the session doesn't exist.
    """
    session_path = get_session_path(session_id)
    data = _read_session(session_path)
    if data is None:
        return False

    data['draft'] = draft_text
    _write_session(session_path, data)
    return True


def clear_draft(session_id):
    """Clear the draft field in a session file."""
    return save_draft(session_id, '')


def save_scenario(session_id, content):
    """
    Save scenario text to a session file.

    Returns True on success, False if the session doesn't exist.
    """
    session_path = get_session_path(session_id)
    data = _read_session(session_path)
    if data is None:
        return False

    data['scenario'] = content
    _write_session(session_path, data)
    return True


def get_session_scenario(session_id):
    """Get the scenario text from a session file. Returns empty string if not set."""
    data = load_session(session_id)
    if data:
        return data.get("scenario", "")
    return ""


def save_thread_memory(session_id, content):
    """
    Save thread memory for a session and stamp the update time in UTC.

    Returns True on success, False if the session doesn't exist.
    """
    session_path = get_session_path(session_id)
    data = _read_session(session_path)
    if data is None:
        return False

    data['thread_memory'] = content
    data['thread_memory_updated_at'] = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    _write_session(session_path, data)
    return True


def remove_last_assistant_message(session_id):
    """
    Remove the last assistant message from a session.

    Returns (success, last_user_message, session_data) tuple.
    - success: True if removal succeeded
    - last_user_message: The content of the last user message (now the final message)
    - session_data: The full session data dict after modification
    """
    session_path = get_session_path(session_id)
    data = _read_session(session_path)
    if data is None:
        return False, None, None

    messages = data.get('messages', [])
    if len(messages) < 2:
        return False, None, None

    if messages[-1].get('role') != 'assistant':
        return False, None, None

    messages.pop()

    if messages[-1].get('role') != 'user':
        return False, None, None

    user_message = messages[-1].get('content', '')
    data['messages'] = messages
    _write_session(session_path, data)
    return True, user_message, data


def update_last_user_message(session_id, new_content):
    """
    Update the content of the last user message in a session.

    Returns True on success, False if session doesn't exist or has no user messages.
    """
    session_path = get_session_path(session_id)
    data = _read_session(session_path)
    if data is None:
        return False

    messages = data.get('messages', [])
    if not messages:
        return False

    last_user_idx = None
    for i in range(len(messages) - 1, -1, -1):
        if messages[i].get('role') == 'user':
            last_user_idx = i
            break

    if last_user_idx is None:
        return False

    messages[last_user_idx]['content'] = new_content
    data['messages'] = messages
    _write_session(session_path, data)
    return True


def update_persona_across_sessions(old_name, new_name):
    """
    Update all session files that reference the old persona name.
    Used when a persona is renamed.
    """
    sessions_dir = django_settings.SESSIONS_DIR
    if not os.path.exists(sessions_dir):
        return

    for filename in os.listdir(sessions_dir):
        if not filename.endswith('.json'):
            continue
        filepath = os.path.join(sessions_dir, filename)
        try:
            with open(filepath, 'r') as f:
                data = json.load(f)

            if isinstance(data, dict) and data.get('persona') == old_name:
                data['persona'] = new_name
                _write_session(filepath, data)
        except (json.JSONDecodeError, IOError) as e:
            logger.error(f"Error updating session {filename}: {e}")
            continue


def generate_session_id():
    """Generate a new session ID based on current timestamp."""
    return f"session_{datetime.now().strftime('%Y%m%d_%H%M%S')}.json"


def make_user_timestamp(user_timezone='UTC'):
    """Create an ISO 8601 timestamp in the user's timezone."""
    try:
        tz = ZoneInfo(user_timezone)
    except (KeyError, ValueError):
        tz = ZoneInfo('UTC')
    return datetime.now(tz).isoformat()
