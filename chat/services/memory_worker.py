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
from .session_manager import load_session, save_thread_memory, now_timestamp
from .thread_memory_manager import (
    ThreadMemoryManager, filter_new_messages,
    DEFAULT_THREAD_MEMORY_SIZE, resolve_thread_memory_settings,
)


# =============================================================================
# Module-level state
# =============================================================================

_global_lock = threading.Lock()
_persona_locks = {}

_status_lock = threading.Lock()
_update_status = {}

_scheduler_thread = None
_thread_memory_scheduler_thread = None
_scheduler_stop = threading.Event()
_scheduler_started = False
_scheduler_guard = threading.Lock()

# Default minimum number of new messages across a persona's non-roleplay
# sessions before an auto-update fires. Paired with `auto_memory_interval`
# to form the unified trigger shape (interval AND floor).
DEFAULT_AUTO_MEMORY_MESSAGE_FLOOR = 10

# Next-due epoch time per persona for auto-update scheduling.
# Only read/written by the scheduler thread, so no lock needed.
_next_fire_time = {}

# Cache for session persona lookups: {filename: (mtime, persona)}
_session_cache = {}
_session_cache_lock = threading.Lock()

# Cache for persona message-floor counting: {filename: {mtime, persona, mode,
# timestamps, missing}}. Scheduler-thread only — no lock needed.
_persona_count_cache = {}

# =============================================================================
# Thread memory (per-session) state
# =============================================================================

_session_locks_guard = threading.Lock()
_session_locks = {}

_thread_status_lock = threading.Lock()
_thread_status = {}

# Next-due epoch time per session for thread-memory auto-update scheduling.
# Only read/written by the thread-memory scheduler, so no lock needed.
_thread_next_fire_time = {}

# Cache of session-derived scheduler inputs keyed by filename.
# Value: {mtime, persona, updated_at, new_message_count, thread_memory_settings}.
# Invalidated whenever the session file's mtime changes (which covers every
# in-app write, since _write_session truncates and rewrites).
_thread_scheduler_cache = {}


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

def _count_new_messages_for_persona(persona_name, since_mtime):
    """
    Count messages across this persona's non-roleplay sessions whose ISO
    timestamps are newer than `since_mtime` (epoch seconds). Used by the
    scheduler to gate auto-update firings on the message-floor threshold.

    If `since_mtime` is 0 (no existing memory file), every message counts
    as new. Per-session data is cached by mtime so unchanged session files
    don't get re-parsed on each call.
    """
    sessions_dir = django_settings.SESSIONS_DIR
    if not os.path.exists(sessions_dir):
        return 0

    since_iso = ""
    if since_mtime > 0:
        since_iso = datetime.fromtimestamp(since_mtime, tz=timezone.utc).isoformat(timespec='microseconds')

    count = 0
    live_filenames = set()
    for filename in os.listdir(sessions_dir):
        if not filename.endswith('.json'):
            continue
        filepath = os.path.join(sessions_dir, filename)
        try:
            file_mtime = os.path.getmtime(filepath)
        except OSError:
            continue
        live_filenames.add(filename)

        cached = _persona_count_cache.get(filename)
        if cached and cached['mtime'] == file_mtime:
            persona = cached['persona']
            mode = cached['mode']
            timestamps = cached['timestamps']
            missing = cached['missing']
        else:
            try:
                with open(filepath, 'r') as f:
                    data = json.load(f)
            except (json.JSONDecodeError, OSError):
                continue
            if not isinstance(data, dict):
                continue
            persona = data.get('persona', 'assistant')
            mode = data.get('mode', 'chatbot')
            timestamps = []
            missing = 0
            for msg in data.get('messages', []):
                ts = msg.get('timestamp', '')
                if ts:
                    timestamps.append(ts)
                else:
                    missing += 1
            _persona_count_cache[filename] = {
                'mtime': file_mtime,
                'persona': persona,
                'mode': mode,
                'timestamps': timestamps,
                'missing': missing,
            }

        if persona != persona_name:
            continue
        if mode == 'roleplay':
            continue

        if not since_iso:
            count += len(timestamps) + missing
        else:
            count += sum(1 for ts in timestamps if ts > since_iso)
            if missing:
                logger.warning(
                    "_count_new_messages_for_persona: %d message(s) without timestamp in %s, counting as new",
                    missing, filename,
                )
                count += missing

    stale = set(_persona_count_cache.keys()) - live_filenames
    for key in stale:
        del _persona_count_cache[key]

    return count


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
            message_floor = persona_cfg.get(
                'auto_memory_message_floor',
                DEFAULT_AUTO_MEMORY_MESSAGE_FLOOR,
            )

            # Respect per-persona interval: skip if not yet due
            due_at = _next_fire_time.get(persona_name, 0)
            if now < due_at:
                next_due_times.append(due_at)
                continue

            # Interval has elapsed — check for sufficient new activity
            memory_file = get_memory_file(persona_name)
            memory_mtime = os.path.getmtime(memory_file) if memory_file.exists() else 0
            new_message_count = _count_new_messages_for_persona(persona_name, memory_mtime)
            if new_message_count < message_floor:
                # Not enough new activity yet — defer by this persona's interval
                _next_fire_time[persona_name] = now + interval_sec
                next_due_times.append(_next_fire_time[persona_name])
                continue

            # Due and enough new activity — fire
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
    """Start the persona-memory and thread-memory scheduler daemons. Idempotent."""
    global _scheduler_thread, _thread_memory_scheduler_thread, _scheduler_started

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

    _thread_memory_scheduler_thread = threading.Thread(
        target=_thread_memory_auto_update_loop,
        args=(_scheduler_stop,),
        daemon=True,
    )
    _thread_memory_scheduler_thread.start()


def stop_scheduler():
    """Stop both auto-update schedulers."""
    global _scheduler_thread, _thread_memory_scheduler_thread, _scheduler_started

    _scheduler_stop.set()
    if _scheduler_thread and _scheduler_thread.is_alive():
        _scheduler_thread.join(timeout=15)
    if _thread_memory_scheduler_thread and _thread_memory_scheduler_thread.is_alive():
        _thread_memory_scheduler_thread.join(timeout=15)
    _scheduler_thread = None
    _thread_memory_scheduler_thread = None
    _next_fire_time.clear()
    _thread_next_fire_time.clear()
    _thread_scheduler_cache.clear()
    _persona_count_cache.clear()

    with _scheduler_guard:
        _scheduler_started = False


# =============================================================================
# Thread memory update worker (per-session, manual-only in phase 2)
# =============================================================================

def run_thread_memory_update(session_id, config, source="manual"):
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
        _set_thread_status(session_id, state="running", source=source,
                           started_at=now, error=None)
        logger.info(f"Thread memory update started for session '{session_id}' ({source})")

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

        # Record which message the summary covers up to. Using "now" here
        # would drop any messages written while the LLM was running, since
        # next-run filtering gates on timestamp > thread_memory_updated_at.
        summarized_through = new_messages[-1].get('timestamp') or updated_at or now_timestamp()

        persona_name = session_data.get('persona', 'assistant')
        persona_display = persona_name.replace('_', ' ').title()
        mode = session_data.get('mode', 'chatbot')

        personas_dir = str(django_settings.PERSONAS_DIR)
        persona_cfg = get_persona_config(persona_name, personas_dir)
        effective = resolve_thread_memory_settings(session_data, persona_cfg)

        api_key = config.get("OPENROUTER_API_KEY")
        site_url = config.get("SITE_URL")
        site_name = config.get("SITE_NAME")
        model = get_memory_model(config, persona_name, personas_dir)

        manager = ThreadMemoryManager(api_key, model, site_url, site_name)
        updated_memory = manager.merge(
            persona_display, existing_memory, new_messages,
            size_limit=effective.get('size_limit', DEFAULT_THREAD_MEMORY_SIZE),
            mode=mode,
        )

        finished = datetime.now(timezone.utc).isoformat()
        if updated_memory is None:
            _set_thread_status(session_id, state="failed", finished_at=finished,
                               error="The model returned an unusable response.")
            logger.warning(f"Thread memory update failed for '{session_id}': unusable response")
            return True

        save_thread_memory(session_id, updated_memory, summarized_through)
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


def reschedule_thread_next_fire(session_id, interval_minutes):
    """
    Re-anchor the scheduler's next-fire time for a session based on a new
    interval. Call this after settings are saved so the new interval kicks
    in cleanly — the next fire is `now + interval` with the new value,
    rather than either (a) waiting out the previously-scheduled interval
    or (b) firing immediately.

    If `interval_minutes <= 0`, the entry is removed entirely (auto disabled).
    """
    if interval_minutes and interval_minutes > 0:
        interval_sec = max(5, min(1440, int(interval_minutes))) * 60
        _thread_next_fire_time[session_id] = time.time() + interval_sec
    else:
        _thread_next_fire_time.pop(session_id, None)


def start_thread_memory_update(session_id, config, source="manual"):
    """
    Start a background thread-memory update. Returns False if an update is
    already running for this session.
    """
    lock = _get_session_lock(session_id)
    if lock.locked():
        return False

    thread = threading.Thread(
        target=run_thread_memory_update,
        args=(session_id, config, source),
        daemon=True,
    )
    thread.start()
    return True


# =============================================================================
# Thread memory auto-update scheduler
# =============================================================================

def _get_cached_scheduler_view(filename, filepath):
    """
    Return cached scheduler-relevant fields for a session, reparsing JSON
    only when the file's mtime has changed. Returns None if the file is
    unreadable or not a dict. The returned dict contains `persona`,
    `updated_at`, `new_message_count`, and `thread_memory_settings`;
    effective settings are resolved at each tick (cheap) because persona
    config changes don't bump session mtime.
    """
    try:
        file_mtime = os.path.getmtime(filepath)
    except OSError:
        return None

    cached = _thread_scheduler_cache.get(filename)
    if cached and cached['mtime'] == file_mtime:
        return cached

    try:
        with open(filepath, 'r') as f:
            session_data = json.load(f)
    except (json.JSONDecodeError, OSError):
        return None
    if not isinstance(session_data, dict):
        return None

    updated_at = session_data.get('thread_memory_updated_at', '')
    new_messages = filter_new_messages(session_data.get('messages', []), updated_at)
    entry = {
        'mtime': file_mtime,
        'persona': session_data.get('persona', 'assistant'),
        'thread_memory_settings': session_data.get('thread_memory_settings') or {},
        'updated_at': updated_at,
        'new_message_count': len(new_messages),
    }
    _thread_scheduler_cache[filename] = entry
    return entry


def _thread_memory_auto_update_loop(stop_event):
    """Main loop for the per-session thread-memory scheduler daemon thread.

    Each session respects its own effective settings (per-thread override →
    persona default → global fallback). A session fires only when:
    (a) its own interval has elapsed since its last fire, and
    (b) the unsummarized message count ≥ effective `message_floor`.
    The loop sleeps until the soonest next-due session.

    Updates are dispatched asynchronously via start_thread_memory_update so
    a slow LLM call doesn't block checks for other sessions.
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
        sessions_dir = django_settings.SESSIONS_DIR

        if not os.path.exists(sessions_dir):
            _sleep_interruptible(60, stop_event)
            continue

        now = time.time()
        next_due_times = []
        live_sessions = set()

        for filename in os.listdir(sessions_dir):
            if stop_event.is_set():
                return
            if not filename.endswith('.json'):
                continue

            filepath = os.path.join(sessions_dir, filename)
            view = _get_cached_scheduler_view(filename, filepath)
            if view is None:
                continue

            live_sessions.add(filename)

            persona_cfg = get_persona_config(view['persona'], personas_dir)
            effective = resolve_thread_memory_settings(
                {'thread_memory_settings': view['thread_memory_settings']},
                persona_cfg,
            )

            interval_min = effective.get('interval_minutes', 0)
            if interval_min <= 0:
                continue  # auto disabled for this thread

            interval_sec = max(5, min(1440, interval_min)) * 60
            message_floor = effective.get('message_floor', 0)

            due_at = _thread_next_fire_time.get(filename, 0)
            if now < due_at:
                next_due_times.append(due_at)
                continue

            if view['new_message_count'] < message_floor:
                _thread_next_fire_time[filename] = now + interval_sec
                next_due_times.append(_thread_next_fire_time[filename])
                continue

            fired = start_thread_memory_update(filename, config, source="auto")
            if fired:
                _thread_next_fire_time[filename] = time.time() + interval_sec
            else:
                # Lock held (manual/auto already running) — retry soon
                _thread_next_fire_time[filename] = time.time() + 60
            next_due_times.append(_thread_next_fire_time[filename])

        # Prune stale entries for deleted sessions
        stale = set(_thread_next_fire_time.keys()) - live_sessions
        for key in stale:
            del _thread_next_fire_time[key]
        cache_stale = set(_thread_scheduler_cache.keys()) - live_sessions
        for key in cache_stale:
            del _thread_scheduler_cache[key]

        if next_due_times:
            sleep_seconds = max(10, min(next_due_times) - time.time())
        else:
            sleep_seconds = 60

        _sleep_interruptible(sleep_seconds, stop_event)
