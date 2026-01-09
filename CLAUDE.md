# CLAUDE.md - Project Overview & Developer Guide

**Last Updated:** January 9, 2026
**Project:** Liminal Salt - Multi-Session LLM Chatbot with Personalities
**Status:** Production-ready Streamlit application

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
9. [Future Plans](#future-plans)

---

## Project Overview

**Liminal Salt** is a Python-based web chatbot application that connects to OpenRouter's API to provide LLM-powered conversations with persistent memory and multiple personalities.

### Key Features

- **Multi-Session Management**: Create, switch between, and manage multiple chat sessions
- **Personality System**: Per-session personality selection with customizable personalities
- **Long-Term Memory**: Automatic user profile building across all conversations
- **Grouped Sidebar**: Collapsible personality-based organization of chat threads
- **Smart Titles**: Multi-tier auto-generation of session titles with artifact detection
- **User Memory View**: Dedicated page for viewing and managing long-term memory
- **Settings Management**: Configure default personality for new chats
- **Nord Theme**: Custom dark theme for the interface

### Technology Stack

- **Language:** Python 3.x
- **Web Framework:** Streamlit
- **API:** OpenRouter (LLM gateway)
- **HTTP Client:** requests
- **Data Storage:** JSON files for sessions, Markdown for memory and personalities
- **UI Theme:** Nord color scheme

---

## Architecture

### System Overview

```
┌─────────────────────────────────────────────────────────┐
│                    Streamlit Web UI                     │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  │
│  │   Sidebar    │  │  Chat View   │  │   Settings   │  │
│  │ - Grouped by │  │ - Messages   │  │ - Default    │  │
│  │   personality│  │ - Input      │  │   personality│  │
│  │ - Collapsible│  │ - Titles     │  │              │  │
│  └──────────────┘  └──────────────┘  └──────────────┘  │
└────────────────────┬────────────────────────────────────┘
                     │
                     ▼
┌─────────────────────────────────────────────────────────┐
│                   ChatCore Logic                        │
│  - Message history management                           │
│  - API request/response handling                        │
│  - Session persistence                                  │
│  - Retry logic for failed requests                      │
└────────────────────┬────────────────────────────────────┘
                     │
                     ▼
┌─────────────────────────────────────────────────────────┐
│              Context Assembly                           │
│  - Load personality .md files                           │
│  - Load long-term memory                                │
│  - Assemble system prompt                               │
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
User sends message
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
Save to session JSON file:
  - title
  - personality
  - messages
    ↓
Display to user
```

### Personality System

Each chat session has an assigned personality that determines the assistant's communication style:

```
personalities/
├── assistant/
│   └── identity.md      # Professional, helpful assistant
└── custom/
    └── identity.md      # Custom personality (user-defined)
```

Personalities are:
- **Per-session**: Each chat maintains its own personality
- **Persistent**: Saved in session JSON files
- **Selectable**: Chosen during new chat creation
- **Expandable**: Add new personality folders with .md files

---

## File Structure

```
chat-test/
├── webapp.py                    # Streamlit web interface (main entry point)
├── chat_core.py                 # Core chatbot logic & API calls
├── context_manager.py           # System prompt assembly from personalities
├── config_manager.py            # Configuration loader
├── summarizer.py                # Title generation & memory updates
├── config.json                  # API keys & settings
├── requirements.txt             # Python dependencies
├── long_term_memory.md          # Persistent user profile (auto-generated)
├── CLAUDE.md                    # This file - project documentation
├── .gitignore                   # Git ignore rules
├── .streamlit/
│   └── config.toml              # Streamlit theme (Nord colors)
├── personalities/
│   ├── assistant/
│   │   └── identity.md          # Professional assistant personality
│   └── custom/
│       └── identity.md          # Custom personality
└── sessions/
    └── session_*.json           # Individual chat sessions
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

### 1. Web Interface (`webapp.py`)

**Purpose:** Streamlit-based multi-session chat application with personality management.

**Main Sections:**
- **Sidebar:**
  - New chat button with personality selection dialog
  - Personality-grouped collapsible chat thread list
  - User Memory and Settings navigation buttons

- **Chat View:**
  - Message display with error handling
  - Chat input with real-time responses
  - Session title displayed in caption with personality indicator

- **User Memory View:**
  - Display long-term memory content
  - Update memory button (aggregates all sessions)
  - Wipe memory button with confirmation
  - Last updated timestamp

- **Settings View:**
  - Default personality selector
  - Personality preview
  - Set as default button

**Key Functions:**
- `get_sessions_with_titles()` - Loads all sessions with title, id, and personality
- `group_sessions_by_personality()` - Groups sessions by personality for sidebar display
- `toggle_personality_group()` - Handles collapse/expand state
- `load_chat_core()` - Initializes ChatCore with appropriate personality context
- `create_new_chat_dialog()` - Modal for new chat creation with personality selection
- `confirm_delete_dialog()` - Confirmation modal for session deletion
- `confirm_wipe_memory_dialog()` - Confirmation modal for memory wipe

### 2. ChatCore (`chat_core.py`)

**Purpose:** Handles all LLM API interactions and message history management.

**Key Methods:**
- `__init__(api_key, model, site_url, site_name, system_prompt, max_history, history_file, personality)`
- `send_message(user_input)` - Sends message with retry logic, returns response
- `clear_history()` - Wipes conversation history
- `_get_payload_messages()` - Assembles messages for API (system + last 100 messages)
- `_save_history()` - Persists session to JSON (title, personality, messages)
- `_load_history()` - Loads session from JSON with personality fallback

**Features:**
- **Retry Logic:** Up to 2 attempts for empty responses with 2-second delay
- **Token Cleanup:** Removes `<s>` and `</s>` artifacts
- **Sliding Window:** Maintains last 100 messages (50 exchanges) in API payload
- **Error Handling:** Returns "ERROR: ..." string on failures

### 3. Context Manager (`context_manager.py`)

**Purpose:** Assembles the complete system prompt from personality and memory.

**Key Functions:**
- `load_context(personality_dir, ltm_file)` - Loads and concatenates context
- `get_available_personalities(personalities_dir)` - Returns list of valid personalities

**Assembly Order:**
1. All `.md` files from personality directory (alphabetically)
2. Long-term memory file with explicit disclaimer:
   ```
   --- USER PROFILE (BACKGROUND KNOWLEDGE) ---
   The following information describes the USER (not you).
   Use this to understand who you're talking to, but DO NOT let it
   change your personality or communication style.
   ```

**Personality Structure:**
Each personality folder can contain multiple `.md` files that define:
- Identity and role
- Communication style
- Behavior guidelines
- Capabilities and limitations

### 4. Summarizer (`summarizer.py`)

**Purpose:** Generates session titles and updates long-term memory.

**Key Methods:**
- `generate_title(first_user_msg, first_assistant_msg)` - Creates 2-4 word title
- `update_long_term_memory(messages, ltm_file)` - Updates user profile

**Title Generation Features:**
- **Tier 1:** Attempts on first message pair (if response is valid)
- **Tier 2:** Retry after second response if title is still "New Chat"
- **Tier 3:** Fix titles with artifacts (brackets, tags, special characters)
- **Artifact Detection:** Identifies and regenerates malformed titles

**Memory Update Process:**
1. Load existing long-term memory
2. Format conversation messages
3. Send to LLM with update instructions
4. Safety check: Won't overwrite substantial content with suspiciously short updates
5. Write updated profile back to file

---

## Features

### Collapsible Personality-Grouped Sidebar

Sessions are organized by personality with collapsible sections:

```
▼ Custom (3)
  Debugging Victory at Midnight  🗑️
  Data Analysis Project         🗑️
  Role-play Session             🗑️

▶ Assistant (2)
```

- Click personality header to toggle collapse/expand
- Arrow indicator (▼ expanded, ▶ collapsed)
- Count badge shows number of sessions
- Personalities ordered by most recent thread
- Threads within groups sorted newest-first
- Current session highlighted in bold
- Delete button (🗑️) with confirmation dialog

### Per-Session Personalities

- **Selection:** Choose personality when creating new chat
- **Persistence:** Personality saved in session JSON
- **Isolation:** Each session maintains its own personality
- **Default:** Configurable default personality for new chats
- **Fallback:** Sessions without personality default to "assistant"

### Long-Term Memory Management

**User Memory View Features:**
- Read-only display of memory content
- "Update User Memory" button aggregates all sessions
- "Wipe Memory" button with confirmation dialog
- Last updated timestamp
- Memory format: Markdown with user profile and knowledge base

**Memory Update Trigger:**
- Manual: Via button in User Memory view
- Aggregates messages from ALL sessions (not just current)
- Updates timestamp on success

### Smart Session Management

**Creation:**
- Dialog-based with personality selector
- Auto-generates session ID with timestamp
- Defaults to configured personality

**Deletion:**
- Confirmation dialog prevents accidents
- Auto-switches to another session if deleting current
- Creates new session if last one deleted
- Group disappears if last session in personality deleted

**Switching:**
- Click session title to switch
- Reloads ChatCore with correct personality context
- Highlights current session in bold
- Maintains collapse states across switches

---

## How to Run

### Prerequisites

```bash
pip install -r requirements.txt
```

### Start Application

```bash
streamlit run webapp.py
```

Access at `http://localhost:8501`

### First-Time Setup

1. Edit `config.json` with your OpenRouter API key
2. Configure default personality (defaults to "assistant")
3. Launch application
4. Create your first chat session

---

## Configuration

### config.json

```json
{
    "OPENROUTER_API_KEY": "sk-or-v1-...",
    "MODEL": "anthropic/claude-haiku-4.5",
    "SITE_URL": "http://localhost:3000",
    "SITE_NAME": "Liminal Salt",
    "DEFAULT_PERSONALITY": "assistant",
    "PERSONALITIES_DIR": "personalities",
    "MAX_HISTORY": 50,
    "SESSIONS_DIR": "sessions",
    "LTM_FILE": "long_term_memory.md"
}
```

**Key Settings:**
- `OPENROUTER_API_KEY`: Your API key from OpenRouter
- `MODEL`: LLM model to use (e.g., "anthropic/claude-haiku-4.5")
- `DEFAULT_PERSONALITY`: Default for new chats (must be valid personality folder name)
- `MAX_HISTORY`: Number of message pairs to keep in context (50 = 100 messages)
- `PERSONALITIES_DIR`: Folder containing personality definitions
- `SESSIONS_DIR`: Folder for session JSON files
- `LTM_FILE`: Filename for long-term memory

### Streamlit Theme (.streamlit/config.toml)

Nord-themed dark mode configuration:
- Primary color: Nord blue (#5E81AC)
- Background: Nord polar night (#2E3440)
- Secondary background: #3B4252
- Text: Nord snow storm (#ECEFF4)

---

## Development Notes

### Adding a New Personality

1. Create a new folder in `personalities/`:
   ```bash
   mkdir personalities/mybot
   ```

2. Create `identity.md` (or multiple `.md` files):
   ```markdown
   # My Bot Personality

   You are a helpful assistant specialized in...

   ## Communication Style
   - Clear and concise
   - Professional tone

   ## Capabilities
   - Answer questions
   - Provide examples
   ```

3. Restart Streamlit (personality appears in dropdown automatically)

### Modifying Existing Personalities

Edit `.md` files in `personalities/<name>/` folder. Changes take effect on next chat load (may need to switch away and back to session).

### Session State Management

Streamlit session state tracks:
- `current_session`: Active session ID
- `view_mode`: "chat" | "profile" | "settings"
- `chat`: ChatCore instance
- `summarizer`: Summarizer instance
- `collapsed_personalities`: Dict mapping personality → collapsed state
- `session_personalities`: Dict mapping session_id → personality (transient)
- `last_memory_update`: Timestamp of last memory update
- `last_loaded_session`: Last loaded session ID (for reload detection)

### API Headers

```python
{
    "Authorization": f"Bearer {api_key}",
    "Content-Type": "application/json",
    "HTTP-Referer": site_url,
    "X-Title": site_name
}
```

### Error Handling Patterns

**API Errors:**
- Retry up to 2 times with 2-second delay
- Return "ERROR: ..." string on failure
- Error messages displayed with expandable details

**Empty Responses:**
- Retry automatically
- Log attempt failures
- Show error after all retries exhausted

**File Errors:**
- Silent exception handling in most cases
- Graceful fallbacks (e.g., "assistant" personality)
- Default values for missing fields

---

## Future Plans

### Short-Term Improvements

1. **Enhanced Error Handling:**
   - Structured logging with Python `logging` module
   - User-friendly error messages
   - Debug mode toggle

2. **UI Enhancements:**
   - Search/filter threads
   - Bulk session operations
   - Export conversations
   - Session tags/categories

3. **Performance:**
   - Lazy loading for large session lists
   - Caching for repeated personality loads
   - Async memory updates

4. **Features:**
   - Markdown rendering in chat
   - Code syntax highlighting
   - Image support in messages
   - Conversation branching

### Django Migration Plan (Future)

**Current State:** Streamlit-based single-user application
**Target State:** Django-based single-user application with flat files

#### Migration Overview

**Why Django?**
- Better web framework for custom UI/UX
- More control over routing and URL structure
- Cleaner template system (Jinja2)
- Easier to extend with custom features
- Better development server than Streamlit
- Standard Python web development patterns

**Important: What's NOT Changing:**
- **Still single-user** - No authentication or user management
- **Still flat files** - JSON for sessions, Markdown for memory/personalities
- **Still local-only** - Designed to run on user's own machine
- **Still forkable** - Easy for users to download and run

**Scope:**
- Convert Streamlit UI to Django templates + HTMX/Alpine.js
- **Keep** JSON file storage (no database)
- **Keep** single-user model (no auth)
- Improve UI/UX with better frontend control
- Preserve all current features (personalities, memory, etc.)
- Maintain simple setup (just `python manage.py runserver`)

#### Key Changes Required

1. **Storage: No Change**
   ```
   sessions/
   ├── session_*.json    # Same format, same location
   personalities/
   ├── assistant/
   └── custom/
   long_term_memory.md   # Same format
   ```

2. **Architecture Shift:**
   - **From:** Streamlit session state
   - **To:** Django sessions (cookie-based, no database needed)
   - **From:** Streamlit components
   - **To:** Django templates + HTMX for reactivity
   - **From:** st.rerun() pattern
   - **To:** AJAX partial updates
   - **Keep:** File-based storage
   - **Keep:** Single-user local deployment

3. **Core Components:**
   - `ChatCore` → Keep as utility class, called from views
   - `Summarizer` → Keep as utility class, no background tasks needed
   - `context_manager` → Keep as utility module
   - Session management → Django views reading/writing JSON files
   - Settings → Django forms + JSON file updates

4. **File Mapping:**
   ```
   webapp.py → views.py + templates/ + urls.py
   chat_core.py → chat_core.py (minimal changes)
   context_manager.py → context_manager.py (no changes)
   summarizer.py → summarizer.py (no changes)
   config.json → config.json (no changes, maybe add settings.py wrapper)
   ```

#### Migration Steps (High-Level)

1. **Phase 1: Django Setup**
   - Initialize Django project structure
   - Configure Django to work without database (SESSION_ENGINE = 'django.contrib.sessions.backends.signed_cookies')
   - Create URL routing for chat, memory, settings views
   - Set up static files and templates directory
   - Keep ChatCore, Summarizer, context_manager as-is

2. **Phase 2: Template Conversion**
   - Convert Streamlit UI to Django templates
   - Sidebar → Base template with includes
   - Chat view → Template with message list
   - Memory view → Simple display template
   - Settings view → Form template
   - Use Tailwind CSS for styling (similar to current Nord theme)

3. **Phase 3: Interactivity**
   - Implement HTMX for dynamic updates
   - Chat message sending without page reload
   - Sidebar session switching without page reload
   - Collapsible personality groups with Alpine.js
   - Delete confirmations with modals
   - Title updates without page reload

4. **Phase 4: Testing & Polish**
   - Test all features work identically to Streamlit version
   - Verify JSON file reading/writing works correctly
   - Test personality system
   - Polish UI/UX
   - Update documentation

#### Technical Stack (Proposed)

- **Backend:** Django 5.x (no Django REST Framework needed)
- **Database:** None - using signed cookie sessions
- **Frontend:** Django templates, HTMX, Alpine.js, Tailwind CSS
- **Storage:** JSON files + Markdown (same as current)
- **Task Queue:** None needed (synchronous operations)
- **Deployment:** `python manage.py runserver` (local development server)

#### Simplified Django Settings

```python
# settings.py (key excerpts)
INSTALLED_APPS = [
    'django.contrib.sessions',     # For session management
    'django.contrib.staticfiles',  # For CSS/JS
    'chatbot',                     # Main app
]

# No database needed
DATABASES = {}

# Use signed cookie sessions (no DB required)
SESSION_ENGINE = 'django.contrib.sessions.backends.signed_cookies'

# File storage paths (same as current)
SESSIONS_DIR = 'sessions'
PERSONALITIES_DIR = 'personalities'
LTM_FILE = 'long_term_memory.md'
```

#### Risks & Considerations

- **Learning Curve:** Django patterns differ from Streamlit (but simpler without DB/auth)
- **More Boilerplate:** More files to manage (views, urls, templates)
- **Manual Updates:** Need to write HTMX code for dynamic behavior (vs Streamlit's auto-rerun)
- **Time:** Still 3 weeks of development work

#### Benefits

- **Better UI Control:** Full control over HTML/CSS
- **Standard Patterns:** Uses common web development patterns
- **Easier Extensions:** Add features like export, search, tags more easily
- **Better Routing:** Clean URLs (`/chat/`, `/memory/`, `/settings/`)
- **Professional Feel:** More like a "real" web app vs Streamlit's generic look
- **Still Simple:** No database, no auth, no deployment complexity
- **Easy Fork:** Users still just `git clone` and `python manage.py runserver`

**Status:** Planning phase - not starting until Streamlit version is feature-complete and tested.

---

## Testing Checklist

When making changes, test these scenarios:

### Basic Operations
- [ ] Create new chat session with personality selection
- [ ] Send messages and receive responses
- [ ] Switch between sessions
- [ ] Delete session with confirmation
- [ ] View different personalities in sidebar groups

### Personality System
- [ ] Collapse/expand personality groups
- [ ] Create chat with different personalities
- [ ] Verify personality persists across app restarts
- [ ] Change default personality in settings
- [ ] Add new personality folder and verify it appears

### Memory Management
- [ ] Navigate to User Memory view
- [ ] Update memory from all sessions
- [ ] Verify timestamp updates
- [ ] Wipe memory with confirmation
- [ ] Verify memory affects responses appropriately

### Edge Cases
- [ ] Empty session directory (first launch)
- [ ] Corrupted session JSON file
- [ ] Missing personality folder
- [ ] API key invalid
- [ ] Network timeout
- [ ] Empty API responses
- [ ] Very long messages
- [ ] Special characters in messages
- [ ] Rapid session switching

### UI/UX
- [ ] Current session highlighted in bold
- [ ] Session counts accurate in headers
- [ ] Delete button works from any group
- [ ] Navigation between views preserves state
- [ ] Collapse states persist during navigation
- [ ] Error messages display properly

---

## Quick Reference

### Important Functions

**webapp.py:**
- `get_sessions_with_titles()` - Load all sessions
- `group_sessions_by_personality()` - Group for sidebar
- `load_chat_core()` - Initialize chat with personality
- `create_new_chat_dialog()` - New chat modal
- `confirm_delete_dialog()` - Delete confirmation
- `aggregate_all_sessions_messages()` - Collect for memory update

**chat_core.py:**
- `send_message(user_input)` - Main chat method
- `_get_payload_messages()` - Build API payload
- `_save_history()` - Persist to JSON

**context_manager.py:**
- `load_context(personality_dir, ltm_file)` - Assemble system prompt
- `get_available_personalities()` - List valid personalities

**summarizer.py:**
- `generate_title(user_msg, assistant_msg)` - Create title
- `update_long_term_memory(messages, ltm_file)` - Update profile

### Important Constants

- `MAX_HISTORY`: 50 (= 100 messages in context)
- `DEFAULT_PERSONALITY`: "assistant" (or configurable)
- Session ID format: `session_YYYYMMDD_HHMMSS.json`

### API Endpoint

```
https://openrouter.ai/api/v1/chat/completions
```

### Useful Commands

```bash
# Start app
streamlit run webapp.py

# Clear browser cache (if UI behaves strangely)
# Settings → Clear Cache → Rerun

# View logs
# Terminal where streamlit is running

# Reset all data
rm -rf sessions/*.json long_term_memory.md
```

---

## Resources

- **OpenRouter API:** https://openrouter.ai/docs
- **Streamlit Docs:** https://docs.streamlit.io
- **Nord Theme:** https://www.nordtheme.com
- **Project Repo:** (Add GitHub URL when available)

---

**End of CLAUDE.md**
