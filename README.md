# Liminal Salt

**v0.1.3**

A multi-session LLM chatbot with persistent memory and customizable personalities. Connects to OpenRouter's API for LLM-powered conversations.

## Features

- **Multi-Session Management** - Create and switch between multiple chat sessions
- **Personality System** - Customizable AI personalities defined in Markdown files
- **Long-Term Memory** - Automatic user profile building across conversations
- **No Database Required** - All data stored in JSON and Markdown files

## Goals & Roadmap

**Goal:** Create an open, customizable chatbot for writers and roleplayers. This is a tool for people who want to create unique characters, talk with them, and bring out more depth and understanding in their creative work.

**Current State:** Fully usable as-is. Create and manage personalities directly from the web interface—no code editing required for basic usage.

**Roadmap:** Continuous, frequent releases focused on quality-of-life improvements, new settings, and features. Development is driven by necessity and interest rather than a rigid schedule, with the aim of rapidly improving the application. Future releases will also include unique, in-depth characters ready to chat with.

**Version:** Version number is getting manually bumped right now, as updates are swift and iterative, so it's not always accurate. By version 0.2.0 the application should be on a tagging system for better organization and version control.

## Requirements

- Python 3.10+
- [OpenRouter API key](https://openrouter.ai/)

## Setup

1. Clone the repository:
   ```bash
   git clone https://github.com/irvj/liminal-salt.git
   cd liminal-salt
   ```

2. Create and activate a virtual environment:
   ```bash
   python3 -m venv .venv
   source .venv/bin/activate
   ```

3. Install dependencies:
   ```bash
   pip install -r requirements.txt
   ```

## Running

```bash
python3 manage.py runserver
```

Open http://localhost:8000 in your browser.

## License

MIT
