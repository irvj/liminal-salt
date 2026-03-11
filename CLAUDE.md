# CLAUDE.md - Project Overview & Developer Guide

**Last Updated:** January 13, 2026
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
- **Long-Term Memory**: Automatic user profile building across all conversations
- **Grouped Sidebar**: Collapsible persona-based organization of chat threads
- **Pinned Chats**: Pin important conversations to the top of the sidebar
- **Smart Titles**: Multi-tier auto-generation of session titles with artifact detection
- **User Memory View**: Dedicated pane for viewing and managing long-term memory
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
│  - Summarizer: Title generation, memory updates         │
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

### Data Flow

```
User sends message (HTMX POST)
    ↓
Django view: send_message()
    ↓
ChatCore.send_message()
    ↓
Build API payload:
  1. System prompt (persona + user memory)
  2. Recent message history (configurable, default 50 pairs)
    ↓
POST to OpenRouter API (with retry logic)
    ↓
Response processing:
  - Clean tokens (<s>, </s>)
  - Handle empty responses
  - Error handling
    ↓
Update message history
    ↓
Save to session JSON file
    ↓
Return HTML fragment (HTMX swap)
```

### Request Flow

```
Browser                    Django                     Services
   │                          │                          │
   │──GET /chat/─────────────>│                          │
   │                          │──load session────────────>│
   │                          │<─session data────────────│
   │<─Full HTML page──────────│                          │
   │                          │                          │
   │──POST /chat/send/ (HTMX)─>│                          │
   │                          │──ChatCore.send_message()─>│
   │                          │                          │──>OpenRouter API
   │                          │                          │<──Response
   │                          │<─response────────────────│
   │<─HTML fragment (swap)────│                          │
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
│   ├── views.py                 # View functions
│   ├── utils.py                 # Helper functions
│   │
│   ├── services/                # Business logic layer
│   │   ├── __init__.py          # Exports all services
│   │   ├── chat_core.py         # LLM API & message handling
│   │   ├── config_manager.py    # Configuration management
│   │   ├── context_manager.py   # System prompt assembly
│   │   ├── persona_context.py   # Persona-specific context file management
│   │   ├── user_context.py      # Global user context file management
│   │   └── summarizer.py        # Title & memory generation
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
│       ├── icons/               # SVG icon components (20 icons)
│       │   ├── alert.html, brain-cog.html, check.html, check-circle.html
│       │   ├── chevron-down.html, chevron-right.html, chevrons-left.html
│       │   ├── circle-plus.html, folder.html, menu.html, moon.html
│       │   ├── pencil.html, plus.html, settings.html
│       │   ├── star-filled.html, star-outline.html, sun.html
│       │   ├── trash.html, user-pen.html, x.html
│       ├── chat/
│       │   ├── chat.html            # Main chat page (full)
│       │   ├── chat_home.html       # New chat home page
│       │   ├── chat_main.html       # Chat content partial
│       │   ├── sidebar_sessions.html # Sidebar session list
│       │   ├── new_chat_button.html # Reusable new chat button
│       │   ├── assistant_fragment.html
│       │   └── message_fragment.html
│       ├── memory/
│       │   ├── memory.html      # Memory page (full)
│       │   └── memory_main.html # Memory content partial
│       ├── persona/
│       │   └── persona_main.html # Persona settings partial
│       ├── settings/
│       │   ├── settings.html    # Settings page (full)
│       │   └── settings_main.html
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
    │       └── config.json      # Optional model override
    ├── user_context/            # User-uploaded context files
    │   ├── config.json          # Global context file settings
    │   ├── *.md, *.txt          # Global context files
    │   └── personas/            # Persona-specific context files
    │       └── [persona_name]/
    │           ├── config.json  # Persona context file settings
    │           └── *.md, *.txt  # Persona-specific files
    └── long_term_memory.md      # Persistent user profile
```

### Session File Format

```json
{
  "title": "Debugging Victory at Midnight",
  "persona": "assistant",
  "pinned": false,
  "messages": [
    {"role": "user", "content": "User message"},
    {"role": "assistant", "content": "Assistant response"}
  ]
}
```

---

## Key Components

### 1. Django Views (`chat/views.py`)

**Purpose:** Handle HTTP requests and coordinate between templates and services.

**Key Views:**
- `index()` - Entry point, redirects to setup or chat
- `setup_wizard()` - Two-step first-time setup (API key, model selection)
- `chat()` - Main chat view, handles both GET and HTMX requests
- `send_message()` - HTMX endpoint for sending messages
- `switch_session()` - HTMX endpoint for session switching
- `new_chat()` - Create new chat session
- `delete_chat()` - Delete current session
- `memory()` - User memory view
- `update_memory()` - Trigger memory update from all sessions
- `wipe_memory()` - Clear long-term memory
- `persona_settings()` - Persona management page
- `save_persona_file()` - Save/create persona content
- `save_persona_model()` - Set model override for persona
- `get_available_models()` - AJAX endpoint for lazy model loading
- `settings()` - Settings view
- `save_settings()` - Save settings changes

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
- **Error Handling:** Returns "ERROR: ..." string on failures

### 3. Context Manager (`chat/services/context_manager.py`)

**Purpose:** Assembles the complete system prompt from persona and memory.

**Key Functions:**
- `load_context(persona_dir, ltm_file)` - Loads and concatenates context
- `get_available_personas(personas_dir)` - Returns list of valid personas
- `get_persona_config(persona_name, personas_dir)` - Loads persona config.json
- `get_persona_model(persona_name, personas_dir)` - Gets model override for persona

**Assembly Order:**
1. All `.md` files from persona directory (alphabetically)
2. Persona-specific context files (from `data/user_context/personas/[name]/`)
3. Global user context files (from `data/user_context/`)
4. Long-term memory file with explicit disclaimer

### 4. Summarizer (`chat/services/summarizer.py`)

**Purpose:** Generates session titles and updates long-term memory.

**Key Methods:**
- `generate_title(first_user_msg, first_assistant_msg)` - Creates 2-4 word title
- `update_long_term_memory(messages, ltm_file)` - Updates user profile

### 5. Templates

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

### 6. JavaScript Architecture (`chat/static/js/`)

**Purpose:** Centralized Alpine.js components and utility functions, extracted from inline scripts for better maintainability and reusability.

**`utils.js` - Shared Utility Functions:**
- `getAvailableThemes()` - Fetches theme list from backend API
- `loadTheme()` / `setTheme()` / `getTheme()` - Color theme and mode management
- `saveThemePreference()` - Persists theme to backend config.json
- `applyThemeColors()` / `cacheThemeColors()` - CSS custom property management
- `getCsrfToken()` - Centralized CSRF token retrieval
- `handleTextareaKeydown()` / `autoResizeTextarea()` - Textarea helpers
- `scrollToBottom()` / `updateScrollButtonVisibility()` - Scroll management
- `addUserMessage()` / `removeThinkingIndicator()` - Message UI helpers
- `animateAssistantResponse()` / `typewriterReveal()` - Response animation
- `convertTimestamps()` / `insertDateSeparators()` - Date formatting
- `scrollDropdownToHighlighted()` - Dropdown keyboard navigation
- `toDisplayName()` / `toFolderName()` - Persona name conversion

**`components.js` - Alpine.js Component Definitions:**

Components are registered via `Alpine.data()` in the `alpine:init` event, making them available across all templates.

| Component | Purpose |
|-----------|---------|
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
- **Memory** (brain-cog icon) - View/manage long-term memory
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

### Long-Term Memory

- Read-only display in main pane
- "Update User Memory" aggregates sessions based on configurable limits
- **Memory Generation Limits:** Control how much history is sent when generating memory
  - **Recent Threads:** Limit to N most recent chat threads (default 10, 0 = unlimited)
  - **Messages Per Thread:** Limit to N most recent messages from each thread (default 100, 0 = unlimited)
- "Wipe Memory" with confirmation
- Status indicator shows update progress
- Context files can be uploaded to augment memory

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
    "USER_HISTORY_MAX_THREADS": 10,
    "USER_HISTORY_MESSAGES_PER_THREAD": 100,
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
- `USER_HISTORY_MAX_THREADS`: Max threads to include when generating user memory (default 10, 0 = unlimited)
- `USER_HISTORY_MESSAGES_PER_THREAD`: Max messages per thread when generating user memory (default 100, 0 = unlimited)
- `THEME`: Color theme identifier (one of 16 themes: liminal-salt [default], nord, dracula, gruvbox, monokai, solarized, rose-pine, tokyo-night, one-dark, night-owl, catppuccin, ayu, everforest, kanagawa, palenight, synthwave)
- `THEME_MODE`: Light or dark mode preference

### Django Settings (`liminal_salt/settings.py`)

Key customizations:
- `DATABASES = {}` - No database required
- `SESSION_ENGINE = 'django.contrib.sessions.backends.signed_cookies'`
- `DATA_DIR`, `SESSIONS_DIR`, `PERSONAS_DIR`, `LTM_FILE` - Data paths

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

### Adding a New Persona

1. Create a new folder in `data/personas/`:
   ```bash
   mkdir data/personas/mybot
   ```

2. Create `identity.md`:
   ```markdown
   # My Bot Persona

   You are a helpful assistant specialized in...

   ## Communication Style
   - Clear and concise
   - Professional tone
   ```

3. Optionally create `config.json` for model override:
   ```json
   {
     "model": "anthropic/claude-sonnet-4"
   }
   ```

4. Restart server (persona appears in dropdown automatically)

### Adding a New Theme

1. Create a new JSON file in `chat/static/themes/`:
   ```bash
   touch chat/static/themes/mytheme.json
   ```

2. Define the theme with dark and light variants:
   ```json
   {
     "name": "My Theme",
     "id": "mytheme",
     "dark": {
       "surface": "#1a1a2e",
       "surface-secondary": "#16213e",
       "surface-elevated": "#0f3460",
       "foreground": "#eaeaea",
       "foreground-secondary": "#b8b8b8",
       "foreground-muted": "#666666",
       "foreground-on-accent": "#ffffff",
       "accent": "#e94560",
       "accent-hover": "#ff6b6b",
       "accent-cyan": "#4ecdc4",
       "success": "#2ecc71",
       "danger": "#e74c3c",
       "danger-hover": "#c0392b",
       "warning": "#f39c12",
       "border": "#0f3460",
       "user-bubble": "#e94560",
       "assistant-bubble": "#0f3460"
     },
     "light": {
       "surface": "#f8f9fa",
       "surface-secondary": "#e9ecef",
       "surface-elevated": "#dee2e6",
       "foreground": "#212529",
       "foreground-secondary": "#495057",
       "foreground-muted": "#adb5bd",
       "foreground-on-accent": "#ffffff",
       "accent": "#e94560",
       "accent-hover": "#ff6b6b",
       "accent-cyan": "#4ecdc4",
       "success": "#2ecc71",
       "danger": "#e74c3c",
       "danger-hover": "#c0392b",
       "warning": "#f39c12",
       "border": "#dee2e6",
       "user-bubble": "#e94560",
       "assistant-bubble": "#dee2e6"
     }
   }
   ```

3. Theme appears automatically in all theme pickers (setup wizard, settings page)

### SVG Icon System

Icons are stored as reusable Django template includes in `chat/templates/icons/`.

**Usage:**
```html
<!-- Basic usage (inherits parent text color) -->
{% include 'icons/trash.html' %}

<!-- With custom size -->
{% include 'icons/trash.html' with class='w-4 h-4' %}

<!-- With custom color via parent -->
<span class="text-danger">{% include 'icons/trash.html' %}</span>
```

**Icon template pattern:**
```html
<svg class="{{ class|default:'w-5 h-5' }}" viewBox="0 0 24 24" fill="none"
     stroke="currentColor" stroke-width="2" stroke-linecap="round"
     stroke-linejoin="round" aria-hidden="true">
    <!-- SVG paths -->
</svg>
```

**Key design decisions:**
- Icons use `currentColor` to inherit text color from parent element
- Default size is `w-5 h-5` (20px), overridable via `class` parameter
- All icons include `aria-hidden="true"` (decorative)
- No wrapper elements - parent controls styling

**Available icons (20):**
`alert`, `brain-cog`, `check`, `check-circle`, `chevron-down`, `chevron-right`,
`chevrons-left`, `circle-plus`, `folder`, `menu`, `moon`, `pencil`, `plus`,
`settings`, `star-filled`, `star-outline`, `sun`, `trash`, `user-pen`, `x`

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
/memory/                       → memory
/memory/update/                → update_memory
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

### HTMX Patterns Used

```html
<!-- Session switching -->
<button hx-post="/chat/switch/"
        hx-vals='{"session_id": "..."}'
        hx-target="#main-content"
        hx-swap="innerHTML">

<!-- Form submission -->
<form hx-post="/chat/send/"
      hx-target="#messages"
      hx-swap="beforeend">

<!-- Load content in pane -->
<button hx-get="/memory/"
        hx-target="#main-content"
        hx-swap="innerHTML">
```

### Alpine.js Patterns Used

**Registered Components (preferred):**
Components are defined in `components.js` and registered via `Alpine.data()`. Templates reference them by name with data attributes for initialization:

```html
<!-- Using a registered modal component -->
<div x-data="deleteModal"
     data-delete-url="{% url 'delete_chat' %}"
     data-csrf-token="{{ csrf_token }}">
    <!-- Component handles its own state and logic -->
</div>

<!-- Using a registered searchable dropdown -->
<div x-data="homePersonaPicker"
     data-default-persona="{{ default_persona }}"
     data-personas='{{ personas_json|safe }}'>
    <input type="text" x-model="search" @focus="open = true">
    <!-- Dropdown renders from component's filteredItems -->
</div>

<!-- Collapsible section with registered component -->
<div x-data="collapsibleSection">
    <button @click="toggle()">
        <span x-show="open">{% include 'icons/chevron-down.html' %}</span>
        <span x-show="!open">{% include 'icons/chevron-right.html' %}</span>
    </button>
    <div x-show="open">Content</div>
</div>
```

**Simple Inline Patterns (for one-off toggles):**
```html
<!-- Simple toggle (no need for registered component) -->
<div x-data="{ open: false }">
    <button @click="open = !open">Toggle</button>
    <div x-show="open">Content</div>
</div>
```

**Global Functions:**
Utility functions from `utils.js` are available globally:
```html
<!-- Theme toggle using global function -->
<button onclick="toggleTheme()">Toggle Theme</button>

<!-- Opening modals via global helpers -->
<button onclick="openDeleteModal('session-123', 'Chat Title')">Delete</button>
```

### Testing Checklist

**Basic Operations:**
- [ ] Create new chat session with persona selection
- [ ] Send messages and receive responses
- [ ] Switch between sessions (HTMX)
- [ ] Delete session with confirmation
- [ ] Pin/unpin chat sessions

**Theme System:**
- [ ] Select theme during setup wizard (step 2)
- [ ] Change color theme in Settings
- [ ] Toggle dark/light mode (sidebar and settings stay in sync)
- [ ] Theme persists after page refresh
- [ ] Theme persists in new browser (server-side storage)
- [ ] No flash of wrong theme on page load

**Memory & Settings:**
- [ ] View User Memory in main pane
- [ ] Update memory, see status indicator
- [ ] Wipe memory with confirmation
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
| `chat/views.py` | All view logic |
| `chat/services/chat_core.py` | LLM API calls |
| `chat/services/persona_context.py` | Persona-specific context file management |
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
rm -rf data/sessions/*.json data/long_term_memory.md
```

### API Endpoint

```
https://openrouter.ai/api/v1/chat/completions
```

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
