# Liminal Salt

**v0.1.0**

A multi-session LLM chatbot with persistent memory and customizable personalities. Connects to OpenRouter's API for LLM-powered conversations.

## Features

- **Multi-Session Management** - Create and switch between multiple chat sessions
- **Personality System** - Customizable AI personalities defined in Markdown files
- **Long-Term Memory** - Automatic user profile building across conversations
- **No Database Required** - All data stored in JSON and Markdown files

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

## Adding Personalities

Create a new folder in `data/personalities/` with an `identity.md` file:

```
data/personalities/mybot/
└── identity.md
```

The personality will appear in the dropdown when creating new chats.

## License

MIT
