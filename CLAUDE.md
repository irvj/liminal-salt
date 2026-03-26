# CLAUDE.md - Project Overview & Developer Guide

**Last Updated:** March 25, 2026
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
│                   Django Views                          │
│  - chat() - Main chat view                              │
│  - send_message() - HTMX message endpoint               │
│  - switch_session() - Session switching                 │
│  - memory() / update_memory() - Memory management       │
│  - settings() / save_settings() - Settings management   │
└────────────────────┬────────────────────────────────────┘
                     │
                     ▼
┌─────────────────────────────────────────────────────────┐
│                 Business Logic (services/)              │
│  - ChatCore: API calls, message history                 │
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
│   ├── utils.py                 # Helper functions
│   │
│   ├── views/                   # View functions (split by domain)
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
│   ├── services/                # Business logic layer
│   │   ├── __init__.py          # Exports all services
│   │   ├── chat_core.py         # LLM API & message handling
│   │   ├── config_manager.py    # Configuration management
│   │   ├── context_manager.py   # System prompt assembly
│   │   ├── llm_client.py        # Shared OpenRouter API utility (call_llm, LLMError, OPENROUTER_API_URL)
│   │   ├── local_context.py     # Local directory context file management (shared)
│   │   ├── memory_manager.py    # Per-persona memory file I/O and update logic
│   │   ├── memory_worker.py     # Background threading, per-persona locks, auto-update scheduler
│   │   ├── persona_context.py   # Persona-specific context file management
│   │   ├── user_context.py      # Global user context file management
│   │   └── summarizer.py        # Title generation
│   │
│   ├── static/                  # Static assets
│   │   ├── css/
│   │   │   ├── input.css        # Tailwind source & theme config
│   │   │   └── output.css       # Compiled CSS (minified)
│   │   ├── js/
│   │   │   ├── utils.js         # Shared utility functions
│   │   │   └── components.js    # Alpine.js component definitions
│   │   └── themes/              # Color theme JSON files (16 themes)
│   │       ├── liminal-salt.json # Liminal Salt (default)
│   │       ├── nord.json        # Nord
│   │       ├── dracula.json     # Dracula
│   │       ├── gruvbox.json     # Gruvbox
│   │       ├── monokai.json     # Monokai
│   │       ├── solarized.json   # Solarized
│   │       ├── rose-pine.json   # Rose Pine
│   │       ├── tokyo-night.json # Tokyo Night
│   │       ├── one-dark.json    # One Dark
│   │       ├── night-owl.json   # Night Owl
│   │       ├── catppuccin.json  # Catppuccin
│   │       ├── ayu.json         # Ayu
│   │       ├── everforest.json  # Everforest
│   │       ├── kanagawa.json    # Kanagawa
│   │       ├── palenight.json   # Palenight
│   │       └── synthwave.json   # Synthwave '84
│   │
│   └── templates/               # Django templates
│       ├── base.html            # Base template with HTMX/Alpine
│       ├── icons/               # SVG icon components (23 icons)
│       │   ├── alert.html, brain-cog.html, check.html, check-circle.html
│       │   ├── chevron-down.html, chevron-right.html, chevrons-left.html
│       │   ├── circle-plus.html, copy.html, cpu.html, folder.html
│       │   ├── menu.html, moon.html, pencil.html, plus.html
│       │   ├── retry.html, settings.html, star-filled.html
│       │   ├── star-outline.html, sun.html, trash.html, user-pen.html, x.html
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

**Purpose:** Handle HTTP requests and coordinate between templates and services. Views are split into domain-specific modules.

**Modules:**

| Module | Views |
|--------|-------|
| `core.py` | `index`, `setup_wizard` |
| `chat.py` | `chat`, `switch_session`, `new_chat`, `start_chat`, `delete_chat`, `toggle_pin_chat`, `rename_chat`, `save_draft`, `send_message`, `retry_message`, `edit_message` |
| `memory.py` | `memory`, `update_memory`, `memory_update_status`, `save_memory_settings`, `wipe_memory`, `modify_memory`, context file CRUD views |
| `personas.py` | `persona_settings`, `save_persona_file`, `create_persona`, `delete_persona`, `save_persona_model` |
| `settings.py` | `settings`, `save_settings`, `save_context_history_limit`, `validate_provider_api_key`, `save_provider_model` |
| `api.py` | `get_available_themes`, `get_available_models`, `save_theme` |

**Notable new views:**
- `save_draft()` - Auto-save message drafts per session
- `retry_message()` - Retry last assistant response
- `edit_message()` - Edit last user message and re-send

**HTMX Pattern:**
Views check `request.headers.get('HX-Request')` to return either:
- Full HTML page (normal request)
- HTML partial fragment (HTMX request for swap)

### 2. ChatCore (`chat/services/chat_core.py`)

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
- **Token Cleanup:** Removes `<s>` and `</s>` artifacts
- **Sliding Window:** Configurable context history limit (default 50 message pairs)
- **Timezone Context:** Injects current date/time into system prompt using user's timezone
- **Message Timestamps:** Records ISO 8601 timestamps on all messages (stored in JSON, stripped from API payload)
- **Error Handling:** Returns "ERROR: ..." string on failures

### 3. Context Manager (`chat/services/context_manager.py`)

**Purpose:** Assembles the complete system prompt from persona and memory.

**Key Functions:**
- `load_context(persona_dir, persona_name=None)` - Loads and concatenates context including per-persona memory
- `get_available_personas(personas_dir)` - Returns list of valid personas
- `get_persona_config(persona_name, personas_dir)` - Loads persona config.json
- `save_persona_config(persona_name, config_data, personas_dir)` - Saves persona config.json
- `get_persona_model(persona_name, personas_dir)` - Gets model override for persona
- `get_persona_identity(persona_dir)` - Returns raw persona .md content (all .md files concatenated, no headers)

**Assembly Order:**
1. All `.md` files from persona directory (alphabetically)
2. Persona-specific context files (uploaded + local directory, from `data/user_context/personas/[name]/`)
3. Global user context files (uploaded + local directory, from `data/user_context/`)
4. Per-persona memory file (`data/memory/{persona}.md`) with "YOUR MEMORY ABOUT THIS USER" preamble

### 4. Summarizer (`chat/services/summarizer.py`)

**Purpose:** Generates session titles (title-only; memory logic moved to MemoryManager).

**Key Methods:**
- `generate_title(first_user_msg, first_assistant_msg)` - Creates 2-4 word title

### 5. MemoryManager (`chat/services/memory_manager.py`)

**Purpose:** Per-persona memory file I/O and LLM-driven memory updates.

**File I/O Functions:**
- `get_memory_file(persona_name)` - Returns path to `data/memory/{persona}.md`
- `get_memory_content(persona_name)` / `save_memory_content(persona_name, content)` - Read/write memory
- `delete_memory(persona_name)` / `rename_memory(old_name, new_name)` - Lifecycle management
- `_safe_persona_name(name)` - Path traversal protection

**MemoryManager Class:**
- `update_persona_memory(persona_name, ...)` - Incremental merge of new conversation data into existing memory, written in second-person from the persona's perspective
- `modify_memory_with_command(persona_name, command)` - Apply natural-language edits to memory

### 6. MemoryWorker (`chat/services/memory_worker.py`)

**Purpose:** Background threading for non-blocking memory updates.

**Key Features:**
- Per-persona threading locks prevent concurrent updates to the same persona
- Status tracking per persona (idle/updating/complete/error)
- `start_manual_update(persona_name, ...)` - Spawns background thread for manual "Update Memory" button
- Auto-update scheduler: daemon thread started via `AppConfig.ready()`, checks for new conversations per persona on a configurable interval (0 = disabled)
- `get_update_status(persona_name)` - Returns current status for JS polling

### 7. LLM Client (`chat/services/llm_client.py`)

**Purpose:** Shared utility for all OpenRouter API calls, used by ChatCore, Summarizer, and MemoryManager.

- `OPENROUTER_API_URL` - Single constant for the API endpoint
- `call_llm(api_key, model, messages, ...)` - Makes API call with configurable timeout and max_tokens
- `LLMError` - Custom exception for API failures

### 8. Templates

**Base Template (`base.html`):**
- Loads HTMX and Alpine.js from CDN
- Loads `utils.js` and `components.js` before Alpine (for component registration)
- Loads compiled Tailwind CSS from `static/css/output.css`
- Configures CSRF token for HTMX requests
- Uses semantic Tailwind classes (bg-surface, text-foreground, etc.)

**Main Chat (`chat/chat.html`):**
- Full page with sidebar + main content area
- Uses registered Alpine.js components (modals, sidebar, dropdowns)
- HTMX attributes for reactive session switching
- Minimal inline JS - components defined in `components.js`

**Partials (`*_main.html`):**
- Content fragments returned by HTMX requests
- Swapped into `#main-content` div

### 9. JavaScript Architecture (`chat/static/js/`)

**Purpose:** Centralized Alpine.js components and utility functions, extracted from inline scripts for better maintainability and reusability.

**`utils.js` - Shared Utility Functions:**

*Theme System:*
- `getAvailableThemes()` / `getColorTheme()` / `initTheme()` - Theme loading and initialization
- `loadTheme()` / `setTheme()` / `getTheme()` - Color theme and mode management
- `saveThemePreference()` - Persists theme to backend config.json
- `applyThemeColors()` / `cacheThemeColors()` - CSS custom property management

*Core Utilities:*
- `getCsrfToken()` - Centralized CSRF token retrieval
- `handleTextareaKeydown()` / `autoResizeTextarea()` - Textarea helpers
- `scrollToBottom()` / `updateScrollButtonVisibility()` / `setupScrollButtonListener()` - Scroll management
- `toDisplayName()` / `toFolderName()` - Persona name conversion
- `setTimezoneInput()` - Sets user timezone for server-side time context
- `selectDropdown()` - Reusable searchable dropdown component with keyboard navigation

*Message UI:*
- `addUserMessage()` / `removeThinkingIndicator()` - Message UI helpers
- `animateAssistantResponse()` / `typewriterReveal()` - Response animation
- `convertTimestamps()` / `insertDateSeparators()` / `formatDateSeparator()` - Date formatting
- `copyMessageToClipboard()` - Copy assistant message content
- `retryLastMessage()` / `editLastMessage()` / `saveEditedMessage()` / `cancelEdit()` - Message editing and retry
- `cleanupMessageButtons()` - Remove action buttons from non-latest messages
- `handleMessageError()` - Error state handling

*Draft Management:*
- `saveDraftDebounced()` / `saveDraftNow()` / `clearDraft()` / `restoreDraft()` - Per-session draft persistence
- `saveNewChatDraftDebounced()` / `restoreNewChatDraft()` / `clearNewChatDraft()` - Home page draft persistence

*Sidebar:*
- `updateSidebarHighlight()` / `clearSidebarHighlight()` - Active session highlighting
- `updateHeaderTitle()` - Update chat header on session switch
- `showMemoryUpdating()` / `showMemoryModifying()` - Memory status indicators
- `pollMemoryUpdateStatus()` - Polls `/memory/update-status/` for background memory update progress

**`components.js` - Alpine.js Component Definitions:**

Components are registered via `Alpine.data()` in the `alpine:init` event, making them available across all templates.

| Component | Purpose |
|-----------|---------|
| `selectDropdown` | Reusable searchable dropdown with keyboard nav and filtering |
| `collapsibleSection` | Simple toggle for sidebar groups |
| `sidebarState` | Responsive sidebar with localStorage persistence |
| `deleteModal` | Chat deletion confirmation modal |
| `renameModal` | Chat rename form modal |
| `wipeMemoryModal` | Memory wipe confirmation modal |
| `editPersonaModal` | Persona content editor modal |
| `deletePersonaModal` | Persona deletion confirmation modal |
| `editPersonaModelModal` | Persona model override modal with lazy loading |
| `contextFilesModal` | Global context file upload/management modal |
| `personaContextFilesModal` | Persona-specific context file modal |
| `memoryPersonaPicker` | Persona selector dropdown on Memory view |
| `providerModelSettings` | Provider and model configuration (settings page) |
| `homePersonaPicker` | Persona picker on home page |
| `personaSettingsPicker` | Persona picker on persona settings page |
| `providerPicker` | Provider selector (setup step 1) |
| `modelPicker` | Model selector (setup step 2) |
| `themePicker` | Color theme dropdown (settings page) |
| `setupThemePicker` | Theme picker for setup wizard step 2 |
| `themeModeToggle` | Dark/light mode toggle buttons (settings page) |

**Global Helper Functions:**
Modal components expose global functions for cross-component communication:
- `openDeleteModal(sessionId, title)` - Open delete confirmation
- `openRenameModal(sessionId, title)` - Open rename form
- `openNewPersonaModal()` / `openEditPersonaModal()` - Persona modals
- `openDeletePersonaModal()` / `openEditPersonaModelModal()` - Persona modals
- `openContextFilesModal()` / `openWipeMemoryModal()` - Memory modals
- `openPersonaContextFilesModal()` - Persona context files modal

**Data Attribute Pattern:**
Components receive Django template data via `data-*` attributes:
```html
<div x-data="deleteModal"
     data-delete-url="{% url 'delete_chat' %}">
```

Components read these in their `init()` method:
```javascript
init() {
    this.deleteUrl = this.$el.dataset.deleteUrl;
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

**No Inline JavaScript:**
- All JavaScript must be placed in dedicated files (`utils.js` or `components.js`)
- Alpine.js components should be defined in `components.js` and registered via `Alpine.data()`
- Use data attributes (`data-*`) to pass Django template values to components
- Exception: Simple Alpine.js state objects like `x-data="{ open: false }"` are acceptable for one-off toggles

**No Inline CSS:**
- All styles must be defined in Tailwind classes or `input.css`
- Use Tailwind's utility classes for styling
- Custom CSS goes in `chat/static/css/input.css`
- Never use `style` attributes in HTML

**Architectural Best Practices:**

*Python/Django:*
- Keep views thin; business logic belongs in `services/`
- Use `load_config()` and `save_config()` for all config.json access
- Validate user input at view boundaries
- Use Django's CSRF protection for all POST requests
- Return appropriate HTTP status codes

*HTML/Templates:*
- Use Django template inheritance (`{% extends %}`, `{% block %}`)
- Create reusable partials for HTMX responses (`*_main.html` pattern)
- Use `{% include %}` for reusable components (icons, buttons)
- Pass data to Alpine components via `data-*` attributes, not inline JS

*JavaScript:*
- Define Alpine components as functions in `components.js`
- Register components in the `alpine:init` event
- Use `utils.js` for shared utility functions
- Dispatch custom events for cross-component communication
- Handle async operations properly with try/catch

*Security:*
- Never expose API keys in frontend code
- Validate and sanitize all user inputs
- Use Django's built-in CSRF protection
- Escape user content in templates (Django does this by default)

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
- [ ] Upload global context files in Memory view
- [ ] Change default persona in Persona Settings
- [ ] Set model override for persona
- [ ] Create new persona
- [ ] Edit persona content
- [ ] Verify "assistant" persona cannot be deleted

**Persona Context Files:**
- [ ] Upload context file to a persona via drag-drop or click
- [ ] Toggle file enabled/disabled status
- [ ] Edit file content inline
- [ ] Delete context file
- [ ] Badge count updates correctly
- [ ] Context appears in LLM prompt only for that persona's chats

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
| `chat/views/` | View functions (split by domain) |
| `chat/services/chat_core.py` | LLM API calls |
| `chat/services/llm_client.py` | Shared OpenRouter API utility |
| `chat/services/memory_manager.py` | Per-persona memory file I/O and update logic |
| `chat/services/memory_worker.py` | Background threading and auto-update scheduler |
| `chat/services/persona_context.py` | Persona-specific context file management |
| `chat/services/local_context.py` | Shared local directory context management |
| `chat/templates/chat/chat.html` | Main UI template |
| `chat/static/js/components.js` | Alpine.js component definitions |
| `chat/static/js/utils.js` | Shared utility functions |
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

## Cleanup Backlog

Prioritized list of improvements for code quality, separation of concerns, and best practices.

### Priority 1: Separation of Concerns

**1.1 Extract a `SessionManager` service**
The pattern of loading session JSON, resolving persona, getting model, loading context, and creating ChatCore is repeated 4+ times across `chat/views/chat.py` (lines ~115, ~378, ~622, ~787). Session file I/O (pin toggle, rename, draft save, message editing) is also done directly in views. Extract to a service with methods like `load_session_data()`, `create_chat_core_for_session()`, `toggle_pinned()`, `rename()`, `save_draft()`.

**1.2 ~~Split `Summarizer` into focused services~~ DONE**
Summarizer is now title-only. Memory logic extracted to `MemoryManager` (`memory_manager.py`) and `MemoryWorker` (`memory_worker.py`).

**1.3 Deduplicate `persona_context.py` and `user_context.py`**
These two files are near-identical duplicates — 7 functions (`get_config`, `save_config`, `list_files`, `upload_file`, `delete_file`, `toggle_file`, `load_enabled_context`) share the same logic with different storage paths. Extract a base `ContextFileManager` class parameterized by scope, eliminating ~250 lines of duplication.

**1.4 Move business logic out of views**
- `chat/views/personas.py`: session update logic on persona rename/delete
- `chat/views/chat.py`: title generation 3-tier logic, new session creation

### Priority 2: Code Duplication in Views

**2.1 Persona model map building**
The pattern of iterating personas and fetching models is repeated 7+ times across `chat.py`, `settings.py`, and `personas.py`. Extract to a shared utility.

**2.2 Model fetching & grouping**
`fetch_available_models()` → `group_models_by_provider()` → `flatten_models_with_provider_prefix()` chain is repeated in `core.py`, `settings.py`, and `personas.py`. Extract to a single helper.

**2.3 ~~Memory context dict building~~ DONE**
Consolidated into `_build_memory_view_context()` helper.

**2.4 Config loaded multiple times per request**
`load_config()` reads and parses `config.json` from disk on every call. Several views call it 2+ times. Load once per view and pass through.

### Priority 3: Django Best Practices

**3.1 Use `@require_POST` / `@require_http_methods` decorators**
Most POST-only views manually check `if request.method != 'POST'` with inconsistent handling. Use Django's built-in decorators instead — already used correctly in `api.py` but not elsewhere.

**3.2 Standardize error responses**
Error responses are inconsistent: some return `HttpResponse(status=405)` with no body, some return `JsonResponse({'error': ...})`, some return plain text. Create helper functions like `json_error(message, status)` and `method_not_allowed()`.

**3.3 Replace bare `except` clauses**
`chat/views/chat.py` has 5 instances of `except: pass` (lines ~125, ~387, ~630, ~840 and more). These mask real errors. Use specific exceptions like `(json.JSONDecodeError, IOError, ValueError)`.

**3.4 Add logging to important operations**
Most views don't log operations. Only `core.py` and `personas.py` use logging. Add logging for session creation/deletion, persona changes, API failures.

**3.5 ~~Fix import placement~~ DONE**
Resolved during memory refactor.

### Priority 4: JavaScript Cleanup

**4.1 Add `.catch()` to all promise chains**
`components.js` has 3 modal components using `.then()` chains without `.catch()`: `wipeMemoryModal` (line ~182), `editPersonaModal` (line ~259), `deletePersonaModal` (line ~324). Failed requests silently corrupt UI state.

**4.2 Standardize on `async/await`**
Mixed patterns throughout `components.js` — some methods use `async/await`, others use `.then()` chains. Standardize on `async/await` for consistency and better error handling.

**4.3 Standardize CSRF token retrieval**
`components.js` inconsistently uses `getCsrfToken()` (safe) in some places and `document.querySelector('[name=csrfmiddlewaretoken]').value` (throws if missing) in others (~4 instances in file upload components).

**4.4 Replace `window` global state with Alpine events**
7 modal components store refs on `window` (e.g., `window.deleteModalComponent`). Use Alpine's `$dispatch()` and `@custom-event` for cross-component communication instead.

**4.5 Remove hardcoded URL fallbacks**
`components.js` has ~6 hardcoded URL strings as fallback defaults. All URLs should come exclusively from `data-*` attributes passed by Django templates.

**4.6 Clean up `selectDropdown` MutationObserver**
`utils.js` lines ~742-758: The MutationObserver is never cleaned up (memory leak) and re-parses JSON on every attribute change. Use Alpine's native reactivity instead.

**4.7 Break down `providerModelSettings` component**
At 192 lines (components.js ~710-902), this component handles API validation, model fetching, provider selection, model selection, and form submission. Split into smaller focused components.

**4.8 Extract magic numbers to constants**
Timeout delays scattered throughout: `3000ms`, `1000ms`, `25ms` animation speed, retry delay of `2s`. Define as named constants.

### Priority 5: Template Cleanup

**5.1 Convert `onclick`/`oninput` handlers to Alpine directives**
13 inline handlers across 6 templates (`chat_main.html`, `chat_home.html`, `assistant_fragment.html`, `sidebar_sessions.html`, `memory_main.html`, `persona_main.html`). These violate the project's "no inline JS" rule. Convert to `@click` / `@input` Alpine directives.

**5.2 Extract inline fetch logic to `components.js`**
`memory_main.html` (lines ~34-48) and `settings_main.html` (lines ~128-140) have inline Alpine components with `fetch()` calls. Extract to registered components.

**5.3 Replace `window` global variables with `data-*` attributes**
`persona_main.html` and `memory_main.html` use `<script>` tags to set `window.personaContextFilesData` and `window.contextFilesData`. Pass via `data-*` attributes instead, consistent with other components.

**5.4 Extract modals from `base.html`**
`base.html` is 342 lines with 8 modal definitions inline. Extract to partial files (e.g., `templates/modals/`) and `{% include %}` them.

**5.5 Consolidate message rendering**
Message rendering differs slightly between `chat_main.html`, `assistant_fragment.html`, and `message_fragment.html`. Clarify which partial is used where, and consolidate where possible.

### Priority 6: Accessibility

**6.1 Add ARIA attributes to modals**
All 8 modals in `base.html` lack `role="dialog"`, `aria-modal="true"`, and `aria-labelledby`. Add focus trapping on modal open.

**6.2 Add `<label>` elements to form inputs**
Textarea in `chat_main.html` and input in `chat_home.html` lack associated `<label>` elements.

**6.3 Add `role="alert"` to error messages**
`utils.js` `handleMessageError()` creates error DOM without `role="alert"` or `aria-live` for screen readers.

**6.4 Use semantic list elements**
Sidebar session groups in `sidebar_sessions.html` use `<div>` instead of `<ul>`/`<li>`.

### Priority 7: Service Hardening

**7.1 ~~Extract hardcoded API values~~ DONE (partial)**
OpenRouter API URL extracted to `OPENROUTER_API_URL` constant in `llm_client.py`. Timeout values still scattered.

**7.2 Use atomic file writes**
Session saves, config saves, and memory updates use `json.dump()` without atomic writes. If the process crashes mid-write, files corrupt. Use the atomic write pattern already in `utils.py` (flush + fsync).

**7.3 Add timeout-specific error handling**
`ChatCore` and `Summarizer` set timeouts but don't distinguish `requests.exceptions.Timeout` from other errors. `ConfigManager` already does this correctly — follow the same pattern.

**7.4 ~~Validate API response structure~~ DONE**
`call_llm()` in `llm_client.py` validates `choices` key existence and content before returning.

**7.5 Add path traversal validation**
Persona context file operations should validate that `persona_name` doesn't contain `../` or other traversal characters, even though `os.path.basename()` is used on filenames.

---

## Resources

- **OpenRouter API:** https://openrouter.ai/docs
- **Django Docs:** https://docs.djangoproject.com/
- **Tailwind CSS:** https://tailwindcss.com/docs
- **HTMX Docs:** https://htmx.org/docs/
- **Alpine.js Docs:** https://alpinejs.dev/
- **Nord Theme:** https://www.nordtheme.com

---

**End of CLAUDE.md**
