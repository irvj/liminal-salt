# CLAUDE.md

Navigation + conventions for Claude working in this repo. Written for Claude, not humans. Keep it tight — feature descriptions belong in commits.

## What this is

Liminal Salt — Django 5 + HTMX + Alpine.js web chatbot on top of OpenRouter. No database; state lives in JSON files under `data/`. Single-process (waitress in prod, `runserver` in dev).

## Directory layout

```
liminal_salt/              Django project (settings, urls, wsgi)
chat/
  views/                   Thin HTTP handlers. No file I/O here.
    core.py, chat.py, memory.py, personas.py, settings.py, api.py
  services/                All file I/O, LLM calls, locks, caches
  templatetags/            |markdown, |display_name filters
  static/
    js/utils.js            Shared helpers (CSRF, URLs, themes, timestamps, drafts)
    js/components.js       Alpine components (Alpine.data(...) registrations)
    css/{input,output}.css Tailwind v4 source + compiled
    themes/*.json          Color themes (16 files)
  templates/
    base.html              Loads HTMX + Alpine, #app-urls, csrf-token meta
    chat/, persona/, memory/, settings/, setup/, components/, icons/
  urls.py                  App routes
  utils.py                 load_config, session listing, model list, theme list
data/                      Gitignored user state (entire folder)
  config.json              App config — API key, default model, default persona, theme
  sessions/session_*.json  Chat sessions
  personas/{name}/         identity.md + config.json (model override, memory settings, thread defaults)
  memory/{name}.md         Per-persona memory
  user_context/            Global + per-persona context files; local_directories config
```

## Services (`chat/services/`)

| File | Owns |
|---|---|
| `session_manager.py` | Session JSON CRUD. **All** session reads/writes go through here, under a per-session lock. Session-id validation regex lives here. |
| `chat_core.py` | `ChatCore` class. Builds LLM payload, calls `llm_client`, persists via `save_chat_history`. Does NOT do raw session file I/O. |
| `thread_memory_manager.py` | Per-session running summary via LLM merge. Distinct from persona memory. Two prompt variants (chatbot / roleplay). Settings resolver. |
| `memory_manager.py` | Per-persona memory file I/O + LLM merge/seed/modify. Voice: 2nd-person for persona, 3rd for user. |
| `memory_worker.py` | Two daemon schedulers (persona memory + thread memory), per-persona locks, status tracking, mtime-keyed caches. |
| `context_manager.py` | Assembles system prompt. Also owns persona config I/O and `get_available_personas`. |
| `context_files.py` | `ContextFileManager` for global + per-persona uploaded context files. |
| `local_context.py` | Local-directory context file scanning (live-read at prompt time). |
| `persona_manager.py` | Persona CRUD + side-effect orchestration (rename cascade, delete cleanup). |
| `llm_client.py` | `call_llm(...)` OpenRouter wrapper, `LLMError`. |
| `config_manager.py` | Model listing, API-key validation. |
| `summarizer.py` | Title generation. |

## Views (grouped)

- **chat.py**: chat/send/switch/new/start/delete/pin/rename/save-draft/retry/edit, plus session scenario + thread-memory update/status + thread-memory settings save/reset + fork-to-roleplay.
- **memory.py**: persona memory view, update/wipe/modify/seed, settings save, status polling, persona context file CRUD, local-directory endpoints.
- **personas.py**: persona settings view, save/create/delete, model override, thread defaults save/clear.
- **settings.py**: settings view, save, provider validation, per-provider model save, context history limit, global context file CRUD.
- **core.py**: index redirect, setup wizard.
- **api.py**: themes list, models list, theme save.

## URL conventions

- `/chat/*` — chat lifecycle
- `/session/*` — per-session operations (scenario, thread memory, fork)
- `/memory/*` — persona memory + its context files
- `/persona/*` — persona CRUD + persona context files
- `/settings/*` — app settings + global context files + persona thread defaults
- `/context/local/*` — local-directory context (shared by global + persona via optional `persona` param)
- `/api/*` — JSON endpoints (themes, models)

## Session JSON schema

Written by `session_manager.py`. Fields:

| Field | Type | Notes |
|---|---|---|
| `title` | str | "New Chat" on create; auto-generated once on first reply unless `title_locked`. |
| `title_locked` | bool | Set True by `rename_session` or the one-shot auto-gen. Absent until finalized. |
| `persona` | str | Persona folder name |
| `mode` | str | `"chatbot"` \| `"roleplay"`. Immutable after create. |
| `messages` | list | `[{role, content, timestamp}]` |
| `draft` | str | Absent until first save |
| `pinned` | bool | Absent until first pin |
| `scenario` | str | Roleplay threads only. Silently ignored if `mode=="chatbot"`. |
| `thread_memory` | str | Per-thread running summary |
| `thread_memory_updated_at` | str | Timestamp of last message included in the summary |
| `thread_memory_settings` | dict | Per-thread override `{interval_minutes, message_floor, size_limit}`. Absent when values match resolved defaults. |

## Non-obvious architectural invariants

**Timestamp canonical form.** Every message timestamp, `thread_memory_updated_at`, and any mtime-derived cutoff is `datetime.now(timezone.utc).isoformat(timespec='microseconds')` → `2026-04-18T12:34:56.123456+00:00`. Fixed-width, `fromisoformat()` round-trips, lexicographic compare matches chronological. Use `now_timestamp()` from `session_manager` — do not hand-roll with `strftime`.

**Per-session lock.** `session_manager._session_lock(session_id)` serializes every read + every RMW. `ChatCore._save_history` routes through `save_chat_history()` so chat-history writes share the lock with background writers. Holding a lock across an LLM call is forbidden — the thread-memory worker reads under the lock, releases, calls the LLM, then re-acquires for the write.

**Session-id validation.** Every public function in `session_manager` that takes `session_id` short-circuits on `_valid_session_id()` (regex `^session_\d{8}_\d{6}(?:_\d+)?\.json$`). Views can pass untrusted POST input straight through — invalid ids surface as the function's normal not-found return.

**Context assembly order** (`context_manager.load_context`):
1. Persona identity `.md` files
2. Persona-scoped context files (uploaded + local dirs)
3. Global user context files (uploaded + local dirs)
4. Scenario — roleplay threads only
5. Thread memory — per-thread running summary
6. Persona memory — chatbot threads only (suppressed in roleplay for immersion)

**Thread memory ≠ persona memory.** Thread memory is per-session, stored inline on the session JSON, written by `ThreadMemoryManager`. Persona memory is cross-thread, stored in `data/memory/{persona}.md`, written by `MemoryManager`. Roleplay sessions are excluded from persona-memory aggregation and don't receive persona memory in their prompt.

**Settings resolver pattern.** `resolve_thread_memory_settings(session_data, persona_config)` merges per-thread override → persona default (`default_thread_memory_settings`) → global fallback. Same pattern for `default_mode`: only `"roleplay"` is persisted as a persona override; `"chatbot"` is the unwritten baseline. Save endpoints compare submitted values to resolved defaults and clear the override rather than persist no-ops.

**Schedulers + caches.** Two daemons in `memory_worker.py`: persona-memory loop (per-persona interval + message floor) and thread-memory loop (per-session effective settings). Both cache session-derived scheduler inputs keyed by mtime (`_session_cache`, `_thread_scheduler_cache`, `_persona_count_cache`) — never re-parse JSON when mtime is unchanged. Caches prune deleted sessions each sweep and clear on `stop_scheduler()`.

**ChatCore doesn't own the file.** `ChatCore._load_history` calls `session_manager.load_session(session_id)`; `_save_history` calls `save_chat_history(session_id, title, persona, messages)`. Chat-owned fields (title, persona, messages) are written; every other field (mode, scenario, thread_memory, thread_memory_updated_at, thread_memory_settings, pinned, draft) is preserved by the service's RMW.

## Separation of concerns — hard rules

Drift here breaks every other invariant (locks bypassed, ids unvalidated, fields clobbered). These are not suggestions.

**Ownership is exclusive.** Each file/directory under `data/` has exactly one service that writes it. Reads may also go through that service (for lock/validation); they MUST if the caller might run concurrently with a writer.

| Resource | Sole writer |
|---|---|
| `data/sessions/*.json` | `session_manager.py` |
| `data/personas/{name}/` (dir + identity) | `persona_manager.py` |
| `data/personas/{name}/config.json` | `context_manager.save_persona_config` |
| `data/memory/{name}.md` | `memory_manager.py` |
| `data/user_context/**` (uploaded) | `context_files.py` |
| `data/config.json` | `utils.save_config` |

If you find yourself wanting to write one of these from somewhere else, stop. Add a function to the owner instead.

**Views do not do work.** Views parse requests, call a service, render a response. That's it. Forbidden in `chat/views/`: `open()`, `json.load/dump`, `os.remove`, `os.path.exists` on data paths, `shutil.*`, direct string manipulation of session/persona/memory files, direct LLM calls. If a view needs something that's not in services yet, add it to the appropriate service — don't inline it.

**Services do not cross domains.** `MemoryManager` does not read session JSON (that's `SessionManager`'s job — inject the messages). `ThreadMemoryManager` does not write files (it returns the merged text; the worker calls `SessionManager.save_thread_memory` to persist). `ChatCore` does not touch session JSON directly (it calls `load_session` / `save_chat_history`). Cross-domain reads go through the owner's public API, not its private helpers.

**`llm_client.call_llm` is the only path to OpenRouter.** No service imports `requests` to hit the API directly. New LLM features add a method to the appropriate manager (`MemoryManager`, `ThreadMemoryManager`, `Summarizer`, etc.) that calls `call_llm`.

**Frontend mirrors the backend split.**
- `utils.js` = shared utility functions (CSRF, URLs, DOM helpers, timestamp/draft/scroll). No Alpine components.
- `components.js` = Alpine component definitions only. No top-level helpers.
- Templates = markup + Alpine directives + data. No business logic, no inline handlers, no inline styles. If a template has a `<script>` block longer than one init call, extract it.

**Drift checklist — ask before writing code.**
1. Which service owns the resource I'm touching?
2. Does the function I need already exist there? If not, am I adding it there or smuggling it elsewhere?
3. If I'm in a view: am I just parsing/calling/rendering? If there's a loop or a condition with business meaning, it belongs in a service.
4. If I'm in a service: am I reaching into another service's data? Use its public API instead.
5. If I'm in a template: am I doing anything that isn't "render this data" or "dispatch an event"?

When in doubt, the correct move is almost always "add a method to the owning service, call it from the caller." Never "quick fix inline, refactor later."

## Code standards

**Python.** Views are thin. No `open()` / `json.load` / `os.remove` / `shutil.*` in `chat/views/`. `@require_POST` on every POST-only view; `@require_http_methods(["GET", "POST"])` for wizards. Catch specific exceptions — no bare `except`. All session writes use `flush() + fsync()`. Timezone handling: store UTC, display in user's zone (user tz lives in Django session under `user_timezone`).

**JavaScript.** Only two files: `utils.js` (shared functions, runs at load) and `components.js` (Alpine `Alpine.data()` registrations inside an `alpine:init` listener). No inline `<script>` with business logic — minimal init calls (`initMemoryView()`) are OK in HTMX partials. Read `data-*` attributes in `init()` and store as instance properties (`this._saveUrl = this.$el.dataset.saveUrl`) — `this.$el` may not point at the component root inside event-handler methods. Use `getCsrfToken()` for CSRF, `getAppUrl(key, fallback)` for named Django routes. `async/await` + `try/catch` everywhere; no `.then()` chains. Modal cross-talk via `window.dispatchEvent(new CustomEvent(...))` and `window.addEventListener` in `init()` — no `window.*` refs to components.

**Templates.** No `onclick`/`onchange`/`oninput` — use Alpine `@click`/`@change`/`@input`. No `style=` — use Tailwind classes or `hidden` / `x-show`. Data flows to Alpine via `data-*` attributes; lists/objects go as JSON in `data-foo='{{ foo_json|safe }}'`. HTMX-swapped partials can set a hidden `<div id="…" data-…>` as a stable data source for modals that live outside the swap region.

## Dev commands

```bash
npm run dev                    # Tailwind watcher + Django server concurrently
.venv/bin/python3 manage.py check    # Django config sanity
.venv/bin/python3 manage.py runserver
node --check chat/static/js/components.js    # JS syntax check
```

Port 8420. Setup wizard at `/setup/` on first launch.

## When touching this repo

- New session field? Add it to the schema table above and make sure `ChatCore._save_history` doesn't clobber it (it shouldn't — it RMWs — but confirm).
- New POST endpoint that takes a `session_id`? Route through a `session_manager` writer so validation + locking apply. Don't build paths from `get_session_path(session_id)` in views.
- Writing a timestamp? `now_timestamp()`. Comparing? String compare works because the format is fixed-width UTC.
- New Alpine component? Register in `components.js` under `alpine:init`. Template passes config via `data-*`; component stores those in `init()`.
- New persona-config key? Add it to `resolve_*` if it has a global fallback + persona-override shape, and clear it on save when submitted equals default.
