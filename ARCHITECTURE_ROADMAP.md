# Liminal Salt — Architecture Roadmap

**Created:** April 14, 2026
**Updated:** April 15, 2026
**Status:** SoC refactor complete, planning Rust migration
**Scope:** Rust backend migration → Tauri desktop app

---

## Roadmap Overview

The SoC refactor is complete. The codebase now has clean service interfaces with zero direct file I/O in views, proper Django decorators, Alpine event-driven modals, and no inline JS/CSS in templates. The service layer maps directly to Rust modules.

The project will evolve through two remaining milestones:

| Milestone | What | Outcome |
|-----------|------|---------|
| **Milestone 1: Rust Backend** | Rewrite backend in Rust (Axum + Tera), drop Django | Same app, compiled backend, still browser-based |
| **Milestone 2: Tauri Desktop App** | Wrap Rust backend in Tauri | Single native binary (~5-15MB), standalone desktop app |

```
Current State             Milestone 1                  Milestone 2
Django + Services    →    Rust Backend            →    Tauri Desktop App
─────────────────         ────────────                 ─────────────────
Clean service layer       Axum + Tera                  Native window
Python 3.x + Django       reqwest → OpenRouter         In-process Axum
Browser access            tokio async                  Single binary
                          Browser access               ~5-15MB
```

Each milestone is independently valuable — the app works at every stage.

---

# Milestone 1: Rust Backend Migration

Replace the Python/Django backend with Rust while keeping the frontend (HTMX, Alpine.js, Tailwind, all JS) completely unchanged. The app continues to run in the browser during this phase.

## Target Stack

| Concern | Django (current) | Rust (target) |
|---------|-----------------|---------------|
| Web framework | Django | Axum |
| Templating | Django templates | Tera (Jinja2-like syntax) |
| HTTP client | `requests` | `reqwest` |
| JSON handling | `json` stdlib | `serde` + `serde_json` |
| Async runtime | threading (`memory_worker.py`) | `tokio` (async tasks) |
| Static files | whitenoise | `tower-http::ServeDir` |
| CSRF protection | Django middleware | Custom middleware or `axum-csrf` |
| Sessions | Django signed cookies | `tower-sessions` or signed cookies via `cookie` crate |
| Markdown | `python-markdown` | `pulldown-cmark` |
| WSGI server | waitress | Built into Axum (hyper) |

## Service Module Mapping

Each Python service maps directly to a Rust module:

| Python Service | Rust Module | Key Types |
|---------------|-------------|-----------|
| `session_manager.py` | `services/session.rs` | `Session`, `SessionManager` |
| `persona_manager.py` | `services/persona.rs` | `Persona`, `PersonaManager` |
| `context_files.py` | `services/context.rs` | `ContextFileManager<Scope>` |
| `chat_core.py` | `services/chat.rs` | `ChatCore`, `Message` |
| `memory_manager.py` | `services/memory.rs` | `MemoryManager` |
| `memory_worker.py` | `services/memory_worker.rs` | `tokio::spawn` tasks instead of threads |
| `context_manager.py` | `services/prompt.rs` | `PromptBuilder` |
| `llm_client.py` | `services/llm.rs` | `LlmClient`, `LlmError` |
| `summarizer.py` | `services/summarizer.rs` | `generate_title()` |
| `config_manager.py` | `services/config.rs` | `AppConfig` |

### Notes on the Mapping

- `SessionManager` has clean read/write/mutate functions with `flush+fsync` — port these as `fn load_session()`, `fn create_session()`, etc. with `serde_json` for serialization.
- `PersonaManager.rename_persona()` orchestrates 5 side effects — in Rust this becomes a single method that calls the other services, same pattern but with `Result<(), PersonaError>` return types.
- `ContextFileManager` is a class parameterized by `base_dir` and `scope_label` — in Rust this becomes a generic struct `ContextFileManager<S: Scope>` or simply a struct with a `base_dir: PathBuf` field.
- `MemoryWorker` uses Python threading — replace with `tokio::spawn` tasks. The per-persona lock pattern maps to `tokio::sync::Mutex<HashMap<String, ()>>` or similar.

## Template Migration

Tera syntax is close to Django's. Most conversions are mechanical:

| Django | Tera | Notes |
|--------|------|-------|
| `{% extends "base.html" %}` | `{% extends "base.html" %}` | Identical |
| `{% include "icons/x.html" %}` | `{% include "icons/x" %}` | Same concept |
| `{% for item in list %}` | `{% for item in list %}` | Identical |
| `{% if condition %}` | `{% if condition %}` | Identical |
| `{{ value\|markdown }}` | `{{ value \| markdown }}` | Register as custom Tera filter |
| `{{ value\|display_name }}` | `{{ value \| display_name }}` | Register as custom Tera filter |
| `{% url 'route_name' %}` | Hardcode paths or build a `url_for()` Tera function | Most significant difference |
| `{% csrf_token %}` | Custom Tera function or middleware injection | Needs manual implementation |

### Template Considerations

- The `#app-urls` div in `base.html` uses `{% url %}` tags — in Tera, either hardcode the paths (they're stable) or register a `url_for()` function.
- The `data-*` attribute pattern for passing data to Alpine components works identically — Tera renders the attributes, Alpine reads them. No change needed.
- Context file data is passed via hidden `<div>` elements with `data-files` JSON attributes — same pattern in Tera.

## Route Mapping

Django URL routing → Axum router:

```rust
// Simplified example
let app = Router::new()
    .route("/", get(index))
    .route("/chat/", get(chat))
    .route("/chat/send/", post(send_message))
    .route("/chat/switch/", post(switch_session))
    .route("/chat/retry/", post(retry_message))
    .route("/memory/", get(memory))
    .route("/memory/update/", post(update_memory))
    .route("/settings/", get(settings))
    .route("/api/themes/", get(get_available_themes))
    // ... all 40+ routes
    .layer(csrf_layer)
    .layer(session_layer)
    .with_state(app_state);
```

All POST-only routes currently use `@require_POST` in Django — in Axum these are naturally enforced by using `.route("/path/", post(handler))` instead of `.route("/path/", get(handler).post(handler))`.

## Migration Strategy

Port one service at a time, validating against the existing test checklist in CLAUDE.md after each:

1. **Project scaffold** — Cargo workspace, Axum server, Tera templates, static file serving
2. **Config + LLM client** — `config.rs`, `llm.rs` (foundational, everything depends on these)
3. **Session manager** — `session.rs` (JSON file I/O, the core data model)
4. **Chat views** — `chat.rs` routes + `ChatCore` service (the main user flow)
5. **Persona + context** — `persona.rs`, `context.rs` (CRUD operations)
6. **Memory system** — `memory.rs`, `memory_worker.rs` (background tasks via tokio)
7. **Settings + API** — remaining routes
8. **Template conversion** — migrate all `.html` templates from Django syntax to Tera
9. **Frontend validation** — verify all HTMX interactions, Alpine components, themes work identically

## Frontend: What Doesn't Change

- `utils.js` — identical (including `getAppUrl()`, `initMemoryView()`, HTMX CSRF config)
- `components.js` — identical (all Alpine components, event dispatch pattern)
- `input.css` / `output.css` — identical
- All Alpine.js components — identical
- All HTMX attributes in templates — identical
- All SVG icons — identical
- All theme JSON files — identical

The frontend doesn't know or care that the server language changed. It sends HTTP requests and receives HTML fragments.

## Milestone 1 Success Criteria

- All 40+ routes return identical HTML to their Django equivalents
- All HTMX interactions work (partials, swaps, triggers)
- All Alpine.js components function correctly
- All 16 themes load and apply correctly
- Memory background updates work via tokio tasks
- OpenRouter API calls work through reqwest
- JSON session files and markdown memory files are read/written in the same format (data portability)
- No Python dependency remains
- App serves on localhost, accessible via browser

---

# Milestone 2: Tauri Desktop App

Wrap the Rust backend in Tauri to produce a standalone native desktop app. Since the backend is already Rust (from Milestone 1), Axum runs in-process — no child process spawning, no bundled runtimes.

## Architecture

```
┌──────────────────────────────────────┐
│           Tauri App (single binary)  │
│                                      │
│  ┌────────────────────────────────┐  │
│  │  Axum Server (in-process)      │  │
│  │  All services from Milestone 1 │  │
│  │  Serves on 127.0.0.1:{port}   │  │
│  └────────────────────────────────┘  │
│                                      │
│  ┌────────────────────────────────┐  │
│  │  OS Native Webview             │  │
│  │  WebKit (macOS/Linux)          │  │
│  │  WebView2 (Windows)            │  │
│  │                                │  │
│  │  Loads localhost app            │  │
│  │  HTMX + Alpine + Tailwind     │  │
│  └────────────────────────────────┘  │
└──────────────────────────────────────┘
```

## New Files

```
src-tauri/
├── Cargo.toml           # Tauri + app dependencies
├── tauri.conf.json      # Window size, title, app metadata, icons
├── build.rs             # Build script
├── src/
│   └── main.rs          # Tauri setup, Axum lifecycle, window management
└── icons/               # Platform-specific app icons
    ├── icon.ico          # Windows
    ├── icon.icns         # macOS
    └── icon.png          # Linux
```

## Implementation Scope

| Task | Details |
|------|---------|
| Tauri scaffold | `cargo tauri init`, configure `tauri.conf.json` |
| Axum integration | Start Axum server on a dynamic port in `main.rs` setup hook |
| Window management | Single window pointing at `http://127.0.0.1:{port}` |
| Lifecycle | Axum starts with app, shuts down on window close |
| Data directory | Use Tauri's `app_data_dir()` for sessions, memory, config |
| App icons | Design and generate platform-specific icons |
| Build pipeline | `cargo tauri build` for each target platform |

## Data Directory Strategy

The app stores all user data in `data/` relative to the project root. In a desktop app, this moves to the OS's standard app data location. Tauri provides `app_data_dir()` to resolve this at runtime.

| Platform | Path |
|----------|------|
| macOS | `~/Library/Application Support/com.liminalsalt.app/` |
| Windows | `C:\Users\<user>\AppData\Roaming\com.liminalsalt.app\` |
| Linux | `~/.local/share/com.liminalsalt.app/` |

The directory structure inside is identical to what exists today:

```
{app_data_dir}/
├── config.json
├── sessions/
│   └── session_*.json
├── personas/
│   └── assistant/
│       ├── identity.md
│       └── config.json
├── user_context/
│   ├── config.json
│   └── personas/
│       └── [persona_name]/
└── memory/
    └── {persona_name}.md
```

Same flat files, same formats — just a different root path. This works because:

- **No database to install or manage** — JSON and markdown are portable and self-contained
- **User-visible data** — users can back up their data by copying a folder
- **Zero migration** — existing `data/` contents can be dropped into the app data dir on first launch
- **Milestone 1 preparation** — the Rust `AppConfig` should resolve `data_dir` once at startup and pass it to all services, so the Tauri switch is a one-line change to the path source

If session volume ever becomes a performance concern (thousands of files, slow directory scans), SQLite would be the natural step up — still a single file, no server process, excellent Rust support via `rusqlite`. But for single-user local usage, flat files are the right fit.

## Platform Outputs

| Platform | Format | Expected Size |
|----------|--------|--------------|
| macOS | `.dmg` | ~5-15MB |
| Windows | `.msi` / `.exe` installer | ~5-15MB |
| Linux | `.deb` / `.AppImage` | ~5-15MB |

## Milestone 2 Success Criteria

- App launches as a native window (no browser, no address bar)
- Single binary, no external dependencies
- Binary size under 20MB
- Native window controls (minimize, maximize, close)
- App icon in taskbar/dock
- Clean shutdown (Axum stops when window closes)
- Data persists in platform-appropriate app data directory
- All functionality identical to browser version
- Builds for macOS, Windows, and Linux
