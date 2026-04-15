# Audit Fixes

Findings from code audit — April 14, 2026.

---

## Security

### XSS in error message rendering
- `chat/static/js/utils.js:985` — `innerHTML` with `${errorMessage}` interpolation. Error messages from the server are inserted without escaping.
- **Fix:** Use `textContent` or create DOM elements programmatically instead of innerHTML.

### Inline onclick handlers in dynamically generated HTML
- `chat/static/js/utils.js:1213-1215` — Edit UI generates `<span onclick="cancelEdit()">` and `<span onclick="saveEditedMessage()">` via innerHTML.
- **Fix:** Use event delegation or create elements with `addEventListener`.

### Missing persona name validation before path join
- `chat/views/chat.py:43` — `session_persona` is used in `os.path.join()` without explicit validation.
- **Fix:** Add `persona_exists()` check or use `_safe_persona_name()` before constructing the path.

---

## Data Durability

### Missing flush + fsync on file writes
Several write paths skip the `flush() + fsync()` pattern used elsewhere in the codebase:
- `chat/services/context_files.py:59-60` — `save_config()` writes JSON without fsync
- `chat/services/context_files.py:100-102` — `upload_file()` writes binary data without fsync
- `chat/services/context_files.py:190-191` — `save_file_content()` writes content without fsync
- `chat/services/chat_core.py:47-48` — `_save_history()` dumps session JSON without fsync
- **Fix:** Add `f.flush()` and `os.fsync(f.fileno())` to all four locations. Consider extracting a shared `safe_write()` utility to avoid repeating this pattern.

---

## Error Handling

### Broad except clauses in services
8 instances of `except Exception` that swallow errors silently:
- `chat/services/chat_core.py:34` — `_load_history()` returns `[]` on any error
- `chat/services/chat_core.py:49` — `_save_history()` prints instead of logging
- `chat/services/chat_core.py:69,82,96,103,111` — Multiple broad catches in `_get_payload_messages()`
- `chat/services/summarizer.py:80` — `generate_title()` suppresses all errors
- `chat/services/local_context.py:251` — Returns `None` with no logging
- `chat/services/memory_worker.py:356` — Silently ignores config load errors
- **Fix:** Replace with specific exception types (`json.JSONDecodeError`, `IOError`, `ValueError`, `requests.exceptions.RequestException`). Add `logger.error()` or `logger.warning()` to each.

### Inconsistent logging
- `chat/services/chat_core.py:50` and `chat/services/memory_manager.py:203` use `print()` instead of `logger`.
- **Fix:** Replace with `logger.error()` to match the rest of the codebase.

### Inconsistent error response formats
Views return errors as JSON, HTML strings, or plain text depending on the endpoint. No standard pattern.
- **Fix:** Create helper functions like `json_error(message, status)` and `html_error(message, status)` as noted in the existing backlog.

---

## Thread Safety

### Lock check race condition
- `chat/services/memory_worker.py:154-156` — Checks `lock.locked()` then tries to acquire, but another thread could acquire between the check and the attempt.
- **Fix:** Use `lock.acquire(blocking=False)` directly (the correct pattern is already used at line 86 in the same file).

---

## JavaScript

### Hardcoded URLs bypassing getAppUrl()
- `chat/static/js/components.js:728,740,767,797,846,869` — `/context/local/*` endpoints are hardcoded strings instead of using `getAppUrl()` or `data-*` attributes.
- **Fix:** Add these URLs to `#app-urls` in `base.html` and read them via `getAppUrl()` or component `data-*` attributes.

### Legacy .then() chains
6 instances of `.then()` instead of async/await:
- `chat/static/js/utils.js:727-750` — `pollMemoryUpdateStatus()`
- `chat/static/js/utils.js:1244-1258` — `saveEditedMessage()`
- `chat/static/js/components.js:1187` — Settings preview
- **Fix:** Refactor to async/await with try/catch.

---

## Templates & Accessibility

### Modals missing ARIA attributes
All modals lack `role="dialog"`, `aria-modal="true"`, `aria-labelledby`, and focus trapping.
- **Fix:** Add ARIA attributes and implement focus trapping on modal open.

### Extract inline modals from chat.html
8 modal definitions are inline in `chat.html`. Should be extracted to `templates/modals/` partials.

### Missing label elements
- Textarea in `chat_main.html` and input in `chat_home.html` lack associated `<label>` elements.

---

## Testing

### No automated tests exist
No test files, test directories, or test configuration anywhere in the project.
- **Fix:** Start with service-layer unit tests (`SessionManager`, `PersonaManager`, `ContextFileManager`) since they have clear inputs/outputs and no LLM dependency. Add view-layer integration tests next.

---

## Service Layer

### chat_core.py does its own file I/O
`ChatCore._save_history()` and `_load_history()` read/write session JSON directly instead of delegating to `SessionManager`.
- **Fix:** Delegate to `SessionManager` for consistency with the rest of the codebase.
