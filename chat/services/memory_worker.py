import json
import logging
import os
import threading
import time
from datetime import datetime, timezone

from django.conf import settings as django_settings

logger = logging.getLogger(__name__)

from ..utils import load_config, aggregate_all_sessions_messages
from .context_manager import get_persona_identity, get_available_personas, get_persona_config
from .memory_manager import (
    MemoryManager, get_memory_file, get_memory_model,
)
from .session_manager import load_session, save_thread_memory
from .thread_memory_manager import (
    ThreadMemoryManager, filter_new_messages, DEFAULT_THREAD_MEMORY_SIZE,
)


# =============================================================================
# Module-level state
# =============================================================================

_global_lock = threading.Lock()
_persona_locks = {}

_status_lock = threading.Lock()
_update_status = {}

_scheduler_thread = None
_scheduler_stop = threading.Event()
_scheduler_started = False
_scheduler_guard = threading.Lock()

# Next-due epoch time per persona for auto-update scheduling.
# Only read/written by the scheduler thread, so no lock needed.
_next_fire_time = {}

# Cache for session persona lookups: {filename: (mtime, persona)}
_session_cache = {}
_session_cache_lock = threading.Lock()

# =============================================================================
# Thread memory (per-session) state
# =============================================================================

_session_locks_guard = threading.Lock()
_session_locks = {}

_thread_status_lock = threading.Lock()
_thread_status = {}


def _get_session_lock(session_id):
    """Get or create a lock for a specific session's thread memory."""
    with _session_locks_guard:
        if session_id not in _session_locks:
            _session_locks[session_id] = threading.Lock()
        return _session_locks[session_id]


def get_thread_update_status(session_id):
    """Return current thread-memory update status for a session. Thread-safe."""
    with _thread_status_lock:
        return dict(_thread_status.get(session_id, {"state": "idle"}))


def _set_thread_status(session_id, **kwargs):
    """Update thread-memory status for a session. Thread-safe."""
    with _thread_status_lock:
        if session_id not in _thread_status:
            _thread_status[session_id] = {"state": "idle"}
        _thread_status[session_id].update(kwargs)


# =============================================================================
# Per-persona locking
# =============================================================================

def _get_persona_lock(persona_name):
    """Get or create a lock for a specific persona."""
    with _global_lock:
        if persona_name not in _persona_locks:
            _persona_locks[persona_name] = threading.Lock()
        return _persona_locks[persona_name]


# =============================================================================
# Status tracking
# =============================================================================

def get_update_status(persona_name):
    """Return current update status for a persona. Thread-safe."""
    with _status_lock:
        return dict(_update_status.get(persona_name, {"state": "idle"}))


def _set_status(persona_name, **kwargs):
    """Update status for a persona. Thread-safe."""
    with _status_lock:
        if persona_name not in _update_status:
            _update_status[persona_name] = {"state": "idle"}
        _update_status[persona_name].update(kwargs)


# =============================================================================
# Core update function (used by both manual and auto)
# =============================================================================

def run_memory_update(persona_name, config, source="manual"):
    """
    Run a full memory update for a persona. Acquires per-persona lock.

    Args:
        persona_name: The persona to update
        config: App config dict (from load_config)
        source: "manual" or "auto"

    Returns:
        True if update ran, False if skipped (lock held or no threads)
    """
    lock = _get_persona_lock(persona_name)
    if not lock.acquire(blocking=False):
        return False

    try:
        now = datetime.now(timezone.utc).isoformat()
        _set_status(persona_name, state="running", source=source, started_at=now, error=None)
        logger.info(f"Memory update started for '{persona_name}' ({source})")

        api_key = config.get("OPENROUTER_API_KEY")
        site_url = config.get("SITE_URL")
        site_name = config.get("SITE_NAME")
        personas_dir = str(django_settings.PERSONAS_DIR)

        # Load per-persona memory settings
        persona_cfg = get_persona_config(persona_name, personas_dir)
        user_history_max_threads = persona_cfg.get('user_history_max_threads', 10)
        user_history_messages_per_thread = persona_cfg.get('user_history_messages_per_thread', 100)

        threads = aggregate_all_sessions_messages(
            user_history_max_threads=user_history_max_threads if user_history_max_threads > 0 else None,
            user_history_messages_per_thread=user_history_messages_per_thread if user_history_messages_per_thread > 0 else None,
            persona_filter=persona_name,
        )

        if not threads:
            finished = datetime.now(timezone.utc).isoformat()
            _set_status(persona_name, state="completed", finished_at=finished,
                        error="No conversations found for this persona.")
            return True

        persona_dir = os.path.join(personas_dir, persona_name)
        persona_identity = get_persona_identity(persona_dir)
        memory_model = get_memory_model(config, persona_name, personas_dir)
        size_limit = persona_cfg.get('memory_size_limit', 8000)

        manager = MemoryManager(api_key, memory_model, site_url, site_name)
        success = manager.update_persona_memory(persona_name, persona_identity, threads, size_limit)

        finished = datetime.now(timezone.utc).isoformat()
        if success:
            _set_status(persona_name, state="completed", finished_at=finished)
            logger.info(f"Memory update completed for '{persona_name}' ({source})")
        else:
            _set_status(persona_name, state="failed", finished_at=finished,
                        error="The model returned an unusable response.")
            logger.warning(f"Memory update failed for '{persona_name}' ({source}): unusable response")
        return True

    except Exception as e:
        finished = datetime.now(timezone.utc).isoformat()
        _set_status(persona_name, state="failed", finished_at=finished, error=str(e))
        logger.error(f"Memory update failed for '{persona_name}' ({source}): {e}")
        return True

    finally:
        lock.release()


# =============================================================================
# Manual update (spawns background thread)
# =============================================================================

def start_manual_update(persona_name, config):
    """
    Start a background memory update for a persona.

    Returns False if an update is already running for this persona.
    """
    lock = _get_persona_lock(persona_name)
    if lock.locked():
        return False

    thread = threading.Thread(
        target=run_memory_update,
        args=(persona_name, config, "manual"),
        daemon=True,
    )
    thread.start()
    return True


def start_modify_update(persona_name, config, command):
    """
    Start a background memory modify for a persona.

    Returns False if an update is already running for this persona.
    """
    lock = _get_persona_lock(persona_name)
    if lock.locked():
        return False

    thread = threading.Thread(
        target=_run_modify_update,
        args=(persona_name, config, command),
        daemon=True,
    )
    thread.start()
    return True


def _run_modify_update(persona_name, config, command):
    """Run a memory modify command. Acquires per-persona lock."""
    lock = _get_persona_lock(persona_name)
    if not lock.acquire(blocking=False):
        return False

    try:
        now = datetime.now(timezone.utc).isoformat()
        _set_status(persona_name, state="running", source="modify", started_at=now, error=None)
        logger.info(f"Memory modify started for '{persona_name}'")

        api_key = config.get("OPENROUTER_API_KEY")
        site_url = config.get("SITE_URL")
        site_name = config.get("SITE_NAME")
        personas_dir = str(django_settings.PERSONAS_DIR)

        persona_cfg = get_persona_config(persona_name, personas_dir)
        size_limit = persona_cfg.get('memory_size_limit', 8000)

        persona_dir = os.path.join(personas_dir, persona_name)
        persona_identity = get_persona_identity(persona_dir)
        memory_model = get_memory_model(config, persona_name, personas_dir)

        manager = MemoryManager(api_key, memory_model, site_url, site_name)
        success = manager.modify_memory_with_command(persona_name, persona_identity, command, size_limit)

        finished = datetime.now(timezone.utc).isoformat()
        if success:
            _set_status(persona_name, state="completed", finished_at=finished)
            logger.info(f"Memory modify completed for '{persona_name}'")
        else:
            _set_status(persona_name, state="failed", finished_at=finished,
                        error="The model returned an unusable response.")
            logger.warning(f"Memory modify failed for '{persona_name}': unusable response")
        return True

    except Exception as e:
        finished = datetime.now(timezone.utc).isoformat()
        _set_status(persona_name, state="failed", finished_at=finished, error=str(e))
        logger.error(f"Memory modify failed for '{persona_name}': {e}")
        return True

    finally:
        lock.release()


def start_seed_update(persona_name, config, seed_content):
    """
    Start a background memory seed for a persona.

    Returns False if an update is already running for this persona.
    """
    lock = _get_persona_lock(persona_name)
    if lock.locked():
        return False

    thread = threading.Thread(
        target=_run_seed_update,
        args=(persona_name, config, seed_content),
        daemon=True,
    )
    thread.start()
    return True


def _run_seed_update(persona_name, config, seed_content):
    """Run a memory seed update. Acquires per-persona lock."""
    lock = _get_persona_lock(persona_name)
    if not lock.acquire(blocking=False):
        return False

    try:
        now = datetime.now(timezone.utc).isoformat()
        _set_status(persona_name, state="running", source="seed", started_at=now, error=None)
        logger.info(f"Memory seed started for '{persona_name}'")

        api_key = config.get("OPENROUTER_API_KEY")
        site_url = config.get("SITE_URL")
        site_name = config.get("SITE_NAME")
        personas_dir = str(django_settings.PERSONAS_DIR)

        persona_cfg = get_persona_config(persona_name, personas_dir)
        size_limit = persona_cfg.get('memory_size_limit', 8000)

        persona_dir = os.path.join(personas_dir, persona_name)
        persona_identity = get_persona_identity(persona_dir)
        memory_model = get_memory_model(config, persona_name, personas_dir)

        manager = MemoryManager(api_key, memory_model, site_url, site_name)
        success = manager.seed_memory(persona_name, persona_identity, seed_content, size_limit)

        finished = datetime.now(timezone.utc).isoformat()
        if success:
            _set_status(persona_name, state="completed", finished_at=finished)
            logger.info(f"Memory seed completed for '{persona_name}'")
        else:
            _set_status(persona_name, state="failed", finished_at=finished,
                        error="The model returned an unusable response.")
            logger.warning(f"Memory seed failed for '{persona_name}': unusable response")
        return True

    except Exception as e:
        finished = datetime.now(timezone.utc).isoformat()
        _set_status(persona_name, state="failed", finished_at=finished, error=str(e))
        logger.error(f"Memory seed failed for '{persona_name}': {e}")
        return True

    finally:
        lock.release()


# =============================================================================
# Auto-update scheduler
# =============================================================================

def _get_newest_session_mtime_for_persona(persona_name):
    """
    Find the newest session file mtime that belongs to a given persona.
    Uses a cache to avoid re-parsing JSON on every tick.
    """
    sessions_dir = django_settings.SESSIONS_DIR
    if not os.path.exists(sessions_dir):
        return None

    newest_mtime = None

    with _session_cache_lock:
        current_files = set()
        for filename in os.listdir(sessions_dir):
            if not filename.endswith('.json'):
                continue
            current_files.add(filename)

            filepath = os.path.join(sessions_dir, filename)
            try:
                file_mtime = os.path.getmtime(filepath)
            except OSError:
                continue

            # Check cache
            cached = _session_cache.get(filename)
            if cached and cached[0] == file_mtime:
                session_persona = cached[1]
            else:
                # Parse persona from session file
                try:
                    with open(filepath, 'r') as f:
                        data = json.load(f)
                    session_persona = data.get('persona', 'assistant')
                except (json.JSONDecodeError, OSError):
                    session_persona = 'assistant'
                _session_cache[filename] = (file_mtime, session_persona)

            if session_persona == persona_name:
                if newest_mtime is None or file_mtime > newest_mtime:
                    newest_mtime = file_mtime

        # Prune cache entries for deleted session files
        stale = set(_session_cache.keys()) - current_files
        for key in stale:
            del _session_cache[key]

    return newest_mtime


def _auto_update_loop(stop_event):
    """Main loop for the auto-update scheduler daemon thread.

    Each persona respects its own auto_memory_interval independently.
    A persona fires only when: (a) its own interval has elapsed since its
    last fire, and (b) there is new session activity since its last memory
    update. The loop sleeps until the soonest next-due persona.
    """
    while not stop_event.is_set():
        try:
            config = load_config()
        except Exception:
            _sleep_interruptible(60, stop_event)
            continue

        if not config:
            _sleep_interruptible(60, stop_event)
            continue

        personas_dir = str(django_settings.PERSONAS_DIR)
        personas = get_available_personas(personas_dir)

        now = time.time()
        next_due_times = []

        for persona_name in personas:
            if stop_event.is_set():
                return

            persona_cfg = get_persona_config(persona_name, personas_dir)
            interval_min = persona_cfg.get('auto_memory_interval', 0)
            if interval_min <= 0:
                continue  # Auto-update disabled for this persona

            interval_sec = max(5, min(1440, interval_min)) * 60

            # Respect per-persona interval: skip if not yet due
            due_at = _next_fire_time.get(persona_name, 0)
            if now < due_at:
                next_due_times.append(due_at)
                continue

            # Interval has elapsed — check for new activity
            newest_session_mtime = _get_newest_session_mtime_for_persona(persona_name)
            if newest_session_mtime is None:
                _next_fire_time[persona_name] = now + interval_sec
                next_due_times.append(_next_fire_time[persona_name])
                continue

            memory_file = get_memory_file(persona_name)
            if memory_file.exists():
                memory_mtime = os.path.getmtime(memory_file)
                if newest_session_mtime < memory_mtime:
                    # No new activity — defer by this persona's interval
                    _next_fire_time[persona_name] = now + interval_sec
                    next_due_times.append(_next_fire_time[persona_name])
                    continue

            # Due and new activity — fire
            fired = run_memory_update(persona_name, config, source="auto")
            if fired:
                _next_fire_time[persona_name] = time.time() + interval_sec
            else:
                # Lock held (e.g. manual update in progress) — retry soon
                _next_fire_time[persona_name] = time.time() + 60
            next_due_times.append(_next_fire_time[persona_name])

        # Sleep until the soonest next-due persona (floor 10s, default 60s)
        if next_due_times:
            sleep_seconds = max(10, min(next_due_times) - time.time())
        else:
            sleep_seconds = 60

        _sleep_interruptible(sleep_seconds, stop_event)


def _sleep_interruptible(total_seconds, stop_event):
    """Sleep in 10-second chunks so we can respond to stop_event promptly."""
    elapsed = 0
    while elapsed < total_seconds and not stop_event.is_set():
        time.sleep(min(10, total_seconds - elapsed))
        elapsed += 10


def start_scheduler():
    """Start the auto-update scheduler daemon thread. Idempotent."""
    global _scheduler_thread, _scheduler_started

    with _scheduler_guard:
        if _scheduler_started:
            return
        _scheduler_started = True

    _scheduler_stop.clear()
    _scheduler_thread = threading.Thread(
        target=_auto_update_loop,
        args=(_scheduler_stop,),
        daemon=True,
    )
    _scheduler_thread.start()


def stop_scheduler():
    """Stop the auto-update scheduler."""
    global _scheduler_thread, _scheduler_started

    _scheduler_stop.set()
    if _scheduler_thread and _scheduler_thread.is_alive():
        _scheduler_thread.join(timeout=15)
    _scheduler_thread = None
    _next_fire_time.clear()

    with _scheduler_guard:
        _scheduler_started = False


# =============================================================================
# Thread memory update worker (per-session, manual-only in phase 2)
# =============================================================================

def run_thread_memory_update(session_id, config):
    """
    Run a thread-memory update for a single session. Acquires per-session lock.

    Returns True if an attempt ran (success or failure); False if the lock
    was already held.
    """
    lock = _get_session_lock(session_id)
    if not lock.acquire(blocking=False):
        return False

    try:
        now = datetime.now(timezone.utc).isoformat()
        _set_thread_status(session_id, state="running", source="manual",
                           started_at=now, error=None)
        logger.info(f"Thread memory update started for session '{session_id}'")

        session_data = load_session(session_id)
        if session_data is None:
            finished = datetime.now(timezone.utc).isoformat()
            _set_thread_status(session_id, state="failed", finished_at=finished,
                               error="Session not found.")
            return True

        messages = session_data.get('messages', [])
        existing_memory = session_data.get('thread_memory', '')
        updated_at = session_data.get('thread_memory_updated_at', '')

        new_messages = filter_new_messages(messages, updated_at)
        if not new_messages:
            finished = datetime.now(timezone.utc).isoformat()
            _set_thread_status(session_id, state="completed", finished_at=finished,
                               error="No new messages since last update.")
            logger.info(f"Thread memory update skipped for session '{session_id}': no new messages")
            return True

        persona_name = session_data.get('persona', 'assistant')
        persona_display = persona_name.replace('_', ' ').title()
        mode = session_data.get('mode', 'chatbot')

        api_key = config.get("OPENROUTER_API_KEY")
        site_url = config.get("SITE_URL")
        site_name = config.get("SITE_NAME")
        personas_dir = str(django_settings.PERSONAS_DIR)
        model = get_memory_model(config, persona_name, personas_dir)

        manager = ThreadMemoryManager(api_key, model, site_url, site_name)
        updated_memory = manager.merge(
            persona_display, existing_memory, new_messages,
            size_limit=DEFAULT_THREAD_MEMORY_SIZE,
            mode=mode,
        )

        finished = datetime.now(timezone.utc).isoformat()
        if updated_memory is None:
            _set_thread_status(session_id, state="failed", finished_at=finished,
                               error="The model returned an unusable response.")
            logger.warning(f"Thread memory update failed for '{session_id}': unusable response")
            return True

        save_thread_memory(session_id, updated_memory)
        _set_thread_status(session_id, state="completed", finished_at=finished)
        logger.info(f"Thread memory update completed for session '{session_id}'")
        return True

    except Exception as e:
        finished = datetime.now(timezone.utc).isoformat()
        _set_thread_status(session_id, state="failed", finished_at=finished, error=str(e))
        logger.error(f"Thread memory update failed for '{session_id}': {e}")
        return True

    finally:
        lock.release()


def start_thread_memory_update(session_id, config):
    """
    Start a background thread-memory update. Returns False if an update is
    already running for this session.
    """
    lock = _get_session_lock(session_id)
    if lock.locked():
        return False

    thread = threading.Thread(
        target=run_thread_memory_update,
        args=(session_id, config),
        daemon=True,
    )
    thread.start()
    return True
