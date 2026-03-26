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

# Cache for session persona lookups: {filename: (mtime, persona)}
_session_cache = {}
_session_cache_lock = threading.Lock()


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
    """Main loop for the auto-update scheduler daemon thread."""
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

        # Find the shortest active auto-update interval across all personas
        min_interval = None
        for persona_name in personas:
            if stop_event.is_set():
                return

            persona_cfg = get_persona_config(persona_name, personas_dir)
            interval = persona_cfg.get('auto_memory_interval', 0)
            if interval <= 0:
                continue  # Auto-update disabled for this persona

            interval = max(5, min(1440, interval))

            # Check if there's new activity since last memory update
            newest_session_mtime = _get_newest_session_mtime_for_persona(persona_name)
            if newest_session_mtime is None:
                continue

            memory_file = get_memory_file(persona_name)
            if memory_file.exists():
                memory_mtime = os.path.getmtime(memory_file)
                if newest_session_mtime < memory_mtime:
                    # No new activity, but still track interval for sleep
                    if min_interval is None or interval < min_interval:
                        min_interval = interval
                    continue

            # New activity detected — run update
            run_memory_update(persona_name, config, source="auto")
            if min_interval is None or interval < min_interval:
                min_interval = interval

        # Sleep for the shortest active interval, or 60s if none are active
        sleep_minutes = min_interval if min_interval else 1
        _sleep_interruptible(sleep_minutes * 60, stop_event)


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

    with _scheduler_guard:
        _scheduler_started = False
