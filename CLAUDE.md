# CLAUDE.md - Project Overview & Developer Guide

**Last Updated:** April 15, 2026
**Project:** Liminal Salt - Multi-Session LLM Chatbot with Personas
**Status:** Production-ready Django application

---

## Table of Contents
1. [Project Overview](#project-overview)
2. [Architecture](#architecture)
3. [File Structure](#file-structure)
4. [Key Components](#key-components)
5. [Features](#features)
6. [How to Run](#how-to-run)
7. [Configuration](#configuration)
8. [Development Notes](#development-notes)

---

## Project Overview

**Liminal Salt** is a Python-based web chatbot application that connects to OpenRouter's API to provide LLM-powered conversations with persistent memory and multiple personas.

### Key Features

- **Multi-Session Management**: Create, switch between, and manage multiple chat sessions
- **Persona System**: Per-session persona selection with customizable personas and persona-specific context files
- **Per-Persona Living Memory**: Each persona maintains its own memory about the user, written from the persona's perspective, with incremental merge updates and background processing
- **Grouped Sidebar**: Collapsible persona-based organization of chat threads
- **Pinned Chats**: Pin important conversations to the top of the sidebar
- **Smart Titles**: Multi-tier auto-generation of session titles with artifact detection
- **Message Editing & Retry**: Edit last user message or retry last assistant response
- **Draft Saving**: Auto-save message drafts per session with debounced persistence
- **Timezone-Aware**: Current time context injected into system prompt with user timezone support
- **Markdown Rendering**: Assistant responses rendered with markdown via custom template filters
- **Memory View**: Dedicated pane with persona selector for viewing and managing per-persona memory
- **Persona Settings**: Dedicated page for managing personas and model overrides
- **Dynamic Theme System**: 16 color themes with dark/light modes, server-side persistence
- **SVG Icon System**: Consistent, theme-aware icons throughout the UI
- **Reactive UI**: HTMX-powered updates without full page reloads

### Technology Stack

- **Language:** Python 3.x
- **Web Framework:** Django 5.x (no database)
- **Frontend:** HTMX + Alpine.js
- **CSS Framework:** Tailwind CSS v4 with @tailwindcss/typography
- **Build Tools:** Node.js / npm for CSS compilation
- **API:** OpenRouter (LLM gateway)
- **HTTP Client:** requests
- **Markdown:** python-markdown (via custom Django template filter)
- **WSGI Server:** waitress (production), Django dev server (development)
- **Static Files:** whitenoise (production serving)
- **Data Storage:** JSON files for sessions, Markdown for memory and personas
- **Sessions:** Django signed cookie sessions (no database required)
- **UI Themes:** 16 color themes (Liminal Salt [default], Nord, Dracula, Gruvbox, Monokai, Solarized, Rose Pine, Tokyo Night, One Dark, Night Owl, Catppuccin, Ayu, Everforest, Kanagawa, Palenight, Synthwave 84)

---

## Architecture

### System Overview

```
┌─────────────────────────────────────────────────────────┐
│                    Django Web UI                        │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  │
│  │   Sidebar    │  │  Main Pane   │  │   Modals     │  │
│  │ - Sessions   │  │ - Chat       │  │ - New Chat   │  │
│  │ - Navigation │  │ - Memory     │  │ - Delete     │  │
│  │ - HTMX       │  │ - Settings   │  │ - Alpine.js  │  │
│  └──────────────┘  └──────────────┘  └──────────────┘  │
└────────────────────┬────────────────────────────────────┘
                     │ HTMX Requests
                     ▼
┌─────────────────────────────────────────────────────────┐
│                   Django Views (thin)                   │
│  - @require_POST on all POST-only endpoints             │
│  - No direct file I/O — delegates to services           │
│  - Coordinates between templates and services           │
└────────────────────┬────────────────────────────────────┘
                     │
                     ▼
┌─────────────────────────────────────────────────────────┐
│                 Business Logic (services/)              │
│  - SessionManager: Session file CRUD & mutations        │
│  - PersonaManager: Persona CRUD with side-effect mgmt   │
│  - ContextFileManager: Unified context file service     │
│  - ChatCore: LLM API calls, message history             │
│  - LLMClient: Shared OpenRouter API utility             │
│  - Summarizer: Title generation                         │
│  - MemoryManager: Per-persona memory updates            │
│  - MemoryWorker: Background threading & auto-updates    │
│  - ContextManager: System prompt assembly               │
│  - ConfigManager: Configuration handling                │
└────────────────────┬────────────────────────────────────┘
                     │
                     ▼
┌─────────────────────────────────────────────────────────┐
│                 OpenRouter API                          │
│  - LLM inference                                        │
│  - Supports multiple models                             │
└─────────────────────────────────────────────────────────┘
```

---

## File Structure

```
liminal-salt/
├── run.py                       # Simple launcher (auto-setup)
├── manage.py                    # Django entry point
├── config.json                  # API keys & app settings
├── requirements.txt             # Python dependencies
├── package.json                 # Node/Tailwind dependencies & scripts
├── package-lock.json            # npm lockfile
├── CLAUDE.md                    # This documentation
│
├── scripts/                     # Utility scripts
│   └── bump_version.py          # Version management & changelog
│
├── liminal_salt/                # Django project settings
│   ├── __init__.py              # Package version defined here
│   ├── settings.py              # Django configuration
│   ├── urls.py                  # Root URL routing
│   ├── wsgi.py                  # WSGI entry point
│   └── asgi.py                  # ASGI entry point
│
├── chat/                        # Main Django app
│   ├── __init__.py
│   ├── apps.py                  # App configuration
│   ├── urls.py                  # App URL routing
│   ├── utils.py                 # Shared helpers (config, model formatting, theme list)
│   │
│   ├── views/                   # View functions (thin — no direct file I/O)
│   │   ├── __init__.py          # Re-exports all views
│   │   ├── core.py              # index, setup_wizard
│   │   ├── chat.py              # Chat views (send, switch, retry, edit, drafts)
│   │   ├── memory.py            # Memory & context file views
│   │   ├── personas.py          # Persona management views
│   │   ├── settings.py          # Settings views
│   │   └── api.py               # JSON API endpoints (themes, models)
│   │
│   ├── templatetags/            # Custom Django template tags
│   │   ├── __init__.py
│   │   └── markdown_extras.py   # |markdown and |display_name filters
│   │
│   ├── services/                # Business logic layer (all file I/O lives here)
│   │   ├── __init__.py          # Exports all services + context file wrappers
│   │   ├── session_manager.py   # Session file CRUD (load, create, delete, pin, rename, draft, retry/edit)
│   │   ├── persona_manager.py   # Persona CRUD with side effects (rename/delete cascade)
│   │   ├── context_files.py     # ContextFileManager class (unified uploaded + local context files)
│   │   ├── chat_core.py         # LLM API & message handling
│   │   ├── config_manager.py    # Configuration management (fetch models, validate keys)
│   │   ├── context_manager.py   # System prompt assembly (persona + context + memory)
│   │   ├── llm_client.py        # Shared OpenRouter API utility (call_llm, LLMError)
│   │   ├── local_context.py     # Local directory context file management (shared)
│   │   ├── memory_manager.py    # Per-persona memory file I/O and update logic
│   │   ├── memory_worker.py     # Background threading, per-persona locks, auto-update scheduler
│   │   └── summarizer.py        # Title generation
│   │
│   ├── static/                  # Static assets
│   │   ├── css/
│   │   │   ├── input.css        # Tailwind source & theme config
│   │   │   └── output.css       # Compiled CSS (minified)
│   │   ├── js/
│   │   │   ├── utils.js         # Shared utilities, HTMX CSRF config, URL helpers
│   │   │   └── components.js    # All Alpine.js component definitions
│   │   └── themes/              # Color theme JSON files (16 themes)
│   │
│   └── templates/               # Django templates (no inline JS or CSS)
│       ├── base.html            # Base template with HTMX/Alpine, #app-urls config
│       ├── icons/               # SVG icon components (23 icons)
│       ├── components/
│       │   └── select_dropdown.html # Reusable searchable dropdown component
│       ├── chat/
│       │   ├── chat.html            # Main chat page (full)
│       │   ├── chat_home.html       # New chat home page
│       │   ├── chat_main.html       # Chat content partial
│       │   ├── context_files_modal.html # Context files modal partial
│       │   ├── dir_browser_modal.html  # Directory browser modal partial
│       │   ├── local_dir_tab.html      # Local directory tab partial
│       │   ├── sidebar_sessions.html # Sidebar session list
│       │   ├── new_chat_button.html # Reusable new chat button
│       │   ├── assistant_fragment.html
│       │   └── message_fragment.html
│       ├── memory/
│       │   └── memory_main.html # Memory content partial
│       ├── persona/
│       │   └── persona_main.html # Persona settings partial
│       ├── settings/
│       │   └── settings_main.html # Settings content partial
│       └── setup/
│           ├── step1.html       # API key setup
│           └── step2.html       # Model selection
│
└── data/                        # User data (gitignored)
    ├── sessions/                # Chat session JSON files
    │   └── session_*.json
    ├── personas/                # Persona definitions
    │   └── assistant/
    │       ├── identity.md      # Persona system prompt
    │       └── config.json      # Model override + memory settings
    ├── user_context/            # User-uploaded context files
    │   ├── config.json          # Global context file settings
    │   ├── *.md, *.txt          # Global context files
    │   └── personas/            # Persona-specific context files
    │       └── [persona_name]/
    │           ├── config.json  # Persona context file settings
    │           └── *.md, *.txt  # Persona-specific files
    └── memory/                  # Per-persona memory files
        └── {persona_name}.md    # Memory written from persona's perspective
```

---

## Key Components

### 1. Django Views (`chat/views/`)

**Purpose:** Handle HTTP requests and coordinate between templates and services. Views are thin — they contain no direct file I/O, no business logic, and no bare `except` clauses.

**Conventions:**
- All POST-only views use `@require_POST` decorator (Django's built-in)
- `setup_wizard` uses `@require_http_methods(["GET", "POST"])`
- Views delegate all file operations to service modules
- HTMX requests return partials; normal requests return full pages or redirects

**Modules:**

| Module | Views |
|--------|-------|
| `core.py` | `index`, `setup_wizard` |
| `chat.py` | `chat`, `switch_session`, `new_chat`, `start_chat`, `delete_chat`, `toggle_pin_chat`, `rename_chat`, `save_draft`, `send_message`, `retry_message`, `edit_message` |
| `memory.py` | `memory`, `update_memory`, `memory_update_status`, `save_memory_settings`, `wipe_memory`, `modify_memory`, `seed_memory`, context file CRUD views |
| `personas.py` | `persona_settings`, `save_persona_file`, `create_persona`, `delete_persona`, `save_persona_model` |
| `settings.py` | `settings`, `save_settings`, `save_context_history_limit`, `validate_provider_api_key`, `save_provider_model` |
| `api.py` | `get_available_themes`, `get_available_models`, `save_theme` |

**Shared view helpers in `chat.py`:**
- `get_model_for_persona()` - Resolve persona-specific or default model
- `_build_chat_core()` - Create ChatCore instance for a session
- `_build_persona_model_map()` - Build `{persona: model}` mapping
- `_get_user_timezone()` - Extract and persist user timezone
- `_resolve_session_persona()` - Get persona from session data with fallback
- `_handle_title_generation()` - 3-tier title generation logic

### 2. SessionManager (`chat/services/session_manager.py`)

**Purpose:** All session file I/O. Views never read or write session JSON directly.

**Key Functions:**
- `load_session(session_id)` / `create_session(session_id, persona, ...)` / `delete_session(session_id)`
- `toggle_pin(session_id)` / `rename_session(session_id, new_title)`
- `save_draft(session_id, draft_text)` / `clear_draft(session_id)`
- `remove_last_assistant_message(session_id)` - For retry
- `update_last_user_message(session_id, new_content)` - For edit
- `update_persona_across_sessions(old_name, new_name)` - Bulk update on persona rename
- `generate_session_id()` / `make_user_timestamp(user_timezone)`
- All writes use `flush() + fsync()` for durability
- All reads use specific exception handling (`json.JSONDecodeError`, `IOError`)

### 3. PersonaManager (`chat/services/persona_manager.py`)

**Purpose:** Persona CRUD with side-effect management. Views never touch persona directories directly.

**Key Functions:**
- `get_persona_preview(persona_name)` - Read identity .md content
- `save_persona_identity(persona_name, content)` - Write identity content
- `create_persona(name, identity_content)` - Create directory + identity.md
- `delete_persona(persona_name)` - Delete directory, memory file, and context files directory
- `rename_persona(old_name, new_name, config, save_config_fn)` - Orchestrates all 5 side effects:
  1. Rename persona directory
  2. Rename memory file
  3. Rename persona context files directory
  4. Update all session files via SessionManager
  5. Update default persona in config if needed
- `persona_exists(persona_name)` - Check existence

### 4. ContextFileManager (`chat/services/context_files.py`)

**Purpose:** Unified context file management for both global and per-persona scopes. Replaces the former `user_context.py` and `persona_context.py` (deleted).

**Class:** `ContextFileManager(base_dir, scope_label, header_description)`

**Methods:** `get_config`, `save_config`, `list_files`, `upload_file`, `delete_file`, `toggle_file`, `get_file_content`, `save_file_content`, `load_enabled_context`, plus local directory wrappers.

**Instantiation:** A global singleton instance is created in `services/__init__.py`. Per-persona instances are created via a factory function `_persona_ctx(persona_name)`. All exported function names are backward-compatible with the old module API.

### 5. ChatCore (`chat/services/chat_core.py`)

**Purpose:** Handles all LLM API interactions and message history management.

**Key Methods:**
- `__init__(...)` - Initialize with API key, model, system prompt, etc.
- `send_message(user_input)` - Sends message with retry logic, returns response
- `clear_history()` - Wipes conversation history
- `_get_payload_messages()` - Assembles messages for API
- `_save_history()` - Persists session to JSON
- `_load_history()` - Loads session from JSON

**Features:**
- **Retry Logic:** Up to 2 attempts for empty responses with 2-second delay
- **Sliding Window:** Configurable context history limit (default 50 message pairs)
- **Timezone Context:** Injects current date/time into system prompt using user's timezone
- **Message Timestamps:** Records ISO 8601 timestamps on all messages (stored in JSON, stripped from API payload)
- **Error Handling:** Returns "ERROR: ..." string on failures

### 6. Context Manager (`chat/services/context_manager.py`)

**Purpose:** Assembles the complete system prompt from persona identity, context files, and memory.

**Key Functions:**
- `load_context(persona_dir, persona_name=None)` - Loads and concatenates all context
- `get_available_personas(personas_dir)` - Returns list of valid personas
- `get_persona_config` / `save_persona_config` / `get_persona_model` / `get_persona_identity`

**Assembly Order:**
1. All `.md` files from persona directory (alphabetically)
2. Persona-specific context files (uploaded + local directory)
3. Global user context files (uploaded + local directory)
4. Per-persona memory file with "YOUR MEMORY ABOUT THIS USER" preamble

### 7. MemoryManager / MemoryWorker

**MemoryManager (`memory_manager.py`):** Per-persona memory file I/O and LLM-driven updates.
- File I/O: `get_memory_content`, `save_memory_content`, `delete_memory`, `rename_memory`
- LLM: `update_persona_memory()` (incremental merge), `modify_memory_with_command()` (natural-language edits)

**MemoryWorker (`memory_worker.py`):** Background threading for non-blocking memory updates.
- Per-persona threading locks prevent concurrent updates
- Status tracking per persona (idle/updating/complete/error)
- Auto-update scheduler daemon thread

### 8. LLM Client (`chat/services/llm_client.py`)

- `OPENROUTER_API_URL` - Single constant for the API endpoint
- `call_llm(api_key, model, messages, ...)` - Makes API call with configurable timeout and max_tokens
- `LLMError` - Custom exception for API failures

### 9. Shared Utilities (`chat/utils.py`)

- `load_config()` / `save_config()` - Config file access
- `get_sessions_with_titles()` / `group_sessions_by_persona()` - Session listing
- `get_formatted_model_list(api_key)` - Fetch + group + flatten models in one call
- `get_theme_list()` - Scan themes directory for available themes
- `group_models_by_provider()` / `flatten_models_with_provider_prefix()` - Model formatting helpers
- `title_has_artifacts()` - Detect malformed titles needing regeneration

### 10. Templates

**Base Template (`base.html`):**
- Loads HTMX and Alpine.js from CDN
- Loads `utils.js` (first, for theme initialization) and `components.js` before Alpine
- Provides `<meta name="csrf-token">` for HTMX CSRF (configured in `utils.js`)
- Provides `<div id="app-urls">` with Django `{% url %}` tags for JS URL resolution
- Uses semantic Tailwind classes (bg-surface, text-foreground, etc.)

**Partials (`*_main.html`):**
- Content fragments returned by HTMX requests, swapped into `#main-content`
- Pass data to Alpine components and modals via `data-*` attributes on hidden `<div>` elements
- No inline `<script>` with business logic, no `window.*` globals, no inline `style` attributes

### 11. JavaScript Architecture (`chat/static/js/`)

**`utils.js` - Shared Utility Functions:**

*Initialization (runs immediately):*
- Theme initialization from localStorage/data attributes (prevents flash)
- HTMX CSRF token configuration via `htmx:configRequest` event

*URL Configuration:*
- `getAppUrl(key, fallback)` - Read URLs from `#app-urls` data attributes (set by Django templates)

*Theme System:*
- `getAvailableThemes()` / `getColorTheme()` / `loadTheme()` / `setTheme()` / `getTheme()`
- `saveThemePreference()` / `applyThemeColors()` / `cacheThemeColors()`

*Core Utilities:*
- `getCsrfToken()` - Centralized CSRF token retrieval (all JS must use this, never querySelector)
- `handleTextareaKeydown()` / `autoResizeTextarea()` - Textarea helpers
- `scrollToBottom()` / `updateScrollButtonVisibility()` / `setupScrollButtonListener()` - Scroll management
- `toDisplayName()` / `toFolderName()` - Persona name conversion
- `setTimezoneInput()` - Sets user timezone for server-side time context

*Message UI:*
- `addUserMessage()` / `removeThinkingIndicator()` - Message UI helpers
- `animateAssistantResponse()` / `typewriterReveal()` - Response animation
- `convertTimestamps()` / `insertDateSeparators()` / `formatDateSeparator()` - Date formatting
- `copyMessageToClipboard()` / `retryLastMessage()` / `editLastMessage()` / `saveEditedMessage()` / `cancelEdit()`
- `cleanupMessageButtons()` / `handleMessageError()`

*Draft Management:*
- `saveDraftDebounced()` / `saveDraftNow()` / `clearDraft()` / `restoreDraft()`
- `saveNewChatDraftDebounced()` / `restoreNewChatDraft()` / `clearNewChatDraft()`

*Memory:*
- `showMemoryUpdating()` / `showMemoryModifying()` - Memory status indicators
- `initMemoryView()` - Format timestamps and start polling after partial load
- `pollMemoryUpdateStatus()` - Polls `/memory/update-status/` for progress

**`components.js` - Alpine.js Component Definitions:**

Components are registered via `Alpine.data()` in the `alpine:init` event.

| Component | Purpose |
|-----------|---------|
| `selectDropdown` | Reusable searchable dropdown with keyboard nav and filtering |
| `collapsibleSection` | Simple toggle for sidebar groups |
| `sidebarState` | Responsive sidebar with localStorage persistence |
| `deleteModal` | Chat deletion confirmation modal |
| `renameModal` | Chat rename form modal |
| `wipeMemoryModal` | Memory wipe confirmation modal |
| `editPersonaModal` | Persona create/edit modal |
| `deletePersonaModal` | Persona deletion confirmation modal |
| `editPersonaModelModal` | Persona model override modal with lazy loading |
| `contextFilesModal` | Context file upload/management (used for both global and persona-scoped) |
| `memoryPersonaPicker` | Persona selector dropdown on Memory view |
| `providerModelSettings` | Provider and model configuration (settings page) |
| `homePersonaPicker` | Persona picker on home page |
| `personaSettingsPicker` | Persona picker on persona settings page |
| `providerPicker` | Provider selector (setup step 1) |
| `modelPicker` | Model selector (setup step 2) |
| `themePicker` | Color theme dropdown (settings page) |
| `setupThemePicker` | Theme picker for setup wizard step 2 |
| `themeModeToggle` | Dark/light mode toggle buttons (settings page) |
| `memorySettings` | Memory generation settings form with inline save |
| `contextHistoryLimit` | Context history limit setting with inline save |

**Cross-Component Communication Pattern:**
Modal components listen for dispatched events on `window` instead of using global function helpers:

```javascript
// In component init():
window.addEventListener('open-delete-modal', (e) => {
    this.open(e.detail.id, e.detail.title);
});

// From template (Alpine directive):
@click="window.dispatchEvent(new CustomEvent('open-delete-modal', { detail: { id: '...', title: '...' } }))"
```

**Data Attribute Pattern:**
Components read configuration from `data-*` attributes in `init()` and store values as instance properties. Never read `this.$el.dataset` in methods called from event handlers — Alpine's `this.$el` may not point to the component root in that context.

```javascript
init() {
    this._saveUrl = this.$el.dataset.saveUrl;  // Store in init
},
async save() {
    await fetch(this._saveUrl, ...);  // Use stored value
}
```

---

## Features

### Collapsible Persona-Grouped Sidebar

Sessions are organized by persona with collapsible sections:

```
★ Pinned (2)
  Important Chat            ☆ 🗑
  Another Pinned            ☆ 🗑

▼ Assistant (3)
  Session Title 1           ☆ 🗑
  Session Title 2           ☆ 🗑

▶ Custom (2)  [collapsed]
```

- Click persona header to toggle collapse/expand
- Chevron icons indicate expanded/collapsed state
- Count badge shows number of sessions per group
- Current session highlighted with accent color
- Pin/unpin and delete buttons on each session
- All icons are SVG-based for theme compatibility

### Pinned Chats

- Star icon to pin/unpin chats
- Pinned chats appear in a separate "Pinned" section at top
- Pinned status persists across sessions

### Sidebar Footer

Icon buttons at bottom of sidebar for quick access:
- **New Chat** (circle-plus icon) - Start a new conversation
- **Memory** (brain-cog icon) - View/manage per-persona memory
- **Personas** (user-pen icon) - Manage personas and model overrides
- **Settings** (gear icon) - Configure preferences
- **Theme Toggle** (sun/moon icon) - Switch dark/light mode

### HTMX-Powered Reactivity

- **Session Switching:** Click session → HTMX swaps main content
- **Send Message:** Form submit → HTMX appends response
- **Memory/Settings:** Load in main pane without navigation
- **Modals:** Alpine.js handles show/hide state

### Per-Session Personas

- **Selection:** Choose persona when creating new chat
- **Persistence:** Persona saved in session JSON
- **Isolation:** Each session maintains its own persona
- **Default:** Configurable default persona for new chats
- **Model Override:** Each persona can have its own model
- **Context Files:** Each persona can have dedicated context files
- **Protected:** The default "assistant" persona cannot be deleted

### Persona Context Files

Upload context files that apply only to a specific persona, enabling separation of concerns:

- **Persona-Scoped:** Files only included in chats using that persona
- **Drag & Drop:** Easy upload via modal on Persona Settings page
- **Toggle Enable/Disable:** Control which files are active
- **Inline Editing:** Edit file content directly in the modal
- **Badge Count:** Shows number of context files per persona
- **Stored Separately:** Files saved in `data/user_context/personas/[name]/`

### Message Actions

Messages support contextual action buttons on the latest exchange:

- **Copy:** Copy assistant message content to clipboard
- **Retry:** Remove last assistant response and re-send the user message
- **Edit:** Inline-edit the last user message and re-submit
- Action buttons are automatically cleaned up from older messages

### Draft Persistence

Message drafts are auto-saved with debounced persistence:

- **Per-Session Drafts:** Draft text saved to server via AJAX on typing
- **Home Page Drafts:** New chat drafts saved to localStorage
- **Auto-Restore:** Drafts restored when switching back to a session
- **Auto-Clear:** Drafts cleared on successful message send

### Per-Persona Living Memory

Each persona maintains its own memory about the user, stored in `data/memory/{persona}.md`:

- **Per-Persona:** Each persona has independent memory written in second-person from the persona's perspective
- **Incremental Merge:** New information is merged into existing memory (grows naturally, not full rewrites)
- **Persona Selector:** Dropdown in Memory view to switch between personas' memories
- **Background Updates:** "Update Memory" button spawns a background thread; JS polls `/memory/update-status/` for progress
- **Auto-Update Scheduler:** Daemon thread checks for new conversations per persona on a configurable interval (0 = disabled)
- **Per-Persona Settings:** Memory settings stored in `data/personas/{name}/config.json`:
  - `user_history_max_threads` (default 10, 0 = unlimited)
  - `user_history_messages_per_thread` (default 100, 0 = unlimited)
  - `memory_size_limit`
  - `auto_memory_interval` (0 = disabled)
- **Wipe Memory:** Per-persona with confirmation
- **Modify Memory:** Natural-language commands to edit memory content
- Context files can be uploaded to augment memory
- Local directory references for live file inclusion

### Local Directory Context Files

Reference `.md` and `.txt` files from local directories without copying them into the app:

- **Live Reading:** Files are read from their original location at prompt-assembly time
- **Tabbed Modal:** "Uploaded Files" and "Local Directory" tabs in both global and persona context modals
- **Directory Browser:** Visual directory browser for selecting folders
- **Toggle Enable/Disable:** Control which files are active per directory
- **Read-Only Viewing:** View local files in the modal without editing
- **Unified Endpoints:** Single set of `/context/local/` API routes serve both global and persona-scoped requests
- **Security:** Path resolution with `os.path.realpath()`, DATA_DIR blocking, config registration verification
- **Config Storage:** Directory paths and file enable states stored in `config.json` under `local_directories` key

---

## How to Run

### Quick Start (Users)

```bash
python run.py
```

The launcher automatically creates a virtual environment and installs dependencies on first run. Access at `http://localhost:8420`

### Developer Setup

For development with Tailwind CSS hot-reloading:

```bash
# Create virtual environment
python3 -m venv .venv
source .venv/bin/activate

# Install Python dependencies
pip install -r requirements.txt

# Install Node dependencies (for Tailwind CSS)
npm install

# Run with Tailwind watcher
npm run dev
```

This runs both the Tailwind CSS watcher and Django server concurrently.

### First-Time Setup

1. Navigate to `http://localhost:8420`
2. Enter your OpenRouter API key
3. Select a model from the list
4. Start chatting!

---

## Configuration

### config.json

```json
{
    "OPENROUTER_API_KEY": "sk-or-v1-...",
    "MODEL": "anthropic/claude-haiku-4.5",
    "SITE_URL": "https://liminalsalt.app",
    "SITE_NAME": "Liminal Salt",
    "DEFAULT_PERSONA": "assistant",
    "PERSONAS_DIR": "personas",
    "CONTEXT_HISTORY_LIMIT": 50,
    "THEME": "liminal-salt",
    "THEME_MODE": "dark"
}
```

**Key Settings:**
- `OPENROUTER_API_KEY`: Your API key from OpenRouter
- `MODEL`: Default LLM model to use
- `DEFAULT_PERSONA`: Default persona for new chats
- `PERSONAS_DIR`: Directory containing persona definitions
- `CONTEXT_HISTORY_LIMIT`: Number of message pairs sent to LLM as context per chat (default 50)
- `THEME`: Color theme identifier (one of 16 themes: liminal-salt [default], nord, dracula, gruvbox, monokai, solarized, rose-pine, tokyo-night, one-dark, night-owl, catppuccin, ayu, everforest, kanagawa, palenight, synthwave)
- `THEME_MODE`: Light or dark mode preference

**Note:** Memory settings (`user_history_max_threads`, `user_history_messages_per_thread`, `memory_size_limit`, `auto_memory_interval`) are now per-persona, stored in `data/personas/{name}/config.json`.

### Django Settings (`liminal_salt/settings.py`)

Key customizations:
- `DATABASES = {}` - No database required
- `SESSION_ENGINE = 'django.contrib.sessions.backends.signed_cookies'`
- `DATA_DIR`, `SESSIONS_DIR`, `PERSONAS_DIR`, `MEMORY_DIR` - Data paths

---

## Development Notes

### Code Standards

These standards are enforced — all new code must follow them.

#### Python / Django

**Separation of Concerns:**
- Views are thin coordinators. All file I/O belongs in `services/`.
- Never use `open()`, `json.load()`, `json.dump()`, `os.path.exists()`, `os.remove()`, `shutil.*` in view files.
- Session file operations → `SessionManager`
- Persona file operations → `PersonaManager`
- Context file operations → `ContextFileManager`
- Use `load_config()` and `save_config()` for all config.json access.

**Django Best Practices:**
- All POST-only views must use `@require_POST` decorator from `django.views.decorators.http`.
- Views that handle both GET and POST use `@require_http_methods(["GET", "POST"])`.
- Never use bare `except:` — always catch specific exceptions (`json.JSONDecodeError`, `IOError`, `ValueError`, etc.).
- Use `get_formatted_model_list(api_key)` for the fetch → group → flatten chain. Never repeat the 3-step pattern.
- Use `get_theme_list()` from `utils.py` for theme enumeration. Never read theme JSON files in views.

**Service Layer Rules:**
- All file writes use `flush() + fsync()` for durability.
- Services raise typed exceptions; views handle them.
- `PersonaManager.rename_persona()` handles all 5 side effects (directory, memory, context dir, sessions, config).
- `PersonaManager.delete_persona()` handles all cleanup (directory, memory, context dir).

#### JavaScript

**No Inline JavaScript in Templates:**
- All JS must be in `utils.js` or `components.js`.
- Alpine.js components must be defined in `components.js` and registered via `Alpine.data()`.
- Exception: Minimal one-line initialization calls like `<script>initMemoryView();</script>` are acceptable for HTMX-swapped partials.

**No `window.*` Globals for Component Communication:**
- Use `window.dispatchEvent(new CustomEvent('event-name', { detail: {...} }))` from templates.
- Modal components listen via `window.addEventListener('event-name', ...)` in `init()`.
- Never store component references on `window`.

**No Hardcoded URLs in JavaScript:**
- Use `getAppUrl(key, fallback)` to read URLs from `#app-urls` data attributes (set in `base.html` using Django `{% url %}` tags).
- Components read URLs from their own `data-*` attributes, set by the template that instantiates them.

**Alpine Component `init()` Rules:**
- Read all `data-*` attributes in `init()` and store as instance properties (e.g., `this._saveUrl = this.$el.dataset.saveUrl`).
- Never read `this.$el.dataset` in methods called from `@click` or other event handlers — `this.$el` may not point to the component root in that context.

**Async and Error Handling:**
- All async operations must use `async/await` with `try/catch`. No `.then()` chains.
- Always use `getCsrfToken()` for CSRF tokens. Never use `document.querySelector('[name=csrfmiddlewaretoken]')`.

#### HTML / Templates

**No Inline Event Handlers:**
- Never use `onclick`, `onchange`, `oninput`, `onkeydown`, etc.
- Use Alpine directives: `@click`, `@change`, `@input`, `@keydown`.
- For function calls that need `this`/element reference, use `$el` (e.g., `@click="copyMessageToClipboard($el)"`).
- For function calls that need the event, use `$event` (e.g., `@keydown="handleTextareaKeydown($event)"`).

**No Inline CSS:**
- All styles via Tailwind classes or `input.css`.
- Never use `style` attributes. Use `hidden` class or `x-show` for visibility.

**Data Passing:**
- Pass data to Alpine components via `data-*` attributes, not inline JS or `window.*` globals.
- For lists/objects, serialize as JSON in `data-*` attributes (e.g., `data-files='{{ files_json|safe }}'`).
- Use hidden `<div>` elements with `id` and `data-*` attributes as data sources for modals that need to read from HTMX-swapped partials.

### SVG Icon System

Icons are stored as reusable Django template includes in `chat/templates/icons/`. They use `currentColor` to inherit text color, default to `w-5 h-5`, and are overridable via `class` parameter.

**Available icons (23):**
`alert`, `brain-cog`, `check`, `check-circle`, `chevron-down`, `chevron-right`,
`chevrons-left`, `circle-plus`, `copy`, `cpu`, `folder`, `menu`, `moon`, `pencil`,
`plus`, `retry`, `settings`, `star-filled`, `star-outline`, `sun`, `trash`, `user-pen`, `x`

### URL Routes

```
/                              → index (redirect to /chat/ or /setup/)
/setup/                        → setup_wizard
/chat/                         → chat (main view)
/chat/send/                    → send_message (HTMX)
/chat/switch/                  → switch_session (HTMX)
/chat/new/                     → new_chat
/chat/start/                   → start_chat (new chat from home)
/chat/delete/                  → delete_chat
/chat/pin/                     → toggle_pin_chat
/chat/rename/                  → rename_chat
/chat/save-draft/              → save_draft (AJAX)
/chat/retry/                   → retry_message (HTMX)
/chat/edit-message/            → edit_message (HTMX)
/memory/                       → memory
/memory/update/                → update_memory
/memory/update-status/         → memory_update_status (JSON polling endpoint)
/memory/wipe/                  → wipe_memory
/memory/modify/                → modify_memory
/memory/save-settings/         → save_memory_settings (AJAX)
/memory/context/upload/        → upload_context_file
/memory/context/delete/        → delete_context_file
/memory/context/toggle/        → toggle_context_file
/memory/context/content/       → get_context_file_content
/memory/context/save/          → save_context_file_content
/persona/                      → persona_settings
/persona/context/upload/       → upload_persona_context_file
/persona/context/delete/       → delete_persona_context_file
/persona/context/toggle/       → toggle_persona_context_file
/persona/context/content/      → get_persona_context_file_content
/persona/context/save/         → save_persona_context_file_content
/context/local/browse/         → browse_directories
/context/local/add/            → add_local_context_dir (accepts optional persona param)
/context/local/remove/         → remove_local_context_dir (accepts optional persona param)
/context/local/toggle/         → toggle_local_context_file (accepts optional persona param)
/context/local/content/        → get_local_context_file_content (accepts optional persona param)
/context/local/refresh/        → refresh_local_context_dir (accepts optional persona param)
/settings/                     → settings
/settings/save/                → save_settings
/settings/validate-api-key/    → validate_provider_api_key
/settings/save-provider-model/ → save_provider_model
/settings/save-context-history-limit/ → save_context_history_limit (AJAX)
/settings/available-models/    → get_available_models (AJAX)
/settings/save-persona/        → save_persona_file
/settings/create-persona/      → create_persona
/settings/delete-persona/      → delete_persona
/settings/save-persona-model/  → save_persona_model
/api/themes/                   → get_available_themes (JSON list of themes)
/api/save-theme/               → save_theme (POST theme preference)
```

### Testing Checklist

**Basic Operations:**
- [ ] Create new chat session with persona selection
- [ ] Send messages and receive responses
- [ ] Switch between sessions (HTMX)
- [ ] Delete session with confirmation
- [ ] Pin/unpin chat sessions
- [ ] Retry last assistant message
- [ ] Edit last user message
- [ ] Copy assistant message to clipboard
- [ ] Draft auto-saves and restores on session switch

**Theme System:**
- [ ] Select theme during setup wizard (step 2)
- [ ] Change color theme in Settings
- [ ] Toggle dark/light mode (sidebar and settings stay in sync)
- [ ] Theme persists after page refresh
- [ ] Theme persists in new browser (server-side storage)
- [ ] No flash of wrong theme on page load

**Memory & Settings:**
- [ ] View per-persona memory in main pane
- [ ] Switch between personas in memory view using persona selector
- [ ] Update memory (background, non-blocking), poll status updates
- [ ] Auto-update scheduler triggers on interval when enabled
- [ ] Per-persona memory settings save correctly in persona config.json
- [ ] Wipe memory per-persona with confirmation
- [ ] Modify memory with natural-language commands
- [ ] Upload global context files in Settings view
- [ ] Change default persona in Persona Settings
- [ ] Set model override for persona
- [ ] Create new persona
- [ ] Edit persona content
- [ ] Rename persona (verify all side effects: sessions, memory, context dir, config)
- [ ] Delete persona (verify cleanup: directory, memory, context dir)
- [ ] Verify "assistant" persona cannot be deleted
- [ ] Context history limit saves correctly

**Context Files:**
- [ ] Upload context file (global and persona-scoped)
- [ ] Toggle file enabled/disabled status
- [ ] Edit file content inline
- [ ] Delete context file
- [ ] Badge count updates correctly
- [ ] Context appears in LLM prompt for correct scope
- [ ] Modal re-open shows updated file list after upload

**Local Directory Context Files:**
- [ ] Add local directory via path input or browser
- [ ] Toggle local files on/off
- [ ] View local file content (read-only)
- [ ] Remove directory from context
- [ ] Refresh directory picks up new files
- [ ] Badge counts enabled uploaded + enabled local files
- [ ] Local files appear in LLM system prompt
- [ ] Persona-scoped local directories work independently
- [ ] Directory browser navigates filesystem correctly
- [ ] Nonexistent path shows error, path inside data/ is rejected

**Edge Cases:**
- [ ] First launch (no config.json)
- [ ] Empty sessions directory
- [ ] Invalid API key
- [ ] Icons render correctly in both themes
- [ ] Lazy model loading works in Edit Model modal

---

## Quick Reference

### Important Files

| File | Purpose |
|------|---------|
| `chat/views/` | View functions (thin, no file I/O) |
| `chat/services/session_manager.py` | Session file CRUD |
| `chat/services/persona_manager.py` | Persona CRUD with side effects |
| `chat/services/context_files.py` | Unified context file management |
| `chat/services/chat_core.py` | LLM API calls |
| `chat/services/llm_client.py` | Shared OpenRouter API utility |
| `chat/services/context_manager.py` | System prompt assembly |
| `chat/services/memory_manager.py` | Per-persona memory file I/O and update logic |
| `chat/services/memory_worker.py` | Background threading and auto-update scheduler |
| `chat/services/local_context.py` | Shared local directory context management |
| `chat/utils.py` | Config, model formatting, theme list |
| `chat/templates/chat/chat.html` | Main UI template |
| `chat/templates/base.html` | Base template with URL config |
| `chat/static/js/components.js` | Alpine.js component definitions |
| `chat/static/js/utils.js` | Shared utility functions + HTMX CSRF config |
| `chat/static/css/input.css` | Tailwind source & CSS variables |
| `chat/static/themes/*.json` | Color theme definitions |
| `liminal_salt/settings.py` | Django config |
| `config.json` | App configuration |

### Useful Commands

```bash
# Development (Tailwind watcher + Django server)
npm run dev

# Build CSS only
npm run build:css

# Django server only (after CSS is built)
python3 manage.py runserver

# Check Django configuration
python3 manage.py check

# Version management
npm run version:patch   # 0.1.3 → 0.1.4
npm run version:minor   # 0.1.3 → 0.2.0
npm run version:major   # 0.1.3 → 1.0.0

# Reset all data
rm -rf data/sessions/*.json data/memory/*.md
```

### API Endpoint

```
https://openrouter.ai/api/v1/chat/completions
```

---

## Remaining Backlog

Items not addressed by the Milestone 1 SoC refactor.

### Code Quality

**Standardize error responses**
Error responses are inconsistent: some return `HttpResponse(status=405)` with no body, some return `JsonResponse({'error': ...})`, some return plain text. Create helper functions like `json_error(message, status)`.

**Add logging to important operations**
Most views don't log operations. Only `core.py` and `personas.py` use logging. Add logging for session creation/deletion, persona changes, API failures.

### JavaScript

**Clean up `selectDropdown` MutationObserver**
`utils.js`: The MutationObserver is never cleaned up (memory leak) and re-parses JSON on every attribute change. Use Alpine's native reactivity instead.

**Break down `providerModelSettings` component**
At ~190 lines, this component handles API validation, model fetching, provider selection, model selection, and form submission. Consider splitting into smaller focused components.

**Extract magic numbers to constants**
Timeout delays scattered throughout: `3000ms`, `1000ms`, `25ms` animation speed, retry delay of `2s`. Define as named constants.

### Templates

**Extract modals from `chat.html`**
`chat.html` has 8 modal definitions inline. Consider extracting to partial files (e.g., `templates/modals/`) and `{% include %}` them.

**Consolidate message rendering**
Message rendering differs slightly between `chat_main.html`, `assistant_fragment.html`, and `message_fragment.html`. Clarify which partial is used where, and consolidate where possible.

### Accessibility

**Add ARIA attributes to modals**
All modals lack `role="dialog"`, `aria-modal="true"`, and `aria-labelledby`. Add focus trapping on modal open.

**Add `<label>` elements to form inputs**
Textarea in `chat_main.html` and input in `chat_home.html` lack associated `<label>` elements.

**Add `role="alert"` to error messages**
`handleMessageError()` creates error DOM without `role="alert"` or `aria-live`.

**Use semantic list elements**
Sidebar session groups in `sidebar_sessions.html` use `<div>` instead of `<ul>`/`<li>`.

### Service Hardening

**Add timeout-specific error handling**
`ChatCore` and `Summarizer` set timeouts but don't distinguish `requests.exceptions.Timeout` from other errors. `ConfigManager` already does this correctly — follow the same pattern.

**Add path traversal validation**
Persona context file operations should validate that `persona_name` doesn't contain `../` or other traversal characters, even though `os.path.basename()` is used on filenames.

---

## Resources

- **OpenRouter API:** https://openrouter.ai/docs
- **Django Docs:** https://docs.djangoproject.com/
- **Tailwind CSS:** https://tailwindcss.com/docs
- **HTMX Docs:** https://htmx.org/docs/
- **Alpine.js Docs:** https://alpinejs.dev/

---

**End of CLAUDE.md**
