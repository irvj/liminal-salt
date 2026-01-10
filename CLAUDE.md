# CLAUDE.md - Project Overview & Developer Guide

**Last Updated:** January 10, 2026
**Project:** Liminal Salt - Multi-Session LLM Chatbot with Personalities
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

**Liminal Salt** is a Python-based web chatbot application that connects to OpenRouter's API to provide LLM-powered conversations with persistent memory and multiple personalities.

### Key Features

- **Multi-Session Management**: Create, switch between, and manage multiple chat sessions
- **Personality System**: Per-session personality selection with customizable personalities
- **Long-Term Memory**: Automatic user profile building across all conversations
- **Grouped Sidebar**: Collapsible personality-based organization of chat threads
- **Smart Titles**: Multi-tier auto-generation of session titles with artifact detection
- **User Memory View**: Dedicated pane for viewing and managing long-term memory
- **Settings Management**: Configure default personality for new chats
- **Nord Theme**: Custom dark theme for the interface
- **Reactive UI**: HTMX-powered updates without full page reloads

### Technology Stack

- **Language:** Python 3.x
- **Web Framework:** Django 5.x (no database)
- **Frontend:** HTMX + Alpine.js
- **CSS Framework:** Tailwind CSS v4 with @tailwindcss/typography
- **Build Tools:** Node.js / npm for CSS compilation
- **API:** OpenRouter (LLM gateway)
- **HTTP Client:** requests
- **Data Storage:** JSON files for sessions, Markdown for memory and personalities
- **Sessions:** Django signed cookie sessions (no database required)
- **UI Theme:** Nord color scheme (dark and light modes)

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
  1. System prompt (personality + user memory)
  2. Recent message history (last 100 messages)
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
│   │   └── summarizer.py        # Title & memory generation
│   │
│   ├── static/                  # Static assets
│   │   └── css/
│   │       ├── input.css        # Tailwind source & theme config
│   │       └── output.css       # Compiled CSS (minified)
│   │
│   └── templates/               # Django templates
│       ├── base.html            # Base template with HTMX/Alpine
│       ├── chat/
│       │   ├── chat.html        # Main chat page (full)
│       │   ├── chat_main.html   # Chat content partial
│       │   ├── assistant_fragment.html
│       │   └── new_chat.html
│       ├── memory/
│       │   ├── memory.html      # Memory page (full)
│       │   └── memory_main.html # Memory content partial
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
    ├── personalities/           # Personality definitions
    │   └── assistant/
    │       └── identity.md
    └── long_term_memory.md      # Persistent user profile
```

### Session File Format

```json
{
  "title": "Debugging Victory at Midnight",
  "personality": "assistant",
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
- **Sliding Window:** Maintains last 100 messages in API payload
- **Error Handling:** Returns "ERROR: ..." string on failures

### 3. Context Manager (`chat/services/context_manager.py`)

**Purpose:** Assembles the complete system prompt from personality and memory.

**Key Functions:**
- `load_context(personality_dir, ltm_file)` - Loads and concatenates context
- `get_available_personalities(personalities_dir)` - Returns list of valid personalities

**Assembly Order:**
1. All `.md` files from personality directory (alphabetically)
2. Long-term memory file with explicit disclaimer

### 4. Summarizer (`chat/services/summarizer.py`)

**Purpose:** Generates session titles and updates long-term memory.

**Key Methods:**
- `generate_title(first_user_msg, first_assistant_msg)` - Creates 2-4 word title
- `update_long_term_memory(messages, ltm_file)` - Updates user profile

### 5. Templates

**Base Template (`base.html`):**
- Loads HTMX and Alpine.js from CDN
- Loads compiled Tailwind CSS from `static/css/output.css`
- Configures CSRF token for HTMX requests
- Uses semantic Tailwind classes (bg-surface, text-foreground, etc.)

**Main Chat (`chat/chat.html`):**
- Full page with sidebar + main content area
- Alpine.js modals for new chat, delete confirmation, wipe memory
- HTMX attributes for reactive session switching

**Partials (`*_main.html`):**
- Content fragments returned by HTMX requests
- Swapped into `#main-content` div

---

## Features

### Collapsible Personality-Grouped Sidebar

Sessions are organized by personality with collapsible sections:

```
▼ Assistant (3)
  Session Title 1          🗑️
  Session Title 2          🗑️
  Session Title 3          🗑️

▶ Custom (2)
```

- Click personality header to toggle collapse/expand
- Arrow indicator (▼ expanded, ▶ collapsed)
- Count badge shows number of sessions
- Current session highlighted in bold
- Delete button with confirmation modal

### HTMX-Powered Reactivity

- **Session Switching:** Click session → HTMX swaps main content
- **Send Message:** Form submit → HTMX appends response
- **Memory/Settings:** Load in main pane without navigation
- **Modals:** Alpine.js handles show/hide state

### Per-Session Personalities

- **Selection:** Choose personality when creating new chat
- **Persistence:** Personality saved in session JSON
- **Isolation:** Each session maintains its own personality
- **Default:** Configurable default personality for new chats

### Long-Term Memory

- Read-only display in main pane
- "Update User Memory" aggregates all sessions
- "Wipe Memory" with confirmation
- Status indicator shows update progress

---

## How to Run

### Quick Start (Users)

```bash
python run.py
```

The launcher automatically creates a virtual environment and installs dependencies on first run. Access at `http://localhost:8000`

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

1. Navigate to `http://localhost:8000`
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
    "DEFAULT_PERSONALITY": "assistant",
    "MAX_HISTORY": 50
}
```

**Key Settings:**
- `OPENROUTER_API_KEY`: Your API key from OpenRouter
- `MODEL`: LLM model to use
- `DEFAULT_PERSONALITY`: Default for new chats
- `MAX_HISTORY`: Number of message pairs to keep in context

### Django Settings (`liminal_salt/settings.py`)

Key customizations:
- `DATABASES = {}` - No database required
- `SESSION_ENGINE = 'django.contrib.sessions.backends.signed_cookies'`
- `DATA_DIR`, `SESSIONS_DIR`, `PERSONALITIES_DIR`, `LTM_FILE` - Data paths

---

## Development Notes

### Adding a New Personality

1. Create a new folder in `data/personalities/`:
   ```bash
   mkdir data/personalities/mybot
   ```

2. Create `identity.md`:
   ```markdown
   # My Bot Personality

   You are a helpful assistant specialized in...

   ## Communication Style
   - Clear and concise
   - Professional tone
   ```

3. Restart server (personality appears in dropdown automatically)

### URL Routes

```
/                  → index (redirect to /chat/ or /setup/)
/setup/            → setup_wizard
/chat/             → chat (main view)
/chat/send/        → send_message (HTMX)
/chat/switch/      → switch_session (HTMX)
/chat/new/         → new_chat
/chat/delete/      → delete_chat
/memory/           → memory
/memory/update/    → update_memory
/memory/wipe/      → wipe_memory
/settings/         → settings
/settings/save/    → save_settings
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

```html
<!-- Modal component -->
<div x-data="{ showModal: false }">
    <button @click="showModal = true">Open</button>
    <div x-show="showModal" class="modal">
        <button @click="showModal = false">Close</button>
    </div>
</div>

<!-- Collapsible group -->
<div x-data="{ open: true }">
    <button @click="open = !open">
        <span x-text="open ? '▼' : '▶'"></span> Title
    </button>
    <div x-show="open">Content</div>
</div>
```

### Testing Checklist

**Basic Operations:**
- [ ] Create new chat session with personality selection
- [ ] Send messages and receive responses
- [ ] Switch between sessions (HTMX)
- [ ] Delete session with confirmation

**Memory & Settings:**
- [ ] View User Memory in main pane
- [ ] Update memory, see status indicator
- [ ] Wipe memory with confirmation
- [ ] Change default personality in Settings

**Edge Cases:**
- [ ] First launch (no config.json)
- [ ] Empty sessions directory
- [ ] Invalid API key

---

## Quick Reference

### Important Files

| File | Purpose |
|------|---------|
| `chat/views.py` | All view logic |
| `chat/services/chat_core.py` | LLM API calls |
| `chat/templates/chat/chat.html` | Main UI template |
| `chat/static/css/input.css` | Tailwind source & theme config |
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
