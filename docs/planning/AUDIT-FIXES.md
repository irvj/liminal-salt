# Audit Fixes

Originally filed 2026-04-14. Last reconciled against the tree on 2026-04-18 after the thread-memory / roleplay audit pass (commits since `v0.9.3`).

---

## Closed in the 2026-04-18 pass

Recording these so they don't get re-filed.

- **`ChatCore` did its own session-file I/O.** `_load_history` / `_save_history` now route through `session_manager.load_session` and `save_chat_history`; the service owns the RMW and the per-session lock.
- **`ChatCore._save_history` skipped fsync.** Now uses `session_manager._write_session` which does `flush + fsync`.
- **Broad `except Exception` in `chat_core._load_history` / `_save_history`.** Gone with the rewrite — those methods are now thin calls into the service.
- **Session timestamp format drift.** Unified on `datetime.now(timezone.utc).isoformat(timespec='microseconds')` (`now_timestamp()` in `session_manager.py`). Single canonical form across message timestamps, `thread_memory_updated_at`, and scheduler mtime cutoffs.
- **Session-id path traversal.** `session_manager._valid_session_id` regex-gates every public entry point.

---

## Security / hardening

**`innerHTML` interpolation in error rendering** — `chat/static/js/utils.js:985` (`handleMessageError`). `errorDiv.innerHTML = \`...${errorMessage}...\`` interpolates text unescaped. Not actively exploitable today — `errorMessage` is built from static strings, `xhr.status` (number), and `xhr.statusText` (Django never returns HTML-bearing statusText). It's a convention violation (the "no innerHTML with interpolation" pattern in CLAUDE.md) and becomes live if someone later extends the message to include server-returned error bodies.
- Fix: build the node with `document.createElement` and set `textContent`.

**Inline `onclick` attributes in edit UI** — `chat/static/js/utils.js:1223,1225` (`editLastMessage`). Injects `<span onclick="cancelEdit()">` / `<span onclick="saveEditedMessage()">` via innerHTML. Contradicts the "no inline handlers" convention in CLAUDE.md.
- Fix: create the elements and attach `addEventListener`, or render them from the template and toggle with Alpine.

**Persona path join without explicit validation in views** — `chat/views/chat.py:52` (`_build_chat_core`): `os.path.join(str(PERSONAS_DIR), session_persona)`. `session_persona` originates from request-side state. Persona names are validated at creation (`persona_manager._validate_persona_name`), but nothing stops a hand-edited session JSON from re-surfacing a bad value here. Defense-in-depth — only exploitable by someone who already has filesystem write access to `data/sessions/`, but the validation belongs in one place.
- Fix: call `persona_exists(session_persona)` before building the path, or route the directory resolution through `persona_manager` so validation happens in one place.

---

## Data durability

**`context_files.py` writes skip `flush + fsync`.** The rest of the codebase (`session_manager`, `memory_manager`, `utils.save_config`) uses the durability pattern; these don't:
- `save_config` at `chat/services/context_files.py:59-60`
- `upload_file` at `chat/services/context_files.py:100-108` (neither the binary write nor the subsequent `save_config` call)
- `save_file_content` at `chat/services/context_files.py:189-192`

Fix: wrap the writes in the same `flush() + os.fsync(f.fileno())` pattern. Worth extracting a `safe_write_json(path, data)` / `safe_write_bytes(path, data)` helper — `session_manager._write_session` already has the shape.

---

## Error handling & logging

**`print()` in place of `logger`.**
- `chat/services/chat_core.py:132,143,147` — retry attempt messages in `send_message`. Switch to `logger.info` / `logger.warning`.
- `chat/services/memory_manager.py:217` — `print("Error updating memory for ...")` inside `_merge_memory`. Switch to `logger.error(...)` and keep the exception.

**Remaining broad `except Exception` that swallow specifics.**
- `chat/services/chat_core.py` `_get_payload_messages` (around lines 62, 75, 87, 95, 104) — catches on `ZoneInfo(...)` and `datetime.fromisoformat(...)`. Narrow to `except (KeyError, ValueError)`.
- `chat/services/summarizer.py:80` — `generate_title` suppresses every error with a fallback. Narrow to `LLMError` + the specific parsing errors; let unknown exceptions propagate.
- `chat/services/local_context.py:251` — silently returns `None` for any read failure. Keep the fallback but narrow to `(OSError, UnicodeDecodeError)` and log at `warning`.
- `chat/services/memory_worker.py` `_auto_update_loop` / `_thread_memory_auto_update_loop` top-level `try: config = load_config(); except Exception` — these are daemon-loop guards and are intentionally broad, but the bare-`Exception` catches in `run_memory_update`, `_run_modify_update`, `_run_seed_update`, `run_thread_memory_update` could narrow to `(LLMError, OSError, json.JSONDecodeError)` and leave programmer errors to surface.

**Inconsistent error response shapes across views.** Some return `HttpResponse(status=...)`, some `JsonResponse({'error': ...}, status=...)`, some plain text.
- Fix: add `json_error(message, status)` / `html_error(message, status)` helpers and use them uniformly. Low priority; cosmetic.

---

## Thread hygiene

**Wasted thread spawn on `lock.locked()` check.** In `memory_worker.py`, every `start_*` wrapper does `if lock.locked(): return False` before spawning the worker thread. If another thread acquires between the check and the spawn, the spawned worker's own `acquire(blocking=False)` returns False and the task no-ops cleanly. Outcome is correct — this is a wasted thread, not a correctness bug. Pattern in `start_manual_update`, `start_modify_update`, `start_seed_update`, `start_thread_memory_update`.
- Fix: drop the pre-check. The worker functions already test-acquire and return cleanly on contention.

---

## JavaScript

**Hardcoded URLs bypassing `getAppUrl()`.** `chat/static/js/components.js:1204, 1234, 1260, 1306` — `/context/local/add/`, `/context/local/remove/`, `/context/local/toggle/`, `/context/local/refresh/`.
- Fix: expose via `#app-urls` in `base.html` and read through `getAppUrl(...)`, or pass as component `data-*` attributes (the pattern CLAUDE.md calls out).

**Legacy `.then()` chains.**
- `chat/static/js/utils.js:726-753` — `pollMemoryUpdateStatus` (two chained `.then` and a `.catch`).
- `chat/static/js/utils.js:1254-1268` — `saveEditedMessage` (`.then` + `.catch`).

Fix: convert to `async/await` + `try/catch` to match the convention.

---

## Accessibility / templates

**Modals lack ARIA + focus trap.** None of the modals in `chat/templates/chat/chat.html` set `role="dialog"`, `aria-modal="true"`, or `aria-labelledby`, and there's no focus trap on open.

**`chat.html` still carries all modals inline.** Eight `<div x-data="...Modal">` blocks; extract to `templates/modals/` and `{% include %}` for readability.

**Form inputs missing `<label>`.** Textarea in `chat_main.html` and input in `chat_home.html`.

**Sidebar session groups use `<div>` lists.** `chat/templates/chat/sidebar_sessions.html` — consider `<ul>`/`<li>` for semantic correctness / AT navigation.

---

## Testing

No test files or harness exist. Service layer (`session_manager`, `persona_manager`, `context_files`, `thread_memory_manager`, resolvers) has clear inputs and no LLM dependency — start there. View-layer integration tests next, with LLM calls stubbed.

---

## Priority (suggested)

1. XSS + inline-handler items (security, small diffs).
2. `context_files.py` fsync (durability; follows established pattern).
3. `print` → `logger` and narrow broad excepts (observability).
4. Accessibility pass (ARIA + labels) — one coordinated diff.
5. JS `.then` → `async/await` and URL-through-`getAppUrl` — single JS cleanup PR.
6. Thread-safety check-then-acquire and standard error-response helpers — low risk, low urgency.
7. Tests — long-running effort; seed with `session_manager` and `thread_memory_manager` resolvers.
