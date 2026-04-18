"""
SessionManager service — all session file I/O in one place.

Every read/write of session JSON files goes through this module.
Views never touch session files directly.

Concurrency: every read and every read-modify-write acquires a per-session
lock so that concurrent writers (e.g. ChatCore saving messages while the
thread-memory worker saves a new summary) can't clobber each other. Locks
are process-local; the app is single-process.
"""
import json
import logging
import os
import threading
from contextlib import contextmanager
from datetime import datetime, timezone

from django.conf import settings as django_settings

logger = logging.getLogger(__name__)


# =============================================================================
# Per-session locking
# =============================================================================

_locks_registry_guard = threading.Lock()
_locks_registry = {}


def _get_session_lock(session_id):
    """Get or create the lock for a specific session id."""
    with _locks_registry_guard:
        lock = _locks_registry.get(session_id)
        if lock is None:
            lock = threading.Lock()
            _locks_registry[session_id] = lock
        return lock


@contextmanager
def _session_lock(session_id):
    """Context manager that holds the per-session lock for the block."""
    lock = _get_session_lock(session_id)
    lock.acquire()
    try:
        yield
    finally:
        lock.release()


def _drop_session_lock(session_id):
    """Remove the lock entry for a deleted session."""
    with _locks_registry_guard:
        _locks_registry.pop(session_id, None)


# =============================================================================
# Timestamps
# =============================================================================

def now_timestamp():
    """
    Canonical UTC timestamp for session data: microsecond-precision ISO 8601
    with `+00:00` offset. Uniform width so lexicographic comparison matches
    chronological order, and `datetime.fromisoformat` round-trips it natively.
    """
    return datetime.now(timezone.utc).isoformat(timespec='microseconds')


# =============================================================================
# Low-level file I/O (always called under a held session lock)
# =============================================================================

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


# =============================================================================
# Public reads
# =============================================================================

def load_session(session_id):
    """
    Load a session file and return its data.

    Returns dict with keys: title, persona, messages, draft, pinned.
    Returns None if the session doesn't exist or can't be read.
    """
    with _session_lock(session_id):
        return _read_session(get_session_path(session_id))


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


def get_session_scenario(session_id):
    """Get the scenario text from a session file. Returns empty string if not set."""
    data = load_session(session_id)
    if data:
        return data.get("scenario", "")
    return ""


# =============================================================================
# Public writes (each acquires the session lock)
# =============================================================================

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
    data = {
        "title": title,
        "persona": persona,
        "mode": mode,
        "messages": messages or [],
    }
    with _session_lock(session_id):
        _write_session(get_session_path(session_id), data)
    return data


def delete_session(session_id):
    """Delete a session file. Returns True if deleted, False if not found."""
    with _session_lock(session_id):
        session_path = get_session_path(session_id)
        if os.path.exists(session_path):
            os.remove(session_path)
            deleted = True
        else:
            deleted = False
    if deleted:
        _drop_session_lock(session_id)
    return deleted


def save_chat_history(session_id, title, persona, messages):
    """
    Write chat-owned fields (title, persona, messages) while preserving
    every other field (mode, scenario, thread_memory, thread_memory_updated_at,
    thread_memory_settings, pinned, draft, etc.). Called by ChatCore in
    place of its own file I/O so writes serialize against other session
    writers via the per-session lock.
    """
    with _session_lock(session_id):
        session_path = get_session_path(session_id)
        data = _read_session(session_path) or {}
        data['title'] = title
        data['persona'] = persona
        data['messages'] = messages
        _write_session(session_path, data)


def toggle_pin(session_id):
    """
    Toggle the pinned status of a session.

    Returns the new pinned state, or None if the session doesn't exist.
    """
    with _session_lock(session_id):
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
    with _session_lock(session_id):
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
    with _session_lock(session_id):
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
    with _session_lock(session_id):
        session_path = get_session_path(session_id)
        data = _read_session(session_path)
        if data is None:
            return False
        data['scenario'] = content
        _write_session(session_path, data)
        return True


def save_thread_memory(session_id, content, summarized_through):
    """
    Save thread memory for a session and stamp it with the timestamp of
    the last message actually included in the summary.

    `summarized_through` must be the timestamp of the newest message that
    went into the LLM input. Using "now" here would silently skip any
    messages written during the LLM call, because `filter_new_messages`
    gates on `timestamp > thread_memory_updated_at`.

    Returns True on success, False if the session doesn't exist.
    """
    with _session_lock(session_id):
        session_path = get_session_path(session_id)
        data = _read_session(session_path)
        if data is None:
            return False
        data['thread_memory'] = content
        data['thread_memory_updated_at'] = summarized_through
        _write_session(session_path, data)
        return True


def save_thread_memory_settings_override(session_id, settings):
    """
    Save a per-thread override for thread-memory settings. Only the keys
    present in `settings` are written — other thread_memory_settings keys
    are preserved. Returns True on success, False if session doesn't exist.
    """
    with _session_lock(session_id):
        session_path = get_session_path(session_id)
        data = _read_session(session_path)
        if data is None:
            return False
        existing = data.get('thread_memory_settings') or {}
        existing.update(settings)
        data['thread_memory_settings'] = existing
        _write_session(session_path, data)
        return True


def reset_thread_memory_settings_override(session_id):
    """
    Remove the per-thread thread_memory_settings override, reverting the
    session to persona/global defaults. Returns True on success, False if
    the session doesn't exist.
    """
    with _session_lock(session_id):
        session_path = get_session_path(session_id)
        data = _read_session(session_path)
        if data is None:
            return False
        if 'thread_memory_settings' in data:
            del data['thread_memory_settings']
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
    with _session_lock(session_id):
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
    with _session_lock(session_id):
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
    Used when a persona is renamed. Locks each session individually during
    its own read-modify-write so concurrent writes on other sessions
    aren't blocked.
    """
    sessions_dir = django_settings.SESSIONS_DIR
    if not os.path.exists(sessions_dir):
        return

    for filename in os.listdir(sessions_dir):
        if not filename.endswith('.json'):
            continue
        with _session_lock(filename):
            filepath = os.path.join(sessions_dir, filename)
            data = _read_session(filepath)
            if isinstance(data, dict) and data.get('persona') == old_name:
                data['persona'] = new_name
                _write_session(filepath, data)


def generate_session_id():
    """Generate a new session ID based on current timestamp."""
    return f"session_{datetime.now().strftime('%Y%m%d_%H%M%S')}.json"


def fork_session_to_roleplay(source_session_id):
    """
    Fork a chatbot thread into a new roleplay session.

    The fork is a create-only action — no memory of any kind is edited.
    Thread memory carries over intact so the user can keep, clear, or
    regenerate it at their discretion. Persona memory is untouched.
    The original session is untouched and continues contributing to
    persona memory normally.

    Copies:  persona, messages, thread_memory, thread_memory_updated_at
    Resets:  title ("New Chat"), pinned/draft (absent), scenario (absent)
    Sets:    mode = "roleplay"

    Returns the new session id on success, None if the source doesn't
    exist or no non-colliding id could be generated.
    """
    with _session_lock(source_session_id):
        source = _read_session(get_session_path(source_session_id))
    if source is None:
        return None

    # Generate a collision-free id. Second-precision timestamps can collide
    # if the user created and forked the source in the same second.
    new_session_id = generate_session_id()
    new_path = get_session_path(new_session_id)
    if new_session_id == source_session_id or os.path.exists(new_path):
        base = new_session_id[:-len('.json')]
        new_session_id = None
        for i in range(1, 100):
            candidate = f"{base}_{i}.json"
            candidate_path = get_session_path(candidate)
            if candidate != source_session_id and not os.path.exists(candidate_path):
                new_session_id = candidate
                new_path = candidate_path
                break
        if new_session_id is None:
            return None

    new_data = {
        "title": "New Chat",
        "persona": source.get("persona", "assistant"),
        "mode": "roleplay",
        "messages": list(source.get("messages", [])),
    }
    if "thread_memory" in source:
        new_data["thread_memory"] = source["thread_memory"]
    if "thread_memory_updated_at" in source:
        new_data["thread_memory_updated_at"] = source["thread_memory_updated_at"]

    with _session_lock(new_session_id):
        _write_session(new_path, new_data)
    return new_session_id


def make_user_timestamp(user_timezone='UTC'):
    """
    Timestamp for a user message. Always UTC so it compares correctly
    against other session timestamps. `user_timezone` is accepted but
    ignored; display code re-applies the zone at render time.
    """
    return now_timestamp()
