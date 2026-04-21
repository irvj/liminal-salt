# Liminal Salt — Architecture Roadmap

**Created:** April 14, 2026
**Updated:** April 21, 2026
**Status:** Rust migration in progress — Phase 0 and Phase 1 complete; Phase 2 up next
**Scope:** Python/Django → Rust (Axum + Tera) → Tauri desktop app

---

## Roadmap Overview

Two milestones, each independently useful:

| Milestone | What | Outcome |
|-----------|------|---------|
| **M1: Rust Backend** | Replace Python/Django with Rust (Axum + Tera). Drop all Python. | Same app, compiled backend, still browser-based |
| **M2: Tauri Desktop App** | Wrap the Rust backend in Tauri. In-process Axum, native webview. | Single native binary (~5–15MB) |

```
Current              M1                           M2
Django + Services →  Rust Backend             →   Tauri Desktop App
──────────────────   ─────────────                ─────────────────
Clean service layer  Axum + Tera                  Native window
Python 3.x + Django  reqwest → OpenRouter         In-process Axum
Browser access       tokio async                  Single binary
                     Browser access               ~5–15MB
```

---

## Project Constraints

These shape every decision in the plan.

- **No external users.** The app is public but pre-adoption. The author is the only user.
- **No backward-compatibility tax.** On-disk formats (session JSON shape, timestamp format, persona config layout, memory file layout) are **not** locked in. They can change. `data/` can be wiped between experiments. This removes the single hardest class of migration bug (silent format drift).
- **Correctness over format parity.** Goal is "the Rust app works correctly," not "the Rust app writes byte-identical files to the Python app." Build against Rust-native idioms (e.g., `chrono::to_rfc3339`), not Python-compat helpers.
- **The frontend is a fixed target.** HTMX + Alpine + Tailwind + all JS is unchanged by M1. This is enforced architecture: the server language must not leak into the browser.

---

## Current State (April 21, 2026)

### What's solid
- Service ownership is exclusive and enforced (see CLAUDE.md ownership table).
- Views are thin: no `open()`, `json.load/dump`, `shutil.*`, `os.remove`, or direct HTTP calls.
- Session ID validation + per-session locking centralized in `session_manager`.
- `llm_client.call_llm` is the only outbound path to OpenRouter.
- No bare `except:` anywhere.
- Templates have no inline handlers, no inline styles, no business logic.

### Minor drift to clean up before migrating
**Done 2026-04-21** (Phase 0). Kept below for historical context.

Not blockers, but simpler to fix in Python first than to port flawed behavior to Rust:

- **`chat/static/js/utils.js:726-753`** — memory-update status poll uses `fetch().then().then().catch()`. Convert to `async/await`.
- **`chat/static/js/utils.js:1156-1165`** — clipboard copy uses `.then().catch()`. Convert to `async/await`.
- **`chat/static/js/utils.js:1268-1285`** — `saveEditedMessage` uses `.then().catch()`. Convert to `async/await`.
- **`chat/static/js/utils.js:479`** — `editBtn.setAttribute('onclick', 'editLastMessage(this)')` dynamically attaches an inline handler. Replace with `addEventListener('click', ...)`, or (better) render the button in the template with Alpine `@click` so `utils.js` stops building this DOM fragment.
- **`chat/views/settings.py:212`** — `os.path.exists(django_settings.CONFIG_FILE)` diagnostic in a view. Move behind a `config_manager.config_file_exists()` helper so views stay free of `os.path` on data paths.

These are Phase 0 of the plan.

---

# Milestone 1: Rust Backend Migration

Replace the Python/Django backend with Rust while keeping the frontend completely unchanged. The app continues to run in the browser during this phase.

## Target Stack

| Concern | Django (current) | Rust (target) |
|---------|-----------------|---------------|
| Web framework | Django | Axum |
| Templating | Django templates | Tera (Jinja2-like) |
| HTTP client | `requests` | `reqwest` |
| JSON handling | `json` stdlib | `serde` + `serde_json` |
| Async / concurrency | `threading` (`memory_worker.py`) | `tokio` |
| Static files | whitenoise | `tower-http::ServeDir` |
| CSRF | Django middleware | `axum-csrf` or hand-rolled double-submit |
| Session state (wizard) | Django signed cookies | `tower-sessions` with cookie store |
| Markdown | `python-markdown` | `pulldown-cmark` |
| Server | waitress (WSGI) | hyper (built into Axum) |
| Time | `datetime` + `timezone.utc` | `chrono` (pick one; use it everywhere) |
| File durability | `flush() + os.fsync(fd.fileno())` | `File::sync_all()` after `write_all` |
| Locks | `threading.Lock` | `tokio::sync::Mutex` (async-aware) |

## Service Module Mapping

| Python | Rust module | Key types |
|--------|-------------|-----------|
| `session_manager.py` | `services/session.rs` | `SessionManager`, `Session`, `Message` |
| `chat_core.py` | `services/chat.rs` | `ChatCore` |
| `persona_manager.py` | `services/persona.rs` | `PersonaManager`, `Persona` |
| `context_files.py` | `services/context_files.rs` | `ContextFileManager` (generic over scope) |
| `local_context.py` | `services/local_context.rs` | `LocalContextScanner` |
| `context_manager.py` | `services/prompt.rs` | `PromptBuilder` |
| `memory_manager.py` | `services/memory.rs` | `MemoryManager` |
| `thread_memory_manager.py` | `services/thread_memory.rs` | `ThreadMemoryManager` |
| `memory_worker.py` | `services/memory_worker.rs` | Two `tokio::spawn` loops + per-persona/session locks |
| `llm_client.py` | `services/llm.rs` | `LlmClient`, `LlmError` |
| `summarizer.py` | `services/summarizer.rs` | `generate_title()` |
| `config_manager.py` + `utils.py` | `services/config.rs` | `AppConfig`, `is_app_ready()`, agreement-version parser |

### Notes on the mapping

- **`SessionManager`**: per-session `Arc<Mutex<()>>` in a `DashMap<String, Arc<Mutex<()>>>` (or `RwLock<HashMap<...>>` if DashMap isn't wanted). `session_id` validation regex ported verbatim. All public functions short-circuit on invalid IDs.
- **`PersonaManager.rename_persona`**: orchestrates 5 side effects (persona dir rename, memory file rename, session persona-field rewrite, user-context dir rename, persona-config-overrides update). In Rust this stays a single method; error handling via `Result<(), PersonaError>` and saga-style rollback if any step fails mid-way.
- **`ContextFileManager`**: Python parameterizes by `base_dir` and `scope_label`. In Rust, a plain struct with `base_dir: PathBuf` and `scope_label: &'static str` fields is cleaner than a generic.
- **`memory_worker.py`**: two schedulers (persona memory + thread memory). Port each to its own `tokio::spawn`ed task with a periodic `tokio::time::interval`. Per-persona/session locks use `tokio::sync::Mutex` so they don't block the runtime. **Do not hold a lock across an `.await` that calls the LLM** — this is an architectural invariant (see CLAUDE.md line 95).

## Template Migration

Tera is very close to Django syntax. Most conversions are `s/$/$/`:

| Django | Tera | Notes |
|--------|------|-------|
| `{% extends "base.html" %}` | `{% extends "base.html" %}` | Identical |
| `{% include "icons/x.html" %}` | `{% include "icons/x.html" %}` | Identical |
| `{% for item in list %}` | `{% for item in list %}` | Identical |
| `{% if condition %}` | `{% if condition %}` | Identical |
| `{{ value\|markdown }}` | `{{ value \| markdown }}` | Custom Tera filter |
| `{{ value\|display_name }}` | `{{ value \| display_name }}` | Custom Tera filter |
| `{% url 'route_name' %}` | Hardcoded path **or** `url_for()` Tera function | **Hardcode recommended** — routes are stable, fewer moving parts |
| `{% csrf_token %}` | Custom Tera function that emits hidden input from request context | Plus middleware to validate on POST |

The `#app-urls` div in `base.html` uses `{% url %}` per named route. Hardcode the paths in place — this matches how frontend already receives them (as data attributes). Frontend code doesn't change.

## Route Mapping (sketch)

```rust
let app = Router::new()
    .route("/", get(core::index))
    .route("/setup/", get(core::setup).post(core::setup))
    .route("/chat/", get(chat::view))
    .route("/chat/send/", post(chat::send))
    .route("/chat/switch/", post(chat::switch))
    .route("/chat/new/", post(chat::new_session))
    .route("/chat/delete/", post(chat::delete))
    .route("/chat/pin/", post(chat::pin))
    .route("/chat/rename/", post(chat::rename))
    .route("/chat/save-draft/", post(chat::save_draft))
    .route("/chat/retry/", post(chat::retry))
    .route("/chat/edit-message/", post(chat::edit_message))
    .route("/session/scenario/", post(chat::save_scenario))
    .route("/session/thread-memory/update/", post(chat::thread_memory_update))
    .route("/session/thread-memory/status/", get(chat::thread_memory_status))
    .route("/session/thread-memory/settings/", post(chat::thread_memory_settings))
    .route("/session/fork-to-roleplay/", post(chat::fork_to_roleplay))
    .route("/memory/", get(memory::view))
    .route("/memory/update/", post(memory::update))
    // ... remaining ~20 routes
    .layer(csrf_layer)
    .layer(session_layer)
    .with_state(app_state);
```

POST-only enforcement is structural: `.route("/x/", post(handler))` inherently rejects GET, replacing Django's `@require_POST`.

---

## Migration Philosophy

1. **Service-by-service, not layer-by-layer.** Port `session.rs` completely (types, CRUD, locks, tests) before touching `chat.rs`. Avoids "half a service" debt.
2. **Handlers and templates together.** Don't separate "port backend" from "port templates" — they're coupled. Each phase lands handlers + their templates simultaneously and ends with that feature actually rendering in a browser.
3. **Write Rust-native tests, not Python-parity tests.** Each service gets integration tests that exercise real files in a temp dir. Tests prove *the Rust code is correct*, not that it matches Python byte-for-byte.
4. **Wipe `data/` between experiments.** No migration scripts. No compat shims. If a schema changes, delete the old data.
5. **One invariant list, re-injected every session.** When working on this with Claude in a future session, always include:
   - Ownership table (who writes what)
   - "No lock across `.await` that calls LLM"
   - Context assembly order (persona identity → persona context → global context → scenario → thread memory → persona memory for chatbot only)
   - View/service/frontend separation rules

---

## Phased Plan

Each phase has: **deliverable**, **files**, **gotchas**, **done-when**. Phases run in order — later phases depend on services from earlier ones.

### Phase 0 — Python-side cleanup (Django, not Rust)

**Done 2026-04-21.** All five tasks landed on main via commit 920b808.

**Deliverable:** SoC drift from the current review is resolved before porting begins, so the Rust port isn't reproducing flaws.

**Tasks:**
1. Convert `utils.js:726-753` to `async/await`.
2. Convert `utils.js:1156-1165` to `async/await`.
3. Convert `utils.js:1268-1285` to `async/await`.
4. Replace `utils.js:479` `setAttribute('onclick', ...)` with `addEventListener`, or move the edit button into the template with Alpine `@click`.
5. Move `settings.py:212` `os.path.exists` into `config_manager.config_file_exists()`.

**Done when:** `node --check chat/static/js/components.js`, `node --check chat/static/js/utils.js`, `.venv/bin/python3 manage.py check` all pass; manual smoke test of memory update, clipboard copy, message edit confirms nothing regressed.

### After Phase 0 — Branch setup for the migration

**Done 2026-04-21.** `python-legacy` and `rust-migration` exist on origin; main carries the freeze notice; python-legacy carries a one-line "final Python version" banner. Optional `v0.99.0` tag was skipped — the branches are sufficient.

Once Phase 0 is merged into main, create the long-lived branches **before** adding the freeze notice to main, so `python-legacy` inherits a clean README (no freeze notice, which only belongs on `main`).

```bash
# From a clean tip of main with Phase 0 merged:
git checkout main && git pull
git branch python-legacy
git branch rust-migration
git push origin python-legacy rust-migration

# Optional: tag the split point (e.g., last stable Python release)
git tag v0.99.0 main && git push origin v0.99.0

# Now add the freeze notice to main (only lands on main):
# ... edit README.md ...
git commit -am "freeze main for Rust migration; direct users to python-legacy"
git push origin main
```

**Three branches, three meanings:**
- `main` — transitional / frozen. No commits (feature or fix) until the Rust migration merges. README directs users to `python-legacy` for running the app and `rust-migration` for tracking progress.
- `python-legacy` — last stable Python-only version. Frozen going forward. Optional: add a one-line banner to its README (in a single commit on that branch) noting "final Python version, no further updates."
- `rust-migration` — active dev for Phases 1–7. Python stays intact here through Phase 6; Phase 7 deletes it. This is the only branch where day-to-day work happens during M1.

**Freeze discipline:** do not land commits on `main` during the migration. If something genuinely urgent surfaces (e.g., a Python dep security issue), cherry-pick into `rust-migration` too so Phase 7's delete commit has visibility of anything new. Otherwise `main` stays exactly as it was at the split.

**At cutover (end of Phase 7):** regular merge commit from `rust-migration` → `main`. No last-minute tagging scramble — `python-legacy` already exists and has been discoverable the whole time. Post-merge, bump the version on `main` to `v1.0.0` to signal the Rust/Tauri break.

---

### Phase 1 — Rust scaffold

**Done 2026-04-21.** See "Outcome" block below for the concrete choices that were settled.

**Deliverable:** Axum server boots, serves `/health`, serves static files from `chat/static/`, renders a hello-world Tera template.

**Files to create:**
- `Cargo.toml` — workspace + member crate `liminal-salt`
- `src/main.rs` — Axum setup, `tokio::main`, static file mount
- `src/routes.rs` — router assembly (stub)
- `src/services/mod.rs` — module stub
- `templates/hello.html` — Tera smoke test
- `.gitignore` — `target/`

**Crates to pin (suggested starting versions):**
- `axum`, `tokio` (with `rt-multi-thread` + `macros` + `fs` + `signal`), `tower`, `tower-http` (with `fs` + `trace`)
- `tera`, `serde`, `serde_json`, `reqwest` (with `json`), `chrono` (with `serde`), `pulldown-cmark`
- `tracing`, `tracing-subscriber`, `anyhow`, `thiserror`
- `regex`, `once_cell` or `std::sync::OnceLock`

**Gotchas:**
- Pick `chrono` **or** `time`, not both. The plan assumes `chrono`.
- Tera template auto-reload only in dev. Add a `cfg!(debug_assertions)` gate.

**Done when:** `cargo run` boots, `curl localhost:PORT/health` returns 200, `curl localhost:PORT/static/js/utils.js` returns the file, `curl localhost:PORT/hello` renders the template.

**Phase 1 outcome (what was actually settled):**

- **Layout:** workspace root at repo root; single member crate at `crates/liminal-salt/`. Keeps room for `src-tauri/` as a sibling member in M2. All source files live inside the crate (`crates/liminal-salt/src/...`, `crates/liminal-salt/templates/...`) rather than at repo root.
- **Toolchain:** Rust 1.95.0, edition = `"2024"`.
- **Port:** 8420 (reused from Django for continuity on localhost).
- **Templates dir:** `crates/liminal-salt/templates/` (inside the crate, not at repo root). Path resolved via `CARGO_MANIFEST_DIR` at compile time so `cargo run` works from any cwd. Positions the crate for `rust-embed` in M2 without further restructuring.
- **Static dir:** unchanged at `chat/static/`; served via `tower-http::ServeDir` mounted at `/static`. Reached from the crate as `manifest_dir.join("../../chat/static")`.
- **Dependency versions actually pinned:** `axum = "0.8"`, `tokio = "1"` (features `rt-multi-thread`/`macros`/`fs`/`signal`), `tower = "0.5"`, `tower-http = "0.6"` (features `fs`/`trace`), `tera = "1"` (2.x is alpha only), `serde = "1"`, `serde_json = "1"`, **`reqwest = "0.13"`** with `default-features = false, features = ["json", "rustls"]` — note the feature was renamed from `rustls-tls` to `rustls` in 0.13; **`pulldown-cmark = "0.13"`**, `chrono = "0.4"` (serde feature), `tracing = "0.1"`, `tracing-subscriber = "0.3"` (env-filter), `anyhow = "1"`, `thiserror = "2"`, `regex = "1"`. `once_cell` deferred — `std::sync::OnceLock` / `LazyLock` works for now.
- **Tera auto-reload under `cfg!(debug_assertions)`: not yet wired.** Phase 1 builds templates once at boot. Add the dev-reload gate when Phase 3 starts real template iteration.

---

### Phase 2 — Foundation services (config, session, LLM)

These three have no intra-layer dependencies and block everything else.

**2a. `services/config.rs`**
- Load/save `data/config.json` (serde).
- Parse `AGREEMENT.md` line-1 version comment (regex: `<!--\s*version:\s*(\S+)\s*-->`).
- `is_app_ready(&AppConfig) -> bool` (SETUP_COMPLETE + AGREEMENT_ACCEPTED == current).
- `data_dir()` resolver — single function that returns the root path. Services take `&Path` via config, not env vars. This is the **Tauri integration seam**: in M2, only this function changes.

**2b. `services/llm.rs`**
- `LlmClient { api_key, model, referer, title }` struct.
- `async fn call_llm(messages, temperature, max_tokens) -> Result<String, LlmError>` — reqwest POST to `openrouter.ai/api/v1/chat/completions`, sets `HTTP-Referer` and `X-Title` for app attribution (currently in `llm_client.py`).
- Error variants: `NoApiKey`, `Network(reqwest::Error)`, `BadStatus(u16, String)`, `BadResponse(String)`.

**2c. `services/session.rs`** — the big one
- `Session` struct with all fields from CLAUDE.md schema table. Use `#[serde(skip_serializing_if = "Option::is_none")]` on optional fields (`title_locked`, `draft`, `pinned`, `scenario`, `thread_memory_settings`) so the on-disk shape is clean.
- `Message { role, content, timestamp }`.
- `now_timestamp() -> String` — decide format now. Proposal: `chrono::Utc::now().to_rfc3339_opts(SecondsFormat::Micros, true)` which yields `2026-04-21T12:34:56.123456Z`. Still fixed-width, still lexicographic-sortable. Fine to differ from Python's output — `data/` will be wiped.
- `valid_session_id(&str) -> bool` — regex ported verbatim (`^session_\d{8}_\d{6}(?:_\d+)?\.json$`).
- Per-session locks: `static SESSION_LOCKS: Lazy<DashMap<String, Arc<Mutex<()>>>>`.
- Public async functions: `load_session`, `save_chat_history`, `create_session`, `delete_session`, `rename_session`, `pin_session`, `save_draft`, `save_scenario`, `save_thread_memory`, `save_thread_memory_settings_override`, `clear_thread_memory_settings_override`, `fork_to_roleplay`, `list_sessions`.
- File writes: open with `O_TRUNC`, `write_all`, `sync_all`, then close. Do not rename-dance — matches current Python semantics.

**Gotchas:**
- `tokio::fs` vs `std::fs`: use `tokio::fs` throughout for consistency with the async runtime. `spawn_blocking` is acceptable for short CPU-bound work (regex, serde) but file I/O should be async.
- **Lock discipline:** `tokio::sync::Mutex` guards are `!Send` across certain `.await` points — not an issue here because session-manager functions don't call the LLM while holding the lock. Memory worker will need care (Phase 5).

**Tests (in `tests/session.rs`):**
- Create → load → assert fields match.
- Create → save messages → load → assert order + timestamps.
- Two concurrent `save_chat_history` calls on same session → both complete, final file is valid JSON, no torn writes.
- Invalid session IDs return `None`/no-op without panicking.
- `fork_to_roleplay` copies the thread, switches mode, leaves origin untouched.

**Done when:** `cargo test -p liminal-salt --test session` green; manual: write a session via a REPL binary and diff the JSON structure against a Python-generated one (shape check only, not byte-equal).

---

### Phase 3 — Chat flow

**Deliverable:** The primary chat loop works end-to-end in a browser. Send a message, see a response, refresh, history persists.

**Services:**
- `services/chat.rs` — `ChatCore` equivalent. Calls `session::load_session` → appends user message → calls `prompt::build_system_prompt` (stub for now, real in Phase 4) → calls `llm::call_llm` → appends assistant message → `session::save_chat_history`.
- `services/prompt.rs` — **stub** that returns persona identity only. Context files, scenario, memory come in later phases.
- `services/summarizer.rs` — `generate_title(&[Message]) -> Option<String>`. One-shot LLM call, first-reply-only, respects `title_locked`.

**Handlers (`src/handlers/chat.rs`):**
- `GET /chat/` → render `chat.html` with current session (or empty state)
- `POST /chat/send/` → append + stream response (start non-streaming; add streaming later if wanted)
- `POST /chat/switch/`, `/chat/new/`, `/chat/delete/`, `/chat/pin/`, `/chat/rename/`, `/chat/save-draft/`, `/chat/retry/`, `/chat/edit-message/`
- `POST /session/scenario/`, `/session/fork-to-roleplay/`

**Templates ported (Django → Tera):**
- `base.html` (including `#app-urls` with hardcoded paths, CSRF meta, theme injection)
- `chat/chat.html` + all its partials (`components/*` that chat uses, `icons/*`)

**Gotchas:**
- **CSRF setup lands here.** Generate a per-session token, embed in `<meta name="csrf-token">`, accept via `X-CSRFToken` header (matches what HTMX already sends). Double-submit cookie pattern is sufficient for localhost use.
- HTMX response fragments: the current views return rendered partials for many endpoints. Make sure Tera partial includes produce byte-for-byte the shape HTMX expects — target element IDs, data attributes.
- The "auto-title on first reply, one shot" logic (sets `title_locked = true`) must live in `ChatCore`, not the handler.

**Tests:**
- Integration: POST a message, receive 200, reload GET /chat/, assert message present.
- Unit: `ChatCore::send` preserves `scenario`, `thread_memory`, `pinned`, `draft` through the RMW.

**Done when:** manual smoke — open browser, send 3 messages, switch sessions, pin one, rename one, refresh; all persists. `chat_core.py` tests (if any) mirrored in Rust and green.

---

### Phase 4 — Personas + context files + local context

**Services:**
- `services/persona.rs` — CRUD, rename cascade, delete cleanup. **Owns** `data/personas/{name}/` and `identity.md`. Config.json goes through `context_manager::save_persona_config` (Python) → in Rust, `prompt::save_persona_config` (to match Python's owner: `context_manager.py`).
- `services/context_files.rs` — `ContextFileManager` with `base_dir` + `scope_label`. Two instances at runtime: global + per-persona.
- `services/local_context.rs` — `LocalContextScanner` for user-configured local directories. Reads at prompt-assembly time (not cached — matches Python).
- `services/prompt.rs` **expanded**: now does real context assembly in CLAUDE.md order: persona identity → persona context → global context → scenario → thread memory → persona memory (chatbot only, suppressed in roleplay).

**Handlers:**
- `/persona/*` (settings view, save, create, delete, model override, thread defaults save/clear)
- `/memory/` view + persona context file CRUD
- `/context/local/*` (shared between global and persona scopes via `persona` query param)

**Templates:** `persona/*.html`, `memory/*.html` (the persona memory + context files view).

**Gotchas:**
- Persona rename must be atomic-ish — if mid-cascade a write fails, the remaining state is broken. Log + best-effort continue is acceptable; perfection isn't.
- Sessions reference personas by folder name. A rename updates every session's `persona` field. This is a write-amplifier; `SessionManager::rename_persona_in_all_sessions` should exist and be called by `PersonaManager::rename_persona`.

**Done when:** create a persona, upload a context file, switch a session to it, send a message; verify the system prompt (log it) includes the expected sections in the expected order.

---

### Phase 5 — Memory system (HIGHEST RISK PHASE)

This is the single highest-risk phase. Allocate the most time. Review concurrency carefully.

**Services:**
- `services/memory.rs` — per-persona memory file I/O + LLM merge/seed/modify. Owns `data/memory/{name}.md`.
- `services/thread_memory.rs` — per-session running summary via LLM merge. Two prompt variants (chatbot / roleplay). Settings resolver (per-thread override → persona default → global fallback).
- `services/memory_worker.rs` — two `tokio::spawn`ed scheduler tasks:
  - **Persona memory scheduler:** per-persona interval + message floor. Per-persona `tokio::sync::Mutex`. Tracks last-seen state per persona.
  - **Thread memory scheduler:** per-session. Reads effective settings via resolver. Tracks `_thread_next_fire_time` equivalent.
- Both caches (`_session_cache`, `_thread_scheduler_cache`, `_persona_count_cache`) keyed by mtime. Invalidate entries whose sessions disappear.

**Handlers:**
- `/memory/update/`, `/memory/wipe/`, `/memory/modify/`, `/memory/seed/`, `/memory/status/`
- `/session/thread-memory/update/`, `/session/thread-memory/status/`, `/session/thread-memory/settings/`

**Critical invariants to preserve (re-inject these into any future session prompt):**

1. **Lock released across LLM call.** Worker reads session under lock → releases → calls LLM → re-acquires lock → writes. In Rust: acquire lock, load data, **drop the guard**, await the LLM, re-acquire, save. Mechanically:
   ```rust
   let messages = {
       let _guard = session_mgr.lock(&session_id).await;
       session_mgr.load_session(&session_id).await?.messages
   }; // guard drops here
   let merged = thread_memory_mgr.merge(&messages, existing).await?; // LLM
   {
       let _guard = session_mgr.lock(&session_id).await;
       session_mgr.save_thread_memory(&session_id, merged, cutoff).await?;
   }
   ```
2. **"Already running" mutex is separate from the JSON lock.** Matches current Python: `memory_worker` has its own `_session_locks` distinct from `session_manager._session_locks`. Keep this separation in Rust — collapsing them re-introduces bug #1.
3. **Status state machine:** `idle → running → (completed | failed) → idle`. Exposed via `/memory/status/` polling.
4. **Roleplay sessions** excluded from persona-memory aggregation, and don't get persona memory injected into their prompts (see `prompt::build_system_prompt`).
5. **Thread-memory "no new messages since last summary"**: on manual runs, reprocess full thread; on auto runs, skip silently. Current Python line 657 in `memory_worker.py` documents the why.

**Gotchas:**
- `tokio::sync::MutexGuard` is not `Send` by default across `.await`. If you need the guard to cross an `.await` point, you've probably violated invariant #1. If you legitimately need `Send`, `parking_lot::Mutex` won't help (it'll block the runtime). Restructure.
- Two scheduler tasks share no state directly — each owns its own tick loop. They coordinate only through the filesystem.

**Tests:**
- Scheduler fires after interval elapses (mock time with `tokio::time::pause` + `advance`).
- Manual update while auto is running → manual queues until auto releases the "already running" mutex.
- Concurrent user message write + memory-worker read: user write sees the latest, worker gets a consistent snapshot.

**Done when:** manual smoke — let a roleplay thread accumulate 20 messages, trigger thread-memory update, verify the summary appears in the next response's context (log the prompt). Wait for the auto-scheduler to fire; verify via logs.

---

### Phase 6 — Settings + setup wizard + API + summarizer

**Services:**
- `services/summarizer.rs` — already drafted in Phase 3, complete it here. Title generation prompt, one-shot call.

**Handlers:**
- `/settings/*` — view, save, provider validation, per-provider model save, context history limit, global context file CRUD.
- `/setup/*` — multi-step wizard (provider → model → agreement). Uses `tower-sessions` for step state.
- `/api/themes/`, `/api/models/`

**Templates:** `settings/*`, `setup/*`.

**Gotchas:**
- **Setup wizard state must survive a page reload.** Use `tower-sessions` with cookie store; don't try to pass state via hidden form fields alone.
- **Agreement version check:** `is_app_ready()` already handles this in `config.rs` (Phase 2). Verify the redirect-to-setup path is wired.
- **Provider validation** makes a live call to OpenRouter (`/api/v1/models`). Budget 10s timeout and a clear error path.

**Done when:** fresh `data/` dir → launch app → redirected to /setup → walk through wizard → land in /chat/. Every settings page saves and reloads correctly.

---

### Phase 7 — Cutover + polish

**Deliverable:** Django removed; Rust is the only backend.

**Tasks:**
1. Delete `liminal_salt/` (Django project), `chat/` (app), `manage.py`, `requirements.txt`, `run.py`, `scripts/`.
2. Update `package.json` — Tailwind build pipeline stays; update the "dev" concurrent command to run `cargo run` instead of `manage.py runserver`.
3. Update `CLAUDE.md`: rewrite the "Directory layout" and "Services" sections for the Rust codebase. Preserve the invariant list — it's language-agnostic.
4. Update `README.md`: build/run instructions (`cargo run` vs `npm run dev`).
5. Manual full-feature smoke test using a checklist (every route, every feature).

**Done when:** `grep -r "python\|django\|waitress" . --include="*.md" --include="*.toml" --include="*.json"` returns only historical references. `cargo run` + `npm run tailwind` is the entire dev loop.

---

# Milestone 2: Tauri Desktop App

Wrap the Rust backend in Tauri. Since M1 produced a Rust Axum server, Axum runs in-process — no child process, no bundled runtimes, no IPC bridge.

## Architecture

```
┌──────────────────────────────────────┐
│           Tauri App (single binary)  │
│                                      │
│  ┌────────────────────────────────┐  │
│  │  Axum Server (in-process)      │  │
│  │  All M1 services               │  │
│  │  Serves on 127.0.0.1:{port}    │  │
│  └────────────────────────────────┘  │
│                                      │
│  ┌────────────────────────────────┐  │
│  │  OS Native Webview             │  │
│  │  WebKit (macOS/Linux)          │  │
│  │  WebView2 (Windows)            │  │
│  │  Loads http://127.0.0.1:{port} │  │
│  └────────────────────────────────┘  │
└──────────────────────────────────────┘
```

## Implementation Scope

| Task | Details |
|------|---------|
| Scaffold | `cargo tauri init` adds `src-tauri/`. Configure `tauri.conf.json` (window size, title, icons, app ID `com.liminalsalt.app`). |
| Axum integration | Start Axum in Tauri's `setup` hook, pick dynamic port (bind `127.0.0.1:0`, read back actual port), pass port to window URL. |
| Window management | Single window → `http://127.0.0.1:{port}`. Disable dev tools in release. |
| Lifecycle | Axum task spawned on Tauri setup; abort on window-close event. Clean shutdown. |
| Data directory | Swap `config::data_dir()` to return Tauri's `app_data_dir()`. **One function change** (this is why it's a single seam in M1). |
| Asset embedding | Use `rust-embed` (or `include_dir!`) to ship: `templates/`, `chat/static/`, `chat/default_personas/`, `chat/static/themes/`, `AGREEMENT.md`. On first launch, seed defaults into `app_data_dir()`. |
| App icons | Generate `.icns` / `.ico` / `.png` via `cargo tauri icon`. |
| Build | `cargo tauri build` per target platform. |

## Data Directory

`app_data_dir()` resolves to:

| Platform | Path |
|----------|------|
| macOS | `~/Library/Application Support/com.liminalsalt.app/` |
| Windows | `C:\Users\<user>\AppData\Roaming\com.liminalsalt.app\` |
| Linux | `~/.local/share/com.liminalsalt.app/` |

Directory shape inside is whatever M1 settled on. Flat files, self-contained, user can back up by copying a folder.

## Success Criteria

- App launches as a native window (no browser, no address bar).
- Single binary, no external dependencies.
- Binary size < 20MB.
- Native window controls.
- App icon in taskbar / dock.
- Clean shutdown (Axum stops on window close).
- Data persists in platform-appropriate location.
- All functionality identical to the browser M1 version.
- Builds for macOS, Windows, Linux.

---

## Known Hard Spots (Consolidated)

The places where AI-generated code compiles but may silently diverge from correct behavior. Call these out explicitly in any session prompt that touches them.

1. **Memory worker concurrency** (Phase 5) — lock-across-`.await` is the #1 footgun. Re-inject CLAUDE.md invariant: "Holding a lock across an LLM call is forbidden."
2. **CSRF wiring** (Phase 3) — silent failures look like "the form just doesn't submit." Test with an actual POST from the browser, not just curl.
3. **Session setup-wizard state** (Phase 6) — `tower-sessions` cookie config (SameSite, HttpOnly, Secure=false for localhost) is easy to get wrong.
4. **HTMX partial shape drift** (Phase 3 onward) — HTMX depends on specific element IDs and data attributes in swap targets. A Tera rewrite that emits subtly different HTML (extra wrapper div, different ID) breaks UX in ways that pass unit tests. Browser smoke test every HTMX interaction.
5. **Atomic multi-file operations** (Phase 4, persona rename) — no transactions; partial-failure is possible. Accept "log and continue" semantics; document what state is possible after a failure.

---

## Working With Claude on This Migration (for future sessions)

Tips for when this doc gets handed to Claude in a new session to execute a phase:

1. **Always start with CLAUDE.md.** The invariant list (ownership table, lock rules, separation rules, context assembly order) is load-bearing regardless of language.
2. **Work one phase at a time.** Do not try to do "Phase 2 + 3" in one go — the context gets too large and gotchas get missed. Land Phase N in a PR, merge, start Phase N+1 fresh.
3. **Re-inject the hard-spot invariant for the phase you're on.** For Phase 5, paste the lock-across-await rule verbatim.
4. **When writing tests, prefer integration tests with temp-dir filesystems.** `tempfile::TempDir` + real `tokio::fs` calls catches 10x more bugs than mocking.
5. **Run `cargo test` + browser smoke test each phase.** Type checking proves the code compiles; the browser proves the feature works. Both are required.
6. **If timestamps or JSON shape looks subtly different from Python, that's fine.** The constraint is "Rust-correct," not "Python-identical." See Project Constraints above.
7. **If in doubt about a service boundary, re-read the CLAUDE.md ownership table.** Don't invent new ownership during the port; preserve the Python boundaries exactly.

---

## Appendix: Frontend Inventory (what must survive unchanged through M1)

- `chat/static/js/utils.js` (post-Phase-0 cleanup) — shared helpers
- `chat/static/js/components.js` — Alpine component registrations
- `chat/static/css/input.css` + `output.css` — Tailwind source + build
- All `chat/templates/**/*.html` — markup only migrates (Django → Tera); data contract unchanged
- `chat/static/themes/*.json` — 16 theme files
- All HTMX attributes (`hx-get`, `hx-post`, `hx-target`, `hx-swap`, `hx-trigger`)
- All SVG icons under `chat/templates/icons/`

Any change to these during M1 is a bug in the port, not a feature.
