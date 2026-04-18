# Liminal Salt

**v0.10.0**

A self-hosted LLM chatbot with persistent per-persona memory, customizable personas, and a roleplay mode. Runs locally; state lives in plain JSON and Markdown files under `data/`.

![Liminal Salt](docs/images/main-screenshot.png)

---

## Why Liminal Salt?

**For writers and roleplayers.** Build personas as Markdown, switch threads into roleplay mode with per-scene memory, and let each persona build up its own continuity across conversations.

**For privacy-conscious users.** Runs on your machine. No database, no cloud, no telemetry. All state is readable text on disk.

**For tinkerers.** Personas are Markdown. Themes are JSON. Django + HTMX + Alpine, easy to extend.

---

## Quick Start

```bash
git clone https://github.com/irvj/liminal-salt-app.git
cd liminal-salt-app
python run.py
```

The launcher creates a virtual environment and installs dependencies on first run. Open http://localhost:8420 and follow the setup wizard.

Requires Python 3.10+ and an [OpenRouter API key](https://openrouter.ai/).

---

## Features

- **Per-persona memory.** Each persona maintains its own evolving notes about you, merged in the background as you talk.
- **Roleplay mode.** Per-thread scenarios and scene-level memory, with persona memory suppressed in-scene for immersion. Fork any chat thread into a roleplay thread without losing context.
- **Context files.** Upload documents per-persona or globally; reference local directories to pull in live files without copying them.
- **Multi-session.** Sessions grouped by persona, pinnable, auto-titled, with drafts saved per thread.
- **OpenRouter.** Hundreds of models, with per-persona model overrides.
- **Themes.** Dark and light modes across 16 color themes.

---

## Screenshots

![Chat](docs/images/chat.png)
![User Memory](docs/images/user-memory.png)
![Persona Settings](docs/images/persona-settings.png)
![Settings](docs/images/settings.png)

---

## Roadmap

Development is usage-driven, not scheduled. Near-term work focuses on continued improvements to the memory and roleplay systems. Longer-term, the app is headed toward a standalone desktop build.

---

## License

MIT
