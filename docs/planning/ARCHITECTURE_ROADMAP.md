# Liminal Salt — Architecture Roadmap

**Created:** April 14, 2026
**Updated:** April 22, 2026
**Status:** Rust migration in progress — Phases 0–4 complete; Phase 5 (memory system) in progress (5a + 5b done)
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

**Done 2026-04-21.** See "Outcome" block below for concrete choices.

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

**Phase 2 outcome (what was actually settled):**

- **Library layout.** Added `src/lib.rs` exposing `services`, `routes`, `AppState`. `main.rs` is now thin — just boot wiring — so integration tests under `tests/` can import the crate. `[lib]` + `[[bin]]` sections coexist in the member Cargo.toml.
- **Per-session lock map** uses `std::sync::LazyLock<StdMutex<HashMap<String, Arc<TokioMutex<()>>>>>` — **no `dashmap` dep**. The outer `StdMutex` is held briefly for registry insert/lookup (never across `.await`); the inner `TokioMutex` guards each session's I/O and is async-safe across awaits.
- **Timestamp format:** `chrono::Utc::now().to_rfc3339_opts(SecondsFormat::Micros, true)` → `2026-04-21T12:34:56.123456Z`. Diverges from Python's `+00:00` suffix — accepted per project constraints.
- **Session ID generation uses UTC** (not Python's local time). Still matches the validation regex; just keeps filenames monotonic across timezones.
- **Session struct** — optional fields (`title_locked`, `draft`, `pinned`, `scenario`, `thread_memory_settings`) are `Option<_>` with `skip_serializing_if = "Option::is_none"`. `thread_memory` / `thread_memory_updated_at` are plain `String` with `skip_if_empty`. On-disk shape matches Python's "absent until first set" semantics.
- **`AppConfig`** uses `#[serde(rename_all = "SCREAMING_SNAKE_CASE")]` to match Python's config.json key style, plus `#[serde(flatten)] extras: BTreeMap<String, serde_json::Value>` so unknown keys round-trip through load → save without loss. Phase 6 will grow the typed fields as the setup wizard / settings UI is ported.
- **`AGREEMENT.md` parsing** is single-shot at startup via `LazyLock<Agreement { version, body }>`. Accessor `current_agreement_version() -> &'static str` returns a zero-copy ref for comparisons in `is_app_ready()`.
- **`data_dir()` Tauri seam** returns `CARGO_MANIFEST_DIR/../../data`. Sibling helpers `sessions_dir(&Path)` / `config_file(&Path)` take a base path so services don't hardcode the resolver (tests inject `TempDir::path()`).
- **LLM headers** match Python exactly: `HTTP-Referer`, `X-OpenRouter-Title`, `X-OpenRouter-Categories` (not the roadmap's `X-Title`). App attribution constants (`https://liminalsalt.app`, `Liminal Salt`, `general-chat,roleplay`) ported verbatim.
- **`LlmMessage { role, content }`** is a dedicated struct distinct from `session::Message` — stripping the `timestamp` field before dispatch so the API doesn't see it. ChatCore in Phase 3 will convert.
- **`LlmError`** variants: `NoApiKey`, `Network(reqwest::Error)`, `BadStatus { status, body }`, `BadResponse(String)`. `Network` uses `#[from]` so `?` on reqwest calls works.
- **Return conventions match Python semantics:** `Option<T>` for reads (invalid id / missing file both → `None`), `bool` for writes (false on any failure), errors logged via `tracing::error!` but not propagated. Trades Rust-idiomatic `Result<_, E>` for cleaner caller ergonomics at the view layer — fine since every call site would otherwise log-and-ignore the same way.
- **`tokio::fs` throughout** for file I/O. Writes use `OpenOptions::truncate(true) + write_all + sync_all` (matches Python's `open('w') + flush + fsync`).
- **Integration tests** live at `tests/session.rs` — 16 cases covering id validation (positive + traversal rejection), create/load round-trip, optional-field absence on disk, RMW preservation across `save_chat_history`, rename / pin / delete / remove-last-assistant / update-last-user, thread-memory-settings merge + clear, `fork_to_roleplay` (new has roleplay mode + copied thread memory + reset pinned/draft/scenario; origin untouched), `update_persona_across_sessions` matching-only rewrite, 10-task concurrent `save_chat_history` produces valid JSON. Uses `tempfile::TempDir` per test.
- **`AppState` unchanged in Phase 2.** No handlers yet use `config`/`session`/`llm`. Wiring (data_dir, config, LlmClient, Tera) happens in Phase 3 when the first handler (`POST /chat/send/`) needs them all.

---

### Phase 3 — Chat flow

**In progress.** Split into three sub-commits so HTMX-shape-drift and CSRF bugs (known hard spots #2 and #4) can be iterated against a narrower blast radius:
- **3a (done 2026-04-21):** infrastructure + services (CSRF middleware, tower-sessions, Tera filters, AppState expansion, `services/{chat,prompt,summarizer}.rs`, ChatLlm trait, default-persona seeding).
- **3b (done 2026-04-21):** templates port (Django → Tera): `base.html`, `chat/*`, `components/*`, `icons/*` as Tera macros. Phase 4+ modals stripped from `chat.html` (threadMemory / editPersona* / contextFiles × 2) — they'll come back as their owning phases land. `escapejs` Tera filter added to match Django's behavior. 9 render-smoke tests exercise every Phase 3 template.
- **3c (done 2026-04-21):** handlers wired + end-to-end curl verified: `/chat/` renders home, `/chat/start/` creates session + triggers auto-send, `/chat/send/` calls the LLM and appends response, auto-title fires on first reply (`title_locked=true` set), session JSON round-trips. Phase 4+ endpoints stubbed so sidebar buttons surface "Coming soon" cleanly.

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

**Phase 3c outcome:**

- **Handler modules:** `src/handlers/{chat,session,stubs}.rs` + `handlers/mod.rs`. Each handler is thin — parse → call service → render → return. No file I/O, no direct LLM calls, all file ops go through `services::session` and all LLM ops through `services::chat` / `services::summarizer`.
- **Route table** (`src/routes.rs`): 12 chat-flow routes + 2 session routes + 7 stub routes for Phase 4/5 endpoints the templates reference. Stubs return a "Coming soon" HTML placeholder for GETs and 501 for POSTs, plus minimal JSON for `/api/themes/` and `/api/save-theme/` so `utils.js` doesn't choke on page load.
- **`AppConfig` keys updated** to match Python's actual config.json: `MODEL` (not `DEFAULT_MODEL`), `THEME_MODE`, `CONTEXT_HISTORY_LIMIT`. Renamed `AppConfig::default_model` → `AppConfig::model`, added `theme_mode` and `context_history_limit` fields.
- **Sidebar grouping:** `grouped_sessions` passed to templates as `Vec<PersonaGroup { persona, sessions }>` rather than a map — Tera iterates the list cleanly (`{% for group in grouped_sessions %}`), preserves insertion order (newest-persona-first per Python semantics), and skips the `preserve_order` feature on `serde_json`.
- **`LlmClient::with_http_client`** builder method added — handlers construct a fresh `LlmClient` per LLM call but share the `AppState::http` connection pool across all of them.
- **Title generation** fires once at the end of `send` (after `chat_svc::send_message` returns successfully): read session fresh, check `title_locked`, call `summarizer::generate_title`, persist with `title_locked=true`. Handler sets `X-Chat-Title` and `X-Chat-Session-Id` response headers; the `base.html` script listens for these and updates the sidebar + header on the fly.
- **Retry flow** (`/chat/retry/`): removes the last assistant message via `session::remove_last_assistant_message`, then delegates to `send` with `skip_user_save=true` so the user message isn't duplicated. One handler entry point, zero duplicated logic.
- **Timezone persistence:** every chat POST persists `form.timezone` via `session_state::set_user_timezone`; subsequent sends pick it up from tower-sessions and pass it to `chat::build_payload` for the `[user's time]` prefix.
- **`/chat/start/` flow:** creates session with initial user message, writes scenario if roleplay, stores `current_session`, renders `chat_main.html` with `pending_message` set — the template fires `hx-trigger="load"` POSTing to `/chat/send/` with `skip_user_save=true` so the user sees the thinking indicator while the LLM responds. Same UX as the Django version.
- **End-to-end curl smoke** verified against a real OpenRouter call (`deepseek/deepseek-v3.2`, 2.8s roundtrip):
  - GET `/chat/` → 46KB page, CSRF meta present, sidebar + home form rendered.
  - POST `/chat/start/` with persona+mode+message → 200, returns `chat_main.html` with pending_message.
  - POST `/chat/send/` with CSRF header + skip_user_save=true → 200, real LLM response rendered via `assistant_fragment.html`.
  - Session file written to `data/sessions/session_20260421_234336.json` — valid JSON, two messages with RFC3339 timestamps, `title: "Smoke Test Explanation"`, `title_locked: true`.
- **CSRF enforcement verified:** POST without `X-CSRFToken` → 403. POST with matching token → handler runs. Form-body fallback (`csrfmiddlewaretoken`) passes through the same path.
- **Browser smoke (all 19 checklist items) passed.** Bugs caught and fixed during the smoke run:
  1. **Session cookie rejected on `http://localhost`** — `tower-sessions` defaults to `Secure=true`, which some browsers silently drop on plain HTTP. Every request minted a fresh session, every POST 403'd on CSRF with no visible error. Fix: `SessionManagerLayer::with_secure(false)` in `main.rs`. (Known hard spot #3 from the roadmap; bit us exactly as documented.)
  2. **First user message invisible after "new chat" submit** — `start_chat` was pre-saving the user message to the session but passing `messages=[]` to the template, so the thinking indicator + assistant response appeared with no user bubble above them. Fix: pass the pre-saved messages list (the auto-send already sets `skip_user_save=true` so no duplication).
  3. **Home-page model name rendered as `deepseek&#x2F;deepseek-v3.2`** — Tera's `.html` templates auto-escape, so `{{ foo | escape }}` double-escapes (the `&` in `&#x2F;` becomes `&amp;#x2F;`, and the browser decodes one level). Fix: drop explicit `| escape` on three data attributes (`data-default-model`, `data-scenario`, `data-memory`); let auto-escape handle it once.
  4. **Edit-message never saved** — `saveEditedMessage` submits via `fetch(FormData)` which is `multipart/form-data`, but (a) my CSRF middleware only parsed urlencoded bodies, and (b) the handler used `Form<_>` extractor which rejects multipart. Fix: added multipart parsing to `csrf.rs` (`multipart_field_matches` + `boundary_from` helpers + 2 tests) and switched `edit_message` handler to `axum::extract::Multipart`. Enabled axum's `multipart` feature in Cargo.toml (pulls `multer` transitively). Persona / context-file uploads in Phase 4 will benefit from the same plumbing.
  5. **Scenario modal `@click` didn't fire on fresh page load** (but worked after navigation) — Alpine's initial DOM walk only processes directives on elements within an `x-data` scope; elements outside get picked up later via the MutationObserver on HTMX swaps. The folder button lives in `chat_main.html` under `<main>`, which had no `x-data`. Fix: added bare `x-data` to `<main id="main-content">` so Alpine gives it an empty scope. This also covers retry / edit / fork buttons on fresh page loads — they all relied on the same implicit scope.

**Known hard spot #3 validated, #2 and #4 dodged.** The CSRF wiring worked first-try except for the `Secure` cookie flag; HTMX partial shapes held up across every interaction.

**Phase 3b outcome:**

- **Templates ported:** 8 Tera files under `crates/liminal-salt/templates/`:
  - `base.html`, `chat/{chat,chat_main,chat_home,assistant_fragment,sidebar_sessions,new_chat_button}.html`, `components/select_dropdown.html`.
  - `chat.html` is the only template with Phase-scope decisions baked in: retains `renameModal` (Phase 3: rename_chat) and `scenarioModal` (Phase 3: save_scenario); strips `threadMemoryModal` (Phase 5), `editPersonaModal` / `editPersonaModelModal` (Phase 4), and both `contextFilesModal` instances (Phase 4). Stripped modals will be re-added in their owning phase.
- **Icons as Tera macros:** `templates/icons.html` defines 24 `{% macro %}` blocks, one per icon, each taking a `class` arg (defaults to `w-5 h-5`). Hyphens → underscores in names (`chevron_down`, `star_filled`, etc.) for Tera identifier rules. Every consumer template does `{% import "icons.html" as icons %}` at the top and calls `{{ icons::<name>(class="...") }}`.
- **URL paths hardcoded everywhere** (per roadmap guidance): `/chat/*`, `/session/*`, `/memory/*`, `/persona/*`, `/settings/*`, `/api/*`. Frontend already expects these via `#app-urls` data attributes; nothing changes client-side.
- **CSRF integration pattern:** base template's `<meta name="csrf-token" content="{{ csrf_token }}">` + per-form `<input type="hidden" name="csrfmiddlewaretoken" value="{{ csrf_token }}">` — a plain context-variable interpolation, no custom Tera function needed. The existing `utils.js` listener wires HTMX to auto-send `X-CSRFToken` and form POSTs to send the field; the CSRF middleware from 3a accepts both.
- **New Tera filter `escapejs`** (`src/tera_extra.rs`): port of Django's `|escapejs`. Escapes quotes, angle brackets, ampersands, `=`, `-`, `;`, backticks, U+2028/U+2029 line separators, and ASCII control chars as `\uXXXX`. Used wherever user-content crosses into JS-string or HTML-attribute contexts (`data-message="{{ msg | escapejs }}"`, modal `CustomEvent` `detail`).
- **Tera gotchas encountered** (documented so Phase 4/5 template ports don't re-hit them):
  1. `{% import %}` must be the first non-content line in a template — comments before it are tolerated but any other tag parses as content and breaks the import.
  2. `loop.revindex` doesn't exist. To port Django's `{% if forloop.revcounter <= 2 %}`: `{% if loop.index >= items | length - 1 %}`.
  3. Tera tests take positional args, not named: `is starting_with("ERROR:")`, not `is starting_with(pat="ERROR:")`.
  4. `slice` filter is Vec-only. For string prefix-stripping, `{{ s | replace(from="PREFIX:", to="") }}` — not semantically "strip leading" but fine when the prefix is a distinctive marker.
  5. Tera errors hard on undefined variables; use `{% set var = var | default(value="...") %}` at the top of includes that accept optional parameters (what Django's loose undefined-is-empty behavior gave us for free).
- **9 render-smoke tests** (`tests/template_render.rs`): every Phase 3 template rendered with plausible context. Asserts key structural elements (CSRF meta, markdown filter applied, error-prefix path, scenario-button-on-roleplay, fork-button-on-chatbot, sidebar pinned/grouped layout, escapejs output in attribute values). Catches render-time errors that parse-time doesn't — like the three caught and fixed during the port.
- **Frontend assets untouched:** HTMX 1.9.10, Alpine 3, Marked 15 CDN URLs preserved byte-for-byte. `{% static 'x' %}` → `/static/x` (one-to-one path swap). `utils.js` / `components.js` / `output.css` loaded from `/static/js/...` and `/static/css/...` — same URLs the 3a static mount serves from `chat/static/`.

**Phase 3a outcome:**

- **Session middleware:** `tower-sessions` 0.15 with `MemoryStore`, cookie name `liminal_salt_session` (matches Django), two-week inactivity expiry. State is process-local — server restart invalidates old session cookies; the browser gets a fresh one on the next GET. Three typed keys layered on top: `csrf_token`, `current_session`, `user_timezone`. No signing key needed since `MemoryStore` keys by server-side UUID.
- **CSRF middleware** (`src/middleware/csrf.rs`): mints a 32-byte hex token into the session on every request (so GET `/chat/` can embed it in `<meta name="csrf-token">`); on POST/PUT/PATCH/DELETE, compares the request's token to the session's using constant-time equality. Accepts `X-CSRFToken` header (fast path, every HTMX request + most `fetch()` calls) or the `csrfmiddlewaretoken` field in a `application/x-www-form-urlencoded` body (needed by the `saveEditedMessage` and persona-form submit paths). Multipart bodies aren't inspected — Phase 4+ file-upload endpoints will need to send the header.
- **Tera filters** (`src/tera_extra.rs`): `markdown` wraps `pulldown-cmark` with GFM options (tables, strikethrough, tasklists, footnotes). `display_name` does `snake_or-kebab` → `Title Case`.
- **`AppState` expanded** with `data_dir`, `sessions_dir`, `http: reqwest::Client` (single shared pool, 30s timeout). Config reloads from disk per-request — matches Python's read-every-time semantics; cost is trivial at localhost scale. Per-request `LlmClient` is constructed from the fresh config.
- **`ChatLlm` trait** (`services/llm.rs`): `fn complete(...) -> impl Future<Output=Result<_,_>> + Send`. `LlmClient` is the production impl; tests use a `FakeLlm` / `FailingLlm`. Generic bound `L: ChatLlm` avoids the async-dyn-trait footgun.
- **`services/chat.rs`:** stateless `send_message(ctx, llm, user_input, skip_user_save)` — loads session, optionally appends user message, builds payload (system prompt with prepended time-context block + per-user-message `[user's time | assistant's time]` prefixes using `chrono-tz`), runs LLM with 2 retries + 2s backoff + 120s per-call timeout, appends assistant message, saves via `session::save_chat_history`. Returns `SendOutcome { response, is_error }`. Title generation is deliberately **not** inside `send_message` — it's a separate summarizer call the handler will orchestrate in 3c.
- **`services/prompt.rs` (Phase 3 stub scope):** emits persona identity `.md` files (alphabetical, headered with `--- SYSTEM INSTRUCTION: filename ---`), plus scenario (roleplay threads only) and thread_memory when present. Persona memory and uploaded/local context files deferred to Phase 4/5. Also owns `seed_default_personas` — on startup copies `chat/default_personas/*` into `<data_dir>/personas/` if absent.
- **`services/summarizer.rs`:** `generate_title(llm, user_msg, assistant_msg)` ported verbatim from Python (artifact stripping, length validation, truncated-user-prompt fallback).
- **Return conventions preserved** from Phase 2: chat/summarizer return plain `String` (error path encoded as `"ERROR: ..."`), middleware returns `StatusCode` on rejection, everything else logs errors via `tracing`.
- **Tests added (13 new):** `csrf::tests` (form_field parsing, constant_time_eq), `tera_extra::tests` (markdown, display_name), `summarizer::tests` (clean_title, has_artifacts, fallback truncation, char-boundary-safe truncate), plus `tests/chat_core.rs` (4 integration tests: scenario/memory/pin/draft survive RMW, `skip_user_save` doesn't duplicate, LLM failure doesn't partial-save, missing session returns error). Total suite: 29 tests.
- **Smoke confirmed on `cargo run`:** session cookie sets correctly, unauthenticated POST returns 403, default personas seed into `data/personas/` on startup.

---

### Phase 4 — Personas + context files + local context

**In progress.** Split into three sub-commits following the Phase 3 pattern:
- **4a (done 2026-04-22):** service layer — `services/{persona,context_files,local_context}.rs` + expanded `services/prompt.rs`. Ownership audit: moved persona `config.json` from `context_manager` into `persona.rs` (fixed the historical Python split). 24 new integration tests; 69 tests total across the crate.
- **4b (done 2026-04-22):** template port for `persona/persona_main.html`, `memory/memory_main.html`, `chat/context_files_modal.html`, `chat/dir_browser_modal.html`, `chat/local_dir_tab.html`; restored the 4 modals in `chat.html` (`editPersonaModal`, `editPersonaModelModal`, global + per-persona `contextFilesModal`); added a `page` routing variable so chat.html acts as a shared shell for persona/memory/chat page variants. 4 new render tests; 73 tests total.
- **4c (done 2026-04-22):** handlers for `/persona/*`, `/memory/` view, context files (global + per-persona) and local directories. Debug log added to `chat::send` so the assembled system prompt prints on every send for easy verification. Browser smoke passed end-to-end: persona CRUD, context file upload/toggle/delete, local directory add + toggle, thread defaults save, memory page renders, system prompt in logs shows context file body.

**Ownership audit (2026-04-22):** CLAUDE.md's table had `data/personas/{name}/config.json` owned by `context_manager.save_persona_config` — a historical Python artifact where persona-scoped state was split across two services for no architectural reason. In the Rust port this moves to `persona.rs` alongside the directory it lives in. CLAUDE.md is not edited now because it still describes current Python reality; Phase 7's rewrite will capture the Rust convention from the actual code.

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

**Phase 4c outcome:**

- **Three new handler modules:** `handlers/{persona,memory,context}.rs`.
  - `persona.rs`: `GET /persona/` (honors `?persona=X` / `?preview=X`), plus `/settings/{create,save,delete}-persona/`, `/settings/save-persona-model/`, `/settings/save-persona-thread-defaults/`, `/settings/clear-persona-thread-defaults/`, and a minimal `/settings/save/` (default-persona only; Phase 6 owns the rest of that route).
  - `memory.rs`: `GET /memory/` read-only view. Memory operations (update / wipe / modify / seed / save-settings / status) remain stubbed — Phase 5's `memory_manager` owns them.
  - `context.rs`: uploaded-file CRUD for global (`/settings/context/*`) and persona (`/persona/context/*`) scopes, plus local-directory CRUD (`/context/local/{browse,add,remove,toggle,refresh,content}`). All POSTs parse multipart manually (JS sends `FormData`) and use the scope detection rule "presence of `persona` form field → persona scope, else global."
- **Shared helpers promoted:** `is_htmx`, `group_sessions`, `base_chat_context` moved from private to `pub(crate)` in `handlers/chat.rs` so persona/memory handlers can reuse them for consistent sidebar rendering.
- **Thread-defaults clamping mirrors the JS side** (0 OR `[5, 1440]` for interval, `[1, 1000]` for message_floor, `[0, 100000]` for size_limit) so the server's "effective" response matches the values the UI just snapped.
- **No-op detection for thread defaults:** if the submitted values match the global defaults and `default_mode` isn't "roleplay," no override is persisted. Matches Python's "clear if it would be a no-op" behavior.
- **`chat::send` now debug-logs the assembled system prompt** (persona, mode, byte count, full text) so the Phase 4 done-when verification is a single glance at the `cargo run` terminal.
- **Known gap:** `/settings/available-models/` still stubbed 501 — "Edit Model" modal shows a load error until Phase 6 wires up OpenRouter model listing.
- **Smoke fix caught during browser testing:** `LocalFileEntry::exists_on_disk` was renaming to JSON as `exists_on_disk`, but the JS frontend reads `file.exists` (matching Python's field name). Added `#[serde(rename = "exists")]` — local directory files now show correct presence state.

**Phase 4b outcome:**

- **Templates ported:** `persona/persona_main.html`, `memory/memory_main.html`, `chat/context_files_modal.html`, `chat/dir_browser_modal.html`, `chat/local_dir_tab.html`.
- **`chat.html` shared shell:** added a `page` context variable so the same top-level template renders 4 variants — `"chat"` (default, branches on `show_home`), `"persona"`, `"memory"`. Handlers set it; template dispatches to the right main-content partial. Keeps sidebar + modals + script blocks shared across all pages without duplication.
- **Restored 4 modals** stripped in Phase 3b: `editPersonaModal`, `editPersonaModelModal`, global `contextFilesModal`, per-persona `contextFilesModal`. Their `x-data` and event listeners are scoped per-modal; presence on non-hosting pages is harmless (they listen for window events and stay closed).
- **URLs hardcoded from Python's `urls.py`:** `/settings/save-persona/`, `/settings/create-persona/`, `/settings/save-persona-model/`, `/settings/save-persona-thread-defaults/`, `/settings/clear-persona-thread-defaults/`, `/settings/available-models/`, `/persona/context/*`, `/memory/*` (update/seed/wipe/modify/save-settings/update-status), `/settings/context/*`. Handlers for these land in 4c (persona + context) and 5/6 (memory + settings save); the stubs registered in Phase 3c keep 4xx-free until real handlers ship.
- **Tera gotchas (same lessons as 3b):** `{% include %}` can't take a variable path, so the shell uses explicit `{% if %}` branches. `{% set %}` + include passes component params to `select_dropdown.html` (no `with` clause in Tera). Optional values normalized at the top of each partial to avoid undefined-variable errors (Tera is stricter than Django).
- **Render tests (4 new, 13 total for `template_render.rs`):** persona page with personas + modals, memory page with empty memory, memory page with content (markdown renders `<li>`), context files modal renders title/description/tabs/nested browser.

**Phase 4a outcome:**

- **`services/persona.rs`** owns `data/personas/{name}/` — directory, identity markdown, **and** `config.json`. Public API: `valid_persona_name`, `list_personas`, `persona_exists`, `load_identity`, `get_preview`, `save_identity`, `create_persona`, `delete_persona`, `rename_persona`, `load_persona_config`, `save_persona_config`. Rename orchestrates the 4-way cascade (dir → memory file → persona user-context dir → sessions via `session::update_persona_across_sessions`); best-effort after step 1, logging warnings but not rolling back.
- **`services/context_files.rs`** — `ContextScope` struct with `global(data_dir)` and `persona(data_dir, name)` constructors. Owns `config.json` under each scope, which unifies uploaded-file state AND local-directory state (matches Python; cleaner than splitting). Methods for upload / delete / toggle / get-content / save-content on uploaded files; add / remove / list / toggle / refresh / get-content on local directories. `load_enabled_context()` concatenates everything with the Django-matching header format (`--- USER CONTEXT FILES ---`, `--- filename ---`, etc.).
- **`services/local_context.rs`** — stateless filesystem primitives: `validate_directory_path` (blocks paths inside `data/`), `scan_directory` (non-recursive, 200-file cap, `.md`/`.txt` only), `read_file` (`String::from_utf8_lossy` for robustness), `browse_directory` (for the directory-picker modal). Holds no persistent state — enabled flags live in context_files.rs's `ScopeConfig`.
- **`services/prompt.rs` expanded** to the full CLAUDE.md context order: persona identity → persona context (uploaded + local) → global context (uploaded + local) → scenario (roleplay only) → thread memory → persona memory (chatbot only). Persona memory is a file read — writes come in Phase 5's `memory_manager`. Missing persona still emits the "Persona not found" warning sentinel Python produced.
- **Return-convention consistency maintained:** service-layer functions follow the same Option/bool/Result<(),Error> pattern established in Phase 2. Persona uses `thiserror`-backed `PersonaError` for create/delete/rename because there are multiple distinct failure reasons callers may want to surface differently; context_files sticks with Option/bool because callers just log-and-ignore.
- **Integration tests added (24 new, 69 total):**
  - `tests/persona.rs` (9 tests): name validation, create/list/delete roundtrip, duplicate rejection, delete cascades memory + persona context, rename cascades directory + memory + context + sessions, config roundtrip preserves unknown keys and omits None fields, same-name rename no-op, target-collision rejection.
  - `tests/context_files.rs` (10 tests): upload/list/toggle/delete, traversal sanitization, enabled-context header + bodies, disabled files excluded, persona-scope header label, empty scope empty output, content roundtrip, local-directory add/remove, local content in enabled context, toggled-off local file excluded.
  - `tests/prompt_assembly.rs` (5 tests): chatbot has all six sections in CLAUDE.md order, roleplay suppresses persona memory even when the file exists, chatbot without memory file omits section, missing persona emits warning sentinel, empty scenario not emitted in roleplay.
- **Handlers not yet wired** — `AppState` unchanged; 4c adds the chat/session handler integration and the new `/persona/*`, `/memory/`, `/context/local/*` endpoints.

---

### Phase 5 — Memory system (HIGHEST RISK PHASE)

This is the single highest-risk phase. Allocate the most time. Review concurrency carefully.

**In progress.** Split into three sub-commits following the Phase 3/4 pattern:
- **5a (done 2026-04-22):** service layer — `services/{memory,thread_memory}.rs`, plus `session::list_persona_threads` (ported from `chat/utils.py`). No handlers, no scheduler. 38 new integration + unit tests; 111 tests total across the crate.
- **5b (done 2026-04-22):** memory worker — `services/memory_worker.rs` + two `tokio::spawn`ed scheduler tasks. Per-persona + per-session "already running" mutex registries distinct from the session-JSON lock. Manual dispatch (`start_manual_update`, `start_modify_update`, `start_seed_update`, `start_thread_memory_update`), status tracking, `reschedule_thread_next_fire` hook, scheduler lifecycle via `watch` channel. `MemoryWorker` wired into `AppState`; schedulers spawn in `main.rs`. 16 new integration tests; 127 total.
- **5c:** handlers + wire-up — `/memory/*` (update/wipe/modify/seed/save-settings/status), `/session/thread-memory/*` (update/status/settings save+reset), ctrl_c → graceful scheduler shutdown.

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

**Phase 5a outcome:**

- **`services/memory.rs`** owns `data/memory/{name}.md` end-to-end. Public surface: `get_memory_content`, `save_memory_content`, `delete_memory`, `rename_memory`, `list_persona_memories`, `get_memory_model` (fallback chain: explicit `MEMORY_MODEL` → persona's `model` → default), plus the three LLM-driven ops `update_memory`, `seed_memory`, `modify_memory` — all sharing a private `merge_memory` helper. The memory-merge prompt is ported verbatim from Python; the chatbot/roleplay variant split lives in `thread_memory` instead.
- **`services/thread_memory.rs`** is stateless — returns merged summary text; doesn't write files. Owns the constants (`DEFAULT_THREAD_MEMORY_SIZE=4000`, `_INTERVAL_MINUTES=0`, `_MESSAGE_FLOOR=4`), `EffectiveThreadMemorySettings` struct (concrete u32s, no Option), `resolve_settings(session, persona_cfg)` walking per-thread override → persona default → global, `resolve_persona_defaults` for the persona-settings form, `filter_new_messages(&[Message], updated_at) -> Vec<Message>`, and `merge<L: ChatLlm>(...) -> Option<String>` dispatching on `Mode` for chatbot vs. roleplay prompts.
- **`session::list_persona_threads`** (new, replaces Python's `chat/utils.py::aggregate_all_sessions_messages`): moved into `services/session.rs` where the session schema already lives. Takes `persona`, `max_threads`, `messages_per_thread` caps. Returns newest-first by file mtime; skips roleplay and empty threads. Read without per-session locks, matching `list_sessions`'s convention for sidebar reads.
- **SoC fixes landed alongside the port** (small things Python had as artifacts):
  1. `aggregate_all_sessions_messages` lived in `chat/utils.py` but did raw session JSON reads + schema-aware filtering — session-ownership work outside the session service. In Rust the equivalent lives on `session.rs` alongside `list_sessions`.
  2. `memory_manager.py::_safe_persona_name` silently mapped bad names (`"../escape"`) to sanitized ones and proceeded. In Rust, every memory.rs public entry short-circuits on `persona::valid_persona_name`, same pattern as `session::valid_session_id`.
  3. `persona.rs` previously inlined the memory-file rename/delete paths via a private `memory_file()` helper. That's been removed; the cascade in `delete_persona` / `rename_persona` now calls `memory::delete_memory` / `memory::rename_memory`. `data/memory/` has exactly one writer.
- **Return conventions preserved** from Phases 2–4: `String` (empty on failure) for reads, `bool` for writes, `Option<String>` for "merged text on success, None on failure" (what `thread_memory::merge` returns — the worker decides whether to persist). Errors log via `tracing` rather than propagate up.
- **`PersonaConfig`'s memory fields** added in Phase 4 (`user_history_max_threads`, `user_history_messages_per_thread`, `memory_size_limit`, `auto_memory_interval`, `auto_memory_message_floor`) are now consumed by the memory service + thread-memory resolver. No schema changes in 5a.
- **Tests (38 new, 111 total):**
  - `tests/memory_service.rs` (17 tests): save/get roundtrip, invalid-name rejection at every entry, missing-file delete, rename + missing-source no-op, list alphabetical, model fallback chain, empty-threads short-circuit, prompt shape (display name, thread formatting, roleplay section, size target), size_limit=0 omits size target, seed/modify label differences, modify refuses when no existing memory, short-response safety check + threshold, LLM-error preserves existing file, first-run placeholder.
  - `tests/thread_memory_service.rs` (9 tests): empty new-messages short-circuit, chatbot uses chatbot prompt + injects persona memory, chatbot omits persona memory section when empty, roleplay uses roleplay prompt + ignores persona memory even if passed, short-response rejection, short-response accepted when no existing, LLM error → None, first-run placeholder, size_limit=0 omits size target.
  - `tests/session.rs` (+4 tests): `list_persona_threads` filters persona + skips roleplay, per-thread message cap keeps most-recent, total-thread cap sorts newest-first by mtime, missing-dir returns empty.
  - Module-level unit tests in `thread_memory.rs` (+5): settings resolver three-tier walk with all-None override no-op, filter_new_messages cutoff + empty + missing-timestamp, transcript role labeling, chatbot prompt persona-memory presence/absence, roleplay prompt omits perspective rules.
  - Module-level unit tests in `memory.rs` (+2): display name formatting, model fallback chain (including "empty string is not set" Python-or semantics).
- **No handler or worker wiring yet.** `AppState` unchanged. 5b adds the worker + its scheduler tasks, then 5c replaces the existing `stubs::not_implemented` routes for `/memory/*` and `/session/thread-memory/*`.

**Phase 5b outcome:**

- **`services/memory_worker.rs`** owns coordination for both memory pipelines. One `MemoryWorker` struct wrapping `Arc<Inner>` — cloneable, lives on `AppState::memory`. Inner holds four scheduler-state maps plus four lock/status maps, all `StdMutex<HashMap>` with guard scopes that never cross `.await`.
- **Two lock namespaces, deliberately separate:**
  - `persona_locks` — per-persona `TokioMutex` "already running" coordination for cross-thread persona memory. Held across the LLM call; this lock IS the coordination mechanism (not a file lock).
  - `session_locks` — per-session `TokioMutex` "already running" coordination for thread memory. **Distinct from** `session::SESSION_LOCKS`. Held across the LLM call; the session-JSON lock is never ours to hold.
- **The invariant from CLAUDE.md / known hard spot #1 is preserved structurally.** In `run_thread_memory_update`: we acquire the thread-memory mutex, call `session::load_session` (which acquires + drops the session-JSON lock internally), call the LLM (session-JSON lock free), then call `session::save_thread_memory` (ditto). We never hold the session-JSON lock across an `.await` that touches the LLM. A regression test (`session_lock_not_held_across_llm_call`) starts a thread-memory update with a slow fake LLM and proves `session::save_draft` on the same session completes within 500ms while the LLM is in flight.
- **Public API mirrors Python names:** `start_{manual,modify,seed}_update(state, persona, …)`, `start_thread_memory_update(state, session_id, source)`, `get_{update,thread_update}_status`, `reschedule_thread_next_fire`, `start_schedulers` / `stop_schedulers`. Dispatch returns `bool` (false when an update is already running — fast 409 path for handlers).
- **Core work functions are generic over `ChatLlm`** so tests inject a `FakeLlm` and run the full pipeline without a network: `run_memory_update<L>`, `run_modify_memory<L>`, `run_seed_memory<L>`, `run_thread_memory_update<L>`. The public `start_*` methods build an `LlmClient` from config and spawn these.
- **Manual vs auto fallback preserved:** `run_thread_memory_update` with `UpdateSource::Manual` and no new messages reprocesses the whole thread so the user can refresh after changing `size_limit` / prompts; `UpdateSource::Auto` in the same state short-circuits with a "no new messages" completed status (matches Python's `memory_worker.py:657`).
- **Schedulers** run as two `tokio::spawn` tasks. Loop: tick → compute next-due → `tokio::select!` on `sleep(next)` vs. `watch::Receiver::changed()`. Shutdown is cooperative: `watch::Sender::send(true)` tells both loops to exit at their next select; `stop_schedulers` awaits the handles (a scheduler mid-LLM-call completes before returning — matches Python's `join(timeout=15)` semantics without the timeout fallback).
- **Persona scheduler fires synchronously** (awaits `run_memory_update`); **thread-memory scheduler dispatches asynchronously** (calls `start_thread_memory_update` which spawns). Asymmetry matches Python: thread memory can have many concurrent sessions due for updates; persona memory is rate-limited per-persona so serializing across personas is acceptable.
- **Caches match Python's mtime-keyed pattern:** `persona_count_cache` (for the persona scheduler's message-floor counter) and `thread_scheduler_cache` (for the thread-memory scheduler's per-session view). Each sweep prunes entries whose session files are gone. Never reparse JSON when mtime is unchanged.
- **Interval clamping at `[5, 1440]` minutes** lives in `interval_clamp()` and applies at every fire site (persona scheduler, thread scheduler, `reschedule_thread_next_fire`).
- **AppState + main wiring:** `AppState` gains a `memory: MemoryWorker` field. `main.rs` constructs `MemoryWorker::new()` alongside the HTTP client and calls `state.memory.start_schedulers(state.clone())` before `axum::serve`. Scheduler handles stored in a `let _scheduler_handles = …` for now — 5c wires a `tokio::signal::ctrl_c` handler that calls `stop_schedulers`.
- **`tokio` test-util dev-dep added** so tests can use `tokio::time::pause()` + `advance()`. Lets the scheduler + concurrent-update tests run in simulated time deterministically.
- **Tests (+16, 127 total):** persona-memory status transitions, empty-threads completed-with-message path, concurrent-manual-update rejection, modify refuses without existing memory, seed writes+completes, thread-memory writes session fields with correct `updated_at` cutoff, missing-session failed status, manual reprocess fallback, auto "no new messages" path, roleplay excludes persona memory from prompt (capturing-LLM verifies), **session-JSON-lock-not-held-across-LLM regression guard**, `start_manual_update` pre-check rejection, scheduler defer-when-floor-unmet, scheduler graceful shutdown, `reschedule_thread_next_fire` no-panic round trip, thread-memory settings resolve through persona default + override.

**Note (flagged to user, not blocking):** Scheduler ticks reload `config::load_config` + `persona::load_persona_config` per persona. Matches Python's "no-restart-needed" semantics for settings changes. Cheap at localhost scale (2+N JSON reads every ≥10s), so preserved as-is.

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
