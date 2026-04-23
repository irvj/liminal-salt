# Liminal Salt

**v0.20.1**

A self-hosted LLM chatbot with persistent per-persona memory, customizable personas, and a roleplay mode. Runs locally; state lives in plain JSON and Markdown files under `data/`.

> **Pre-release beta.** A Rust (Axum) server you run locally in your browser. A standalone desktop build via Tauri is the next milestone. Internals and on-disk data formats may still change — back up anything you care about. If you want a stable install-and-forget app, wait for the desktop build.

![Liminal Salt](docs/images/main-screenshot.png)

---

## Why Liminal Salt?

**For writers and roleplayers.** Build personas as Markdown, switch threads into roleplay mode with per-scene memory, and let each persona build up its own continuity across conversations.

**For privacy-conscious users.** Runs on your machine. No database, no cloud, no telemetry. All state is readable text on disk.

**For tinkerers.** Personas are Markdown. Themes are JSON. Rust (Axum + Tera) + HTMX + Alpine, easy to extend.

---

## Quick Start

```bash
git clone https://github.com/irvj/liminal-salt.git
cd liminal-salt
cargo run -p liminal-salt
```

Open http://localhost:8420 and follow the setup wizard.

Requires [Rust](https://rustup.rs/) (stable) and an [OpenRouter API key](https://openrouter.ai/).

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

![Chat: Chatbot](docs/images/chat-chatbot.png)
![Chat: Roleplay](docs/images/chat-roleplay.png)
![Persona Memory](docs/images/persona-memory.png)
![Persona Settings](docs/images/persona-settings.png)
![Settings](docs/images/settings.png)

---

## Roadmap

Development is usage-driven, not scheduled. Near-term work focuses on continued improvements to the memory and roleplay systems. The next milestone is a Tauri desktop build so you can run Liminal Salt as a native window instead of in a browser tab.

---

## Scope

Liminal Salt is a local application, not a hosted service. You run it on your own machine with your own API key. Running it as a service for other people is outside the project's scope and would make you the operator of whatever you built.

---

## User Agreement

Using Liminal Salt means agreeing to the terms in [AGREEMENT.md](AGREEMENT.md) — short, plain-language, covers age, open source, non-determinism, provider terms, and responsibility for content submitted to and returned from the LLM. The app presents it once during setup.

---

## Development

Working on templates, Tailwind classes, or the vendored JS deps requires [Node.js](https://nodejs.org/):

```bash
npm install
npm run dev          # Tailwind watcher + cargo run, concurrent
```

Tests: `cargo test -p liminal-salt`. Conventions and the full dev workflow live in [CLAUDE.md](CLAUDE.md).

---

## License

MIT
