# CLAUDE.md

Navigation + conventions for Claude working in this repo. Written for Claude, not humans. Keep it tight — feature descriptions belong in commits.

## What this is

Liminal Salt — Rust + Axum + Tera web chatbot on top of OpenRouter. No database; state lives in JSON/Markdown files under `data/`. Single-process (hyper via Axum). HTMX + Alpine.js + Tailwind on the frontend. Current state is a browser-based Rust server; multi-provider LLM support, user-editable prompts, and Tauri desktop distribution are on the roadmap.

See [`docs/planning/ARCHITECTURE_ROADMAP.md`](docs/planning/ARCHITECTURE_ROADMAP.md) for the milestone plan, ordering rationale, and what's deliberately out of scope.

## Directory layout

```
crates/liminal-salt/          Sole crate. Workspace root is the repo root.
  src/
    main.rs                   Boot: state, layers, schedulers, ServeDir, graceful shutdown.
    lib.rs                    Exposes services, routes, handlers, middleware, AppState.
    routes.rs                 Router assembly.
    tera_extra.rs             Custom Tera filters (markdown, display_name, escapejs).
    services/                 All file I/O, LLM calls, locks, caches.
    handlers/                 Thin HTTP: parse → call service → render.
    middleware/               csrf, app_ready, session_state.
  templates/                  Tera. chat/, persona/, memory/, settings/, setup/, components/, icons.html.
  static/                     JS (utils.js, components.js), CSS, themes/*.json, favicon.
  default_personas/           Bundled personas copied into data/personas/ on first boot.
  tests/                      Integration tests. One file per service area.
data/                         Gitignored user state.
  config.json                 App config (snake_case keys): openrouter_api_key, model, theme, setup_complete, agreement_accepted, etc.
  sessions/session_*.json     Chat sessions.
  personas/{name}/            identity.md + config.json (model override, memory settings, thread defaults).
  memory/{name}.md            Per-persona cross-thread memory.
  user_context/               Global + per-persona uploaded files + local-directory refs.
AGREEMENT.md                  User agreement. Version in HTML comment on line 1.
docs/planning/                Roadmap + phase history.
```

## Services (`crates/liminal-salt/src/services/`)

| Module | Owns |
|---|---|
| `session.rs` | Session JSON CRUD. Reads and RMW'd writes hold a per-session `tokio::sync::Mutex`. Session-id regex, `now_timestamp()`. Returns `Result<_, SessionError>`. |
| `chat.rs` | `send_message(ctx, llm, input, skip_user_save) -> Result<String, ChatError>`. Loads via `session::load_session`, runs LLM with retry, persists via `session::save_chat_history`. Does NOT touch session JSON directly. |
| `thread_memory.rs` | Per-session summary merge. Stateless: returns merged text, worker persists. Settings resolver (per-thread → persona default → global). Two prompt variants (chatbot/roleplay). |
| `memory.rs` | Per-persona memory file I/O + LLM merge/seed/modify. Owns `data/memory/{persona}.md`. Returns `Result<(), MemoryError>`. `memory_file()` path builder is module-private; siblings use `get_memory_content`/`get_mtime`/`get_mtime_secs`. |
| `memory_worker.rs` | Two `tokio::spawn` schedulers (persona memory + thread memory). "Already running" mutex registries (`persona_locks`, `session_locks`) are **separate** from `session::SESSION_LOCKS` — collapsing them reintroduces the "lock across LLM call" bug. `MutexRecover` trait recovers StdMutex-guarded maps from poison. |
| `prompt.rs` | Assembles system prompt; owns `seed_default_personas`. Reads through other services' public APIs — never builds paths into another service's domain. |
| `context_files.rs` | `ContextScope { global, persona }`. Owns `data/user_context/**`. Uploaded files + local-directory refs unified under `ContextScopeError`. |
| `local_context.rs` | Stateless FS primitives for user-configured directories. `read_file` returns `Result<String, ReadError>` — rejects invalid UTF-8 loudly instead of lossy-replace, so the prompt doesn't silently get U+FFFD. |
| `persona.rs` | Persona CRUD + rename/delete cascade. Owns `data/personas/{name}/` **including** `config.json` (moved here from `context_manager` during Phase 4a). Returns `Result<_, PersonaError>`. |
| `llm.rs` | `ChatLlm` trait + `LlmClient` impl (OpenRouter). `LlmError`. The only outbound chat-completions path. |
| `config.rs` | `AppConfig` load/save, `is_app_ready`, agreement version parser. `data_dir()` is the **Tauri integration seam** — the one function that changes when wrapped in Tauri. |
| `summarizer.rs` | Title generation (one-shot, first-exchange). |
| `openrouter.rs` | Provider validation + model list fetch + pricing formatting. Uses the shared `AppState::http` client. |
| `themes.rs` | Theme listing — scans `static/themes/*.json`. |

## Handlers (`crates/liminal-salt/src/handlers/`)

- **chat.rs** — chat/send/switch/new/start/delete/pin/rename/save-draft/retry/edit, plus render helpers (`render_view`, `render_sidebar_fragment`, `base_chat_context`).
- **session.rs** — `/session/scenario/save/`, `/session/fork-to-roleplay/`.
- **thread_memory.rs** — `/session/thread-memory/{update,status,settings/save,settings/reset}`.
- **memory.rs** — persona memory page + update/wipe/modify/seed/save-settings/update-status.
- **persona.rs** — persona page + save/create/delete/save-model/save-thread-defaults/clear-thread-defaults.
- **context.rs** — uploaded files (global + persona) + local-directory ops. `scope_for(&state, persona)` picks the scope from an optional form field.
- **settings.rs** — settings page + save/validate-api-key/save-provider-model/save-context-history-limit.
- **setup.rs** — 3-step wizard. Uses `session_state::setup_step()` for page-refresh persistence.
- **api.rs** — `/api/themes/`, `/api/save-theme/`, `/settings/available-models/`.

## URL conventions

- `/chat/*` — chat lifecycle
- `/session/*` — per-session ops (scenario, thread memory, fork)
- `/memory/*` — persona memory + its context files
- `/persona/*` — persona CRUD + persona context files
- `/settings/*` — app settings + global context files
- `/context/local/*` — local-directory context (shared across scopes via optional `persona` form param)
- `/api/*` — JSON endpoints (themes, models)

POST-only is structural: `.route("/x/", post(handler))` rejects GET inherently.

## Session JSON schema

Written by `session.rs`. `skip_serializing_if = "Option::is_none"` keeps the on-disk shape clean.

| Field | Type | Notes |
|---|---|---|
| `title` | `String` | "New Chat" on create; auto-generated once on first reply unless `title_locked`. |
| `title_locked` | `Option<bool>` | `Some(true)` after `rename_session` or the one-shot auto-gen. Absent until finalized. |
| `persona` | `String` | Persona folder name. |
| `mode` | `Mode` (`"chatbot"` / `"roleplay"`) | Immutable after create. |
| `messages` | `Vec<Message>` | `{role, content, timestamp}`. |
| `draft` | `Option<String>` | Absent until first save. |
| `pinned` | `Option<bool>` | Absent until first pin. |
| `scenario` | `Option<String>` | Roleplay threads only. |
| `thread_memory` | `String` | Per-thread running summary. Omitted on disk when empty. |
| `thread_memory_updated_at` | `String` | Timestamp stamped at update *start* (wall-clock) — used as the next filter cutoff. Omitted when empty. |
| `thread_memory_settings` | `Option<ThreadMemorySettings>` | Per-thread override `{interval_minutes, message_floor, size_limit}`. Absent when merged values resolve to the persona/global defaults. |

## Non-obvious architectural invariants

**Timestamp canonical form.** Every message timestamp, `thread_memory_updated_at`, and every mtime-derived cutoff is `chrono::Utc::now().to_rfc3339_opts(SecondsFormat::Micros, true)` → `2026-04-22T12:34:56.123456Z`. Fixed-width, `DateTime::parse_from_rfc3339` round-trips, lexicographic compare matches chronological. Use `session::now_timestamp()` — don't hand-roll.

**Per-session lock — no lock held across LLM awaits.** `session::SESSION_LOCKS` is a `StdMutex<HashMap<String, Arc<TokioMutex<()>>>>`. Public session ops acquire briefly; the guard is always dropped before any `.await` that touches the LLM. The memory worker has its own separate "already running" mutex namespaces (`persona_locks` for cross-thread memory, `session_locks` for thread memory) — these ARE held across the LLM call (they're coordination, not file locks). The session-JSON lock is never ours to hold during the LLM call; `session::save_thread_memory` re-acquires it for the write.

**Session-id validation.** Every public function in `session.rs` that takes `session_id` short-circuits on `valid_session_id()` (regex `^session_\d{8}_\d{6}(?:_\d+)?\.json$`) and returns `Err(SessionError::InvalidId(...))`. Handlers can pass untrusted POST input straight through; invalid ids surface as 400s.

**Context assembly order** (`prompt::build_system_prompt`):
1. Persona identity `.md` files
2. Persona-scoped context files (uploaded + local dirs)
3. Global user context files (uploaded + local dirs)
4. Scenario — roleplay threads only
5. Thread memory — per-thread running summary
6. Persona memory — chatbot threads only (suppressed in roleplay for immersion)

**Thread memory ≠ persona memory.** Thread memory is per-session, stored inline on the session JSON, persisted via `session::save_thread_memory` (called by the worker). Persona memory is cross-thread, stored in `data/memory/{persona}.md`, written by `memory::save_memory_content`. Roleplay sessions are excluded from persona-memory aggregation and don't get persona memory injected into their prompt.

**Settings resolver pattern.** `thread_memory::resolve_settings(session, persona_config)` merges per-thread override → persona default (`default_thread_memory_settings`) → global fallback. Save endpoints compare submitted values to the resolved defaults and clear the override rather than persist a no-op (prevents the "Custom" badge from lighting up for identical-to-default values). Same pattern applies to persona `default_mode` — only `"roleplay"` is persisted as an override.

**App-access gate.** `middleware/app_ready.rs` redirects any non-exempt request to `/setup/` when `config::is_app_ready(&cfg)` is false — both `setup_complete == true` AND `agreement_accepted == current_agreement_version()` must hold. Exempt paths: `/setup/*`, `/static/*`, `/health`, `/api/themes/`. A missing or broken API key does NOT gate access; it surfaces as an in-chat error when the user tries to send.

**Agreement source of truth.** `AGREEMENT.md` at the repo root owns the user-agreement copy and version. The version lives in an HTML comment on line 1 (`<!-- version: X.Y -->`), invisible on GitHub, parsed by `config::current_agreement_version()` at startup via `LazyLock`. To force a re-prompt without redoing provider/model: edit the file *and* bump the comment; restart.

**Schedulers + caches.** `memory_worker.rs` spawns two tokio tasks: persona-memory (per-persona interval + message floor) and thread-memory (per-session effective settings). Both cache session-derived scheduler inputs keyed by mtime (`persona_count_cache`, `thread_scheduler_cache`) — never re-parse JSON when mtime is unchanged. Caches prune deleted sessions each sweep. `stop_schedulers` times out at 15s per handle so a scheduler mid-LLM-call doesn't block shutdown indefinitely.

**StdMutex poison recovery.** The worker's StdMutex-guarded maps (status, locks, next-fire, caches) use the file-local `MutexRecover::lock_recover()` extension trait — `unwrap_or_else(|e| e.into_inner())` — so a panicked task doesn't freeze subsequent status queries or scheduler ticks. The data is recoverable state (lookup maps, coordination metadata), not invariant-critical.

**Chat doesn't own the session file.** `chat::send_message` loads via `session::load_session`; persists via `session::save_chat_history`. Chat-owned fields (title, persona, messages) are written; every other field (mode, scenario, thread_memory, thread_memory_updated_at, thread_memory_settings, pinned, draft, title_locked) is preserved by the RMW.

**Error conventions.** Services return `Result<T, ServiceError>` with `thiserror`-derived enums (`SessionError`, `MemoryError`, `ContextScopeError`, `PersonaError`, `ChatError`, `LlmError`, `ReadError`). Handlers map variants to HTTP status codes:
- `InvalidId`, `InvalidFilename`, `InvalidPath`, `InvalidState` → 400
- `NotFound`, `NotTracked` → 404
- `ReadError::InvalidUtf8` → 422
- `Io`, `Llm`, `UnusableResponse`, `Corrupt` → 500

Best-effort scans (`list_sessions`, `list_persona_threads`, `list_themes`, `list_files`) stay `Vec<T>` — individual failures shouldn't fail the whole list. Simple attribute reads (`get_memory_content`, `persona::load_identity`) return `String` with "" as the null-object value; no Option wrapping needed at that layer.

**Tauri seam.** `config::data_dir()` is the single function that changes for the Tauri wrap — Tauri will have it return `app_data_dir()`. No other path literal in the crate hard-codes the data root.

## Separation of concerns — hard rules

Drift here breaks every other invariant (locks bypassed, ids unvalidated, fields clobbered).

**Ownership is exclusive.** Each file/directory under `data/` has exactly one writer.

| Resource | Sole writer |
|---|---|
| `data/sessions/*.json` | `session.rs` |
| `data/personas/{name}/` (dir + identity + config.json) | `persona.rs` |
| `data/memory/{name}.md` | `memory.rs` |
| `data/user_context/**` | `context_files.rs` |
| `data/config.json` | `config::save_config` |

Reads from any resource that might race with a writer go through that owner's public API (with lock acquisition). Reads that tolerate brief staleness (sidebar listing, scheduler scans) can read directly — but ONLY for best-effort summaries, never for data feeding an LLM call.

**Handlers do not do work.** Handlers parse requests, call a service, render a response. Forbidden in `handlers/`: `tokio::fs`, `serde_json::from_slice`/`to_vec` on data paths, direct `reqwest`, direct `LlmClient` instantiation, business-logic loops. If a handler needs something that's not in services yet, add it to the appropriate service.

**Services do not cross domains.** `memory.rs` does not read session JSON (inject `ThreadSnapshot`s instead). `thread_memory.rs` does not write files (it returns the merged text; the worker calls `session::save_thread_memory`). `chat.rs` does not touch session JSON directly. Cross-domain reads go through the owner's public API, not its private helpers — if you need a new accessor, add one to the owner.

**`llm::LlmClient::complete` is the only chat-completions path.** No service imports `reqwest` to hit `/v1/chat/completions` directly. `openrouter.rs` is the exception for model-list + key-validation endpoints; it uses the shared `AppState::http` client.

**Frontend mirrors the backend split.**
- `utils.js` = shared utility functions (CSRF, URLs, DOM helpers, timestamp/draft/scroll). No Alpine components.
- `components.js` = Alpine component definitions only, inside an `alpine:init` listener. No top-level helpers.
- Templates = markup + Alpine directives + data. No business logic, no inline handlers, no inline styles.

**Drift checklist — ask before writing code.**
1. Which service owns the resource I'm touching?
2. Does the function I need already exist there? If not, am I adding it there or smuggling it elsewhere?
3. If I'm in a handler: am I just parsing/calling/rendering?
4. If I'm in a service: am I reaching into another service's data? Use its public API.
5. If I'm in a template: am I doing anything that isn't "render this data" or "dispatch an event"?

When in doubt: add a method to the owning service, call it from the caller. Never "quick fix inline, refactor later."

## Code standards

**Rust.** Handlers are thin — no `tokio::fs`, `serde_json` on data paths, direct `reqwest`, or fresh `LlmClient` construction inside `handlers/`. All session writes use `write_all + sync_all` (no rename dance). Don't hold a lock across an `.await` that touches the LLM. `tracing::warn!` / `tracing::error!` for service-level failures; never `println!`. Timezone handling: store UTC, display in user's zone (user tz lives in `tower-sessions` under `user_timezone`). Panics reserved for genuine programmer errors (regex that proved valid at unit-test time, invariants that can't break without a bug); runtime failure paths return typed errors.

**JavaScript.** Only two files: `utils.js` (shared functions, runs at load) and `components.js` (Alpine `Alpine.data()` registrations inside an `alpine:init` listener). No inline `<script>` tags — HTMX-swapped fragments wire themselves via Alpine `x-data` (Alpine auto-inits new components on swap). Read `data-*` attributes in `init()` and store as instance properties — `this.$el` may not point at the component root inside event-handler methods. Use `getCsrfToken()` for CSRF, `getAppUrl(key, fallback)` for named routes. `async/await` + `try/catch` throughout; no `.then()` chains. Modal cross-talk via `window.dispatchEvent(new CustomEvent(...))` and `window.addEventListener` in `init()`.

**Templates.** No `onclick`/`onchange`/`oninput` — use Alpine `@click`/`@change`/`@input`. No `style=` — use Tailwind classes or `hidden`/`x-show`. Data flows to Alpine via `data-*` attributes; lists/objects go as JSON in `data-foo='{{ foo_json | safe }}'`. Tera gotchas: `{% import %}` must be the first non-content line; undefined variables hard-error (use `{% set var = var | default(value="") %}` at the top of includes); `slice` is Vec-only; `escapejs` is a custom filter that ports Django's semantic for attribute-safe string escaping.

## Dev commands

```bash
cargo run -p liminal-salt                                    # run the server (port 8420)
npm run dev                                                  # tailwind watcher + cargo run, concurrent
npm run vendor                                               # copy pinned htmx + alpine from node_modules into static/vendor/
cargo test -p liminal-salt                                   # integration + unit tests (tempdirs)
cargo clippy -p liminal-salt --all-targets -- -D warnings
node --check crates/liminal-salt/static/js/components.js     # JS syntax sanity
./scripts/bump-version.sh {patch|minor|major|set X.Y.Z}      # bump Cargo.toml + package.json + README + CHANGELOG
```

Setup wizard at `/setup/` on first launch.

## Git workflow

At the end of a task or phase: propose a terse commit message in the repo's style and ask before committing. Once the user approves, the commit/push loop (stage → commit → push) is pre-authorized — don't ask a second time for the same approval.

- **Commit style.** Lowercase first word. Optional conventional prefix (`docs:`, `fix:`, `refactor:`, `feat:`). Terse; focus on *why* when the *what* isn't self-evident from the diff. No `Co-Authored-By` trailer.
- **Version bumping.** `./scripts/bump-version.sh {patch|minor|major}`. Ask which bump type if ambiguous; default to `patch` for internal work and bug fixes, `minor` for user-visible features. A version bump is its own commit, created by the script.
- **One commit per logical unit.** If a task produced unrelated changes (e.g., a refactor plus an unrelated doc tweak), surface that and ask whether to split before committing.
- **Never without an explicit ask.** Force push, `reset --hard`, `--amend` of a pushed commit, `--no-verify`, branch deletion. Pre-commit hook failure → fix the underlying issue and create a new commit; don't skip the hook.

## When touching this repo

- New session field? Add to the schema table above. Ensure `Session::blank()` includes it and `session::save_chat_history`'s RMW preserves it (it should by default — the struct is loaded whole, fields are mutated selectively).
- New POST endpoint that takes a `session_id`? Route through a `session.rs` writer so validation + locking apply. Don't build paths from `session_path(sessions_dir, session_id)` in handlers.
- Writing a timestamp? `session::now_timestamp()`. Comparing? String compare works (fixed-width UTC).
- New Alpine component? Register in `components.js` under `alpine:init`. Template passes config via `data-*`; component stores those in `init()`.
- New persona-config key? Add it to `resolve_*` if it has a global fallback + persona-override shape, and clear it on save when submitted equals default.
- New service error variant? Add it to the enum via `thiserror`, then extend the handler `match` for a proper status code. Don't log-and-return-default unless the variant genuinely has no handler remediation.
