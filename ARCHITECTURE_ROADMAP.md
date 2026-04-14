# Liminal Salt — Architecture Roadmap

**Created:** April 14, 2026
**Status:** Planning
**Scope:** SoC refactoring → Rust backend migration → Tauri desktop app

---

## Roadmap Overview

This project will evolve through three major milestones:

| Milestone | What | Outcome |
|-----------|------|---------|
| **Milestone 1: SoC Refactor** | Extract services, clean views, fix frontend discipline | Clean service interfaces ready to port |
| **Milestone 2: Rust Backend** | Rewrite backend in Rust (Axum + Tera), drop Django | Same app, compiled backend, still browser-based |
| **Milestone 3: Tauri Desktop App** | Wrap Rust backend in Tauri | Single native binary (~5-15MB), standalone desktop app |

```
Milestone 1              Milestone 2                  Milestone 3
SoC Refactor       →     Rust Backend            →    Tauri Desktop App
─────────────            ────────────                  ─────────────────
Clean services           Axum + Tera                   Native window
Clear interfaces         reqwest → OpenRouter          In-process Axum
Django still runs        tokio async                   Single binary
Browser access           Browser access                ~5-15MB
```

Each milestone is independently valuable — the app works at every stage.

---

# Milestone 1: Separation of Concerns Refactor

---

## Overview

This document outlines every separation-of-concerns violation in the codebase and the work required to fix them. The refactoring is organized into 7 phases, ordered by impact and dependency (later phases build on earlier ones).

---

## Phase 1: Extract `SessionManager` Service

**Why first:** Session I/O is the single biggest violation — 12 direct `json.load()`/`json.dump()` operations scattered across `chat/views/chat.py`. Every other view-layer cleanup depends on having this service in place.

**New file:** `chat/services/session_manager.py`

### Functions to Extract

| Current Location | Line(s) | Operation | New Method |
|-----------------|---------|-----------|------------|
| `chat()` | 117-125 | Load session JSON, read persona + draft | `load_session(session_id)` |
| `start_chat()` | 305-313 | Create initial session file with first message | `create_session(session_id, persona, message, model, title)` |
| `delete_chat()` | 355-356 | Delete session file | `delete_session(session_id)` |
| `delete_chat()` | 379-386 | Load next session after deletion | `load_session(session_id)` (reuse) |
| `toggle_pin_chat()` | 476-487 | Toggle pinned flag | `toggle_pin(session_id)` |
| `rename_chat()` | 517-528 | Update title | `rename_session(session_id, new_title)` |
| `save_draft()` | 558-568 | Save draft text | `save_draft(session_id, draft_text)` |
| `send_message()` | 599-606 | Create session during send | `create_session(...)` (reuse) |
| `send_message()` | 621-628 | Load persona from session | `get_session_persona(session_id)` |
| `send_message()` | 665-670 | Clear draft after send | `clear_draft(session_id)` |
| `retry_message()` | 754-779 | Remove last assistant message | `remove_last_assistant_message(session_id)` |
| `edit_message()` | 835-860 | Update last user message | `update_last_user_message(session_id, new_content)` |

**Additional methods needed:**

| Method | Purpose |
|--------|---------|
| `list_sessions(sessions_dir)` | Return sorted session list (used in sidebar building) |
| `get_session_path(session_id)` | Centralize path construction |
| `update_session_persona(session_id, old_name, new_name)` | Currently in `personas.py` `_update_sessions_persona()` (lines 50-63) |

### Implementation Notes

- Use atomic file writes (write to temp file, then rename) — the pattern already exists in `chat/utils.py`
- Replace all 7 bare `except:` clauses in `chat.py` with specific exceptions (`json.JSONDecodeError`, `IOError`, `FileNotFoundError`)
- The service should handle the `os.path.exists()` checks internally and return `None` or raise a clear exception

---

## Phase 2: Extract `PersonaManager` Service

**Why:** `chat/views/personas.py` has 9 direct filesystem operations for persona CRUD. This logic belongs in a service, not views.

**New file:** `chat/services/persona_manager.py`

### Functions to Extract

| Current Location | Line(s) | Operation | New Method |
|-----------------|---------|-----------|------------|
| `personas.py` `_update_sessions_persona()` | 50-63 | Iterate sessions, rename persona in each | Move to `SessionManager.update_persona_across_sessions()` |
| `persona_settings()` | 86-92 | Read persona preview (list .md files, read content) | `get_persona_preview(persona_name)` |
| `save_persona_file()` | 140-153 | Rename persona directory | `rename_persona(old_name, new_name)` |
| `save_persona_file()` | 177-182 | Find and read .md files | `save_persona_identity(persona_name, content)` |
| `create_persona()` | 256-259 | Create directory + write files | `create_persona(name, identity_content)` |
| `delete_persona()` | 329 | Delete persona directory | `delete_persona(persona_name)` |
| `delete_persona()` | 370-373 | Read replacement persona preview | `get_persona_preview()` (reuse) |
| `save_persona_model()` | 416-430 | Read/write persona config.json | Already partially in `context_manager.py` — consolidate |

### Orchestration

`rename_persona()` should handle all side effects in one call:
1. Rename directory (`shutil.move`)
2. Rename memory file (`memory_manager.rename_memory()`)
3. Rename persona context directory
4. Update all session files (`session_manager.update_persona_across_sessions()`)
5. Update default persona in config if needed

---

## Phase 3: Unify Context File Services

**Why:** `persona_context.py` and `user_context.py` are near-identical — 8 function pairs with the same logic differing only by storage path. ~250 lines of pure duplication.

### Duplicated Function Pairs

| Function | `persona_context.py` | `user_context.py` | Difference |
|----------|---------------------|-------------------|------------|
| `get_config_path()` | 32-34 | 20-22 | Path includes persona name vs not |
| `get_config()` | 37-43 | 25-31 | Takes `persona_name` param vs no param |
| `save_config()` | 46-50 | 34-38 | Takes `persona_name` param vs no param |
| `list_files()` | 53-72 | 41-57 | Persona-scoped dir vs global dir |
| `upload_file()` | 75-110 | 60-94 | Persona-scoped dir vs global dir |
| `delete_file()` | 113-141 | 97-124 | Persona-scoped dir vs global dir |
| `toggle_file()` | 144-174 | 127-156 | Persona-scoped dir vs global dir |
| `load_enabled_context()` | 221-257 | 159-192 | Header text differs |

### Approach

Create a generic `ContextFileManager` class in `chat/services/context_files.py`:

```python
class ContextFileManager:
    def __init__(self, base_dir, scope_label="USER"):
        self.base_dir = base_dir
        self.scope_label = scope_label

    def get_config(self): ...
    def save_config(self, config): ...
    def list_files(self): ...
    def upload_file(self, uploaded_file): ...
    def delete_file(self, filename): ...
    def toggle_file(self, filename): ...
    def load_enabled_context(self): ...
```

Then create two instances:
- `GlobalContextFiles = ContextFileManager(USER_CONTEXT_DIR, "USER")`
- Per-persona: instantiated with `ContextFileManager(persona_context_dir, "PERSONA")`

Delete `persona_context.py` and `user_context.py` after migration. Update views and `context_manager.py` to use the new class.

---

## Phase 4: Extract Shared View Utilities

**Why:** Several patterns are copy-pasted across 4-7 view files. Extract once, use everywhere.

### 4A: Persona Model Map Builder

**Problem:** 4 identical loops building `{persona: model}` dicts across `chat.py`.

**Locations:**
- `chat.py` lines 56-60
- `chat.py` lines 88-92
- `chat.py` lines 248-251 (`new_chat()`)
- `chat.py` lines 443-446 (`delete_chat()`)

**Solution:** Add to `context_manager.py` or a new `chat/services/view_helpers.py`:

```python
def build_persona_model_map(personas, personas_dir, default_model):
    return {p: get_persona_model(p, personas_dir) or default_model for p in personas}
```

### 4B: Model Fetching + Grouping Chain

**Problem:** The chain `fetch_available_models()` → `group_models_by_provider()` → `flatten_models_with_provider_prefix()` appears in 7 locations.

**Locations:**
- `personas.py` lines 202-206, 276-280, 360-364
- `settings.py` lines 112-116
- `core.py` lines 140-143, 177-187
- `api.py` lines 27-32

**Solution:** Single function in `chat/utils.py`:

```python
def get_formatted_model_list(api_key):
    models = fetch_available_models(api_key)
    grouped = group_models_by_provider(models)
    return flatten_models_with_provider_prefix(grouped)
```

### 4C: Theme List Builder

**Problem:** `_get_theme_list()` in `core.py` (lines 18-32) reads theme JSON files directly.

**Solution:** Move to a `ThemeManager` service or add to `config_manager.py`.

---

## Phase 5: Django Best Practices

### 5A: Add `@require_POST` / `@require_http_methods` Decorators

Replace manual `if request.method` checks with Django's built-in decorators.

| File | Function | Line | Action |
|------|----------|------|--------|
| `chat.py` | `switch_session()` | 222 | Add `@require_POST` |
| `chat.py` | `delete_chat()` | 345 | Add `@require_POST` |
| `chat.py` | `toggle_pin_chat()` | — | Add `@require_POST` |
| `chat.py` | `rename_chat()` | — | Add `@require_POST` |
| `chat.py` | `save_draft()` | — | Add `@require_POST` |
| `chat.py` | `retry_message()` | — | Add `@require_POST` |
| `chat.py` | `edit_message()` | — | Add `@require_POST` |
| `core.py` | `setup_wizard()` | 72, 133 | Handles both GET and POST — use `@require_http_methods(["GET", "POST"])` |
| `memory.py` | `update_memory()` | 90 | Add `@require_POST` |
| `memory.py` | `wipe_memory()` | 165 | Add `@require_POST` |
| `memory.py` | `modify_memory()` | — | Add `@require_POST` |
| `memory.py` | `save_memory_settings()` | — | Add `@require_POST` |
| `settings.py` | `save_settings()` | 80 | Add `@require_POST` |
| `settings.py` | `save_context_history_limit()` | — | Add `@require_POST` |
| `settings.py` | `validate_provider_api_key()` | — | Add `@require_POST` |
| `settings.py` | `save_provider_model()` | — | Add `@require_POST` |
| `personas.py` | `save_persona_file()` | — | Add `@require_POST` |
| `personas.py` | `create_persona()` | — | Add `@require_POST` |
| `personas.py` | `delete_persona()` | — | Add `@require_POST` |
| `personas.py` | `save_persona_model()` | — | Add `@require_POST` |

### 5B: Replace Bare `except` Clauses

All 7 are in `chat/views/chat.py`:

| Line | Context | Replace With |
|------|---------|-------------|
| 124 | Load session JSON | `except (json.JSONDecodeError, IOError, KeyError)` |
| 300 | Parse timezone | `except (ValueError, TypeError)` |
| 385 | Load session after delete | `except (json.JSONDecodeError, IOError, KeyError)` |
| 627 | Load persona from session | `except (json.JSONDecodeError, IOError, KeyError)` |
| 671 | Clear draft | `except (json.JSONDecodeError, IOError)` |
| 756 | Load session for retry | `except (json.JSONDecodeError, IOError)` |
| 837 | Load session for edit | `except (json.JSONDecodeError, IOError)` |

Most of these will be eliminated naturally when session I/O moves to `SessionManager` (Phase 1). The service should raise typed exceptions that views handle explicitly.

### 5C: Reduce Redundant `load_config()` Calls

**Problem:** `load_config()` reads and parses `config.json` from disk on every call. Some views call it 2-3 times. 35+ total calls across all views.

**Approach:** Load config once at the top of each view function, pass it through. For `setup_wizard()` in `core.py` (worst offender with 3 calls), load once and reuse.

---

## Phase 6: Frontend Cleanup

### 6A: Replace Inline Event Handlers with Alpine Directives

**20 inline handlers** across 8 template files that violate the project's "no inline JS" rule.

| Template | Handler | Replacement |
|----------|---------|-------------|
| `chat_main.html:9` | `onclick="openRenameModal(...)"` | `@click="$dispatch('open-rename-modal', {...})"` |
| `chat_main.html:42` | `onclick="copyMessageToClipboard(this)"` | `@click="copyMessageToClipboard($el)"` |
| `chat_main.html:50` | `onclick="retryLastMessage()"` | `@click="retryLastMessage()"` |
| `chat_main.html:61` | `onclick="editLastMessage(this)"` | `@click="editLastMessage($el)"` |
| `chat_main.html:102` | `onclick="scrollToBottom()"` | `@click="scrollToBottom()"` |
| `chat_main.html:129` | `onkeydown="handleTextareaKeydown(event)"` | `@keydown="handleTextareaKeydown($event)"` |
| `chat_main.html:130` | `oninput="autoResizeTextarea(this); saveDraftDebounced()"` | `@input="autoResizeTextarea($el); saveDraftDebounced()"` |
| `chat_home.html:34` | `oninput="saveNewChatDraftDebounced()"` | `@input="saveNewChatDraftDebounced()"` |
| `sidebar_sessions.html:37` | `onclick="openDeleteModal(...)"` | `@click="$dispatch('open-delete-modal', {...})"` |
| `sidebar_sessions.html:77` | `onclick="openDeleteModal(...)"` | `@click="$dispatch('open-delete-modal', {...})"` |
| `assistant_fragment.html:17` | `onclick="copyMessageToClipboard(this)"` | `@click="copyMessageToClipboard($el)"` |
| `assistant_fragment.html:24` | `onclick="retryLastMessage()"` | `@click="retryLastMessage()"` |
| `persona_main.html:8` | `onclick="openNewPersonaModal()"` | `@click="$dispatch('open-new-persona-modal')"` |
| `persona_main.html:46` | `onclick="openEditPersonaModal()"` | `@click="$dispatch('open-edit-persona-modal')"` |
| `persona_main.html:47` | `onclick="openEditPersonaModelModal()"` | `@click="$dispatch('open-edit-model-modal')"` |
| `persona_main.html:48` | `onclick="openDeletePersonaModal(...)"` | `@click="$dispatch('open-delete-persona-modal', {...})"` |
| `memory_main.html:49` | `onchange="this.closest('form').requestSubmit()"` | `@change="$el.closest('form').requestSubmit()"` |
| `memory_main.html:51` | `onclick="...click()"` | `@click="$refs.seedInput.click()"` |
| `memory_main.html:55` | `onclick="openWipeMemoryModal()"` | `@click="$dispatch('open-wipe-memory-modal')"` |
| `settings_main.html:126` | `onclick="openContextFilesModal()"` | `@click="$dispatch('open-context-files-modal')"` |

### 6B: Replace `window` Globals with Alpine Events

**6 modal components** store themselves on `window` for cross-component access.

**Current pattern:**
```javascript
// In component init:
window.deleteModalComponent = this;

// Called from templates:
function openDeleteModal(id, title) {
    window.deleteModalComponent.open(id, title);
}
```

**Target pattern:**
```javascript
// In component — listen for dispatched event:
init() {
    this.$el.addEventListener('open-delete-modal', (e) => {
        this.open(e.detail.id, e.detail.title);
    });
}

// From template:
@click="$dispatch('open-delete-modal', { id: '...', title: '...' })"
```

**Components to migrate:**

| Component | Global Variable | Event Name |
|-----------|----------------|------------|
| `deleteModal` | `window.deleteModalComponent` | `open-delete-modal` |
| `renameModal` | `window.renameModalComponent` | `open-rename-modal` |
| `wipeMemoryModal` | `window.wipeMemoryModalComponent` | `open-wipe-memory-modal` |
| `editPersonaModal` | `window.editPersonaModalComponent` | `open-edit-persona-modal` |
| `deletePersonaModal` | `window.deletePersonaModalComponent` | `open-delete-persona-modal` |
| `editPersonaModelModal` | `window.editPersonaModelModalComponent` | `open-edit-model-modal` |

### 6C: Eliminate Hardcoded URLs in JavaScript

**8+ hardcoded API paths** in `utils.js` that should come from Django templates via `data-*` attributes or a URL config object.

| Line | Hardcoded URL | Solution |
|------|--------------|----------|
| 45 | `/api/themes/` | Pass via `data-themes-url` on body/root element |
| 68 | `/api/save-theme/` | Pass via `data-save-theme-url` |
| 121 | `/static/themes/${themeId}.json` | Pass via `data-themes-static-path` |
| 982 | `/chat/save-draft/` | Pass via `data-save-draft-url` on chat form |
| 1122 | `/chat/retry/` | Pass via `data-retry-url` on chat container |
| 1191 | `/chat/edit-message/` | Pass via `data-edit-url` on chat container |
| 1197 | `/chat/` | Pass via `data-chat-url` on chat container |

**Approach:** Add a `<div id="app-urls" data-...>` block in `base.html` using Django `{% url %}` tags. JS reads from that element.

### 6D: Standardize Async Patterns

Mixed `.then()` chains and `async/await` in `utils.js` and `components.js`. Standardize on `async/await` with `try/catch`.

### 6E: Standardize CSRF Token Retrieval

~4 instances in `components.js` use `document.querySelector('[name=csrfmiddlewaretoken]').value` (throws if element missing) instead of the safe `getCsrfToken()` utility.

---

## Phase 7: Template Cleanup

### 7A: Extract Inline `<script>` and Alpine Fetch Logic

| Template | Line(s) | Content | Action |
|----------|---------|---------|--------|
| `base.html` | 25-34 | HTMX CSRF config | Move to `utils.js` |
| `memory_main.html` | 34-48 | Inline Alpine with `fetch()` | Extract to `components.js` |
| `settings_main.html` | 128-140 | Inline Alpine with `fetch()` | Extract to `components.js` |
| `persona_main.html` | — | `window.personaContextFilesData` | Pass via `data-*` attribute |
| `memory_main.html` | — | `window.contextFilesData` | Pass via `data-*` attribute |

### 7B: Remove Inline `style` Attribute

| Template | Line | Current | Replacement |
|----------|------|---------|-------------|
| `settings_main.html` | 127 | `style="display:none"` | Use `x-show` or `hidden` class |

---

## Execution Order & Dependencies

```
Phase 1: SessionManager ──────────────────────┐
Phase 2: PersonaManager (depends on Phase 1) ─┤
Phase 3: Unify Context Files ─────────────────┤── Can parallelize 3-5
Phase 4: Shared View Utilities ───────────────┤
Phase 5: Django Best Practices ───────────────┘
Phase 6: Frontend Cleanup (independent) ──────── Can run in parallel with 1-5
Phase 7: Template Cleanup (depends on Phase 6)
```

**Phase 1 is the prerequisite** — the `SessionManager` eliminates the most violations and is required by Phase 2 (persona rename needs to update sessions through the service).

**Phases 3-5** are independent of each other and can be done in any order after Phase 1.

**Phase 6** is entirely frontend and can be done in parallel with backend phases.

---

## Milestone 1 Success Criteria

When complete:
- **Zero** direct file I/O in any view file (`json.load`, `json.dump`, `os.path.exists`, `open()`, `shutil.*`)
- **Zero** bare `except:` clauses
- **Zero** inline event handlers in templates (all use Alpine directives)
- **Zero** `window.*` component references in JS (all use Alpine events)
- **Zero** hardcoded URLs in JS files
- **One** context file service (not two near-identical modules)
- **Every** POST-only view uses `@require_POST` or `@require_http_methods`
- **Every** model-fetching view calls a single utility function
- **Every** async operation uses `async/await` consistently

---

# Milestone 2: Rust Backend Migration

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

Each service extracted in Milestone 1 maps directly to a Rust module:

| Python Service | Rust Module | Key Types |
|---------------|-------------|-----------|
| `session_manager.py` | `services/session.rs` | `Session`, `SessionManager` |
| `persona_manager.py` | `services/persona.rs` | `Persona`, `PersonaManager` |
| `context_files.py` (unified) | `services/context.rs` | `ContextFileManager<Scope>` |
| `chat_core.py` | `services/chat.rs` | `ChatCore`, `Message` |
| `memory_manager.py` | `services/memory.rs` | `MemoryManager` |
| `memory_worker.py` | `services/memory_worker.rs` | `tokio::spawn` tasks instead of threads |
| `context_manager.py` | `services/prompt.rs` | `PromptBuilder` |
| `llm_client.py` | `services/llm.rs` | `LlmClient`, `LlmError` |
| `summarizer.py` | `services/summarizer.rs` | `generate_title()` |
| `config_manager.py` | `services/config.rs` | `AppConfig` |

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

- `utils.js` — identical
- `components.js` — identical
- `input.css` / `output.css` — identical
- All Alpine.js components — identical
- All HTMX attributes in templates — identical
- All SVG icons — identical
- All theme JSON files — identical

The frontend doesn't know or care that the server language changed. It sends HTTP requests and receives HTML fragments.

## Milestone 2 Success Criteria

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

# Milestone 3: Tauri Desktop App

Wrap the Rust backend in Tauri to produce a standalone native desktop app. Since the backend is already Rust (from Milestone 2), Axum runs in-process — no child process spawning, no bundled runtimes.

## Architecture

```
┌──────────────────────────────────────┐
│           Tauri App (single binary)  │
│                                      │
│  ┌────────────────────────────────┐  │
│  │  Axum Server (in-process)      │  │
│  │  All services from Milestone 2 │  │
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

The app currently stores all user data in `data/` relative to the project root. In a desktop app, this moves to the OS's standard app data location. Tauri provides `app_data_dir()` to resolve this at runtime.

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
- **Milestone 2 preparation** — the Rust `AppConfig` should resolve `data_dir` once at startup and pass it to all services, so the Tauri switch is a one-line change to the path source

If session volume ever becomes a performance concern (thousands of files, slow directory scans), SQLite would be the natural step up — still a single file, no server process, excellent Rust support via `rusqlite`. But for single-user local usage, flat files are the right fit.

## Platform Outputs

| Platform | Format | Expected Size |
|----------|--------|--------------|
| macOS | `.dmg` | ~5-15MB |
| Windows | `.msi` / `.exe` installer | ~5-15MB |
| Linux | `.deb` / `.AppImage` | ~5-15MB |

## Milestone 3 Success Criteria

- App launches as a native window (no browser, no address bar)
- Single binary, no external dependencies
- Binary size under 20MB
- Native window controls (minimize, maximize, close)
- App icon in taskbar/dock
- Clean shutdown (Axum stops when window closes)
- Data persists in platform-appropriate app data directory
- All functionality identical to browser version
- Builds for macOS, Windows, and Linux
