# Liminal Salt — Architecture Roadmap

**Updated:** 2026-04-22
**Status:** Milestone 1 (Python → Rust) complete at v0.20.0. Milestone 2 (Tauri desktop) is next.

---

## Where We Are

- Rust + Axum + Tera backend; HTMX + Alpine + Tailwind frontend. Single-process server on port 8420.
- Python/Django codebase removed. `cargo run -p liminal-salt` is the dev loop.
- 159+ integration + unit tests; clippy clean under `-D warnings`.
- [`CLAUDE.md`](../../CLAUDE.md) holds the architecture invariants, service ownership, and code standards.
- [`CHANGELOG.md`](../../CHANGELOG.md) holds the commit-level history (the full Python → Rust story is aggregated under `v0.20.0`).

Persistent state lives under `data/`. `config::data_dir()` resolves this path and is the single function that changes for M2.

---

# Milestone 2: Tauri Desktop App

Wrap the Rust backend in Tauri. Since M1 produced a Rust Axum server, Axum runs in-process — no child process, no bundled runtimes, no IPC bridge.

## Architecture

```
┌──────────────────────────────────────┐
│           Tauri App (single binary)  │
│                                      │
│  ┌────────────────────────────────┐  │
│  │  Axum Server (in-process)      │  │
│  │  All M1 services               │  │
│  │  Serves on 127.0.0.1:{port}    │  │
│  └────────────────────────────────┘  │
│                                      │
│  ┌────────────────────────────────┐  │
│  │  OS Native Webview             │  │
│  │  WebKit (macOS/Linux)          │  │
│  │  WebView2 (Windows)            │  │
│  │  Loads http://127.0.0.1:{port} │  │
│  └────────────────────────────────┘  │
└──────────────────────────────────────┘
```

## Implementation Scope

| Task | Details |
|------|---------|
| Scaffold | `cargo tauri init` adds `src-tauri/` as a workspace member. Configure `tauri.conf.json` (window size, title, icons, app ID `com.liminalsalt.app`). |
| Axum integration | Start Axum in Tauri's `setup` hook, bind `127.0.0.1:0` to pick a dynamic port, read back the actual port, pass to the window URL. |
| Window management | Single window → `http://127.0.0.1:{port}`. Disable dev tools in release. |
| Lifecycle | Axum task spawned on Tauri setup; abort on window-close event. Clean shutdown via the same `tokio::signal::ctrl_c` equivalent the CLI server uses. |
| Data directory | Swap `config::data_dir()` to return Tauri's `app_data_dir()`. This is the single seam M1 was designed around — no other path literal hard-codes the data root. |
| Asset embedding | Use `rust-embed` (or `include_dir!`) to ship: `crates/liminal-salt/templates/`, `crates/liminal-salt/static/` (including `static/vendor/htmx.min.js`, `static/vendor/alpinejs.min.js`, and 16 theme JSONs), `crates/liminal-salt/default_personas/`, and `AGREEMENT.md`. On first launch, seed defaults into `app_data_dir()`. |
| App icons | Generate `.icns` / `.ico` / `.png` via `cargo tauri icon`. |
| Build | `cargo tauri build` per target platform. |

## Data Directory

`app_data_dir()` resolves to:

| Platform | Path |
|----------|------|
| macOS | `~/Library/Application Support/com.liminalsalt.app/` |
| Windows | `C:\Users\<user>\AppData\Roaming\com.liminalsalt.app\` |
| Linux | `~/.local/share/com.liminalsalt.app/` |

Directory shape inside matches M1: `config.json`, `sessions/`, `personas/`, `memory/`, `user_context/`. Flat files, self-contained; the user can back up by copying the folder.

## Success Criteria

- App launches as a native window (no browser, no address bar).
- Single binary, no external dependencies.
- Binary size < 20MB.
- Native window controls.
- App icon in taskbar / dock.
- Clean shutdown (Axum stops on window close).
- Data persists in platform-appropriate location.
- All functionality identical to the browser M1 version.
- Builds for macOS, Windows, Linux.

---

## Known Hard Spots

Places where things can break in subtle ways. Consult this list when touching related code.

1. **Memory worker concurrency.** Don't hold a session-JSON lock across an LLM `.await`. The worker's "already running" mutex namespaces (`persona_locks`, `session_locks` in `memory_worker.rs`) are deliberately separate from `session::SESSION_LOCKS` — collapsing them reintroduces the lock-across-await bug. See CLAUDE.md for the structural enforcement pattern.
2. **Atomic writes.** All `data/` writes go through `services::fs::write_atomic` (write to `<path>.tmp`, fsync, rename). Lockless readers (sidebar, schedulers) rely on this — skipping the rename exposes a truncated-zero window that shows up as "EOF while parsing" on concurrent reads.
3. **Session cookie configuration.** `tower-sessions` with `with_secure(false)` is required for `http://localhost` — browsers silently drop `Secure`-flagged cookies on plain HTTP, which manifests as every POST getting a fresh session (with a new CSRF token) and 403-ing.
4. **HTMX partial shape drift.** Swap targets depend on specific element IDs and data attributes. A template edit that emits subtly different HTML (extra wrapper div, different ID) can break UX in ways that pass unit tests. Browser smoke test every HTMX interaction.
5. **HTMX 2 error-response semantics.** HTMX 2 doesn't swap on 4xx/5xx by default. Handlers that return plain-text errors on failure don't propagate anything to the user UI — the client-side error display path in `utils.js` fills that gap, but custom swap behavior on error requires explicit config.
6. **Orphan template URL attributes after refactors.** When renaming a route, the `data-*-url` attributes in templates and `getAppUrl` fallbacks in JS can go stale without breaking the build or tests. They're strings; the JS then fetches them and gets 404s. After any route rename, grep the new route literal across templates + JS and confirm old-path references got updated.
7. **Persona rename cascade** is best-effort, not transactional. A mid-cascade failure leaves partial state (directory renamed but memory file didn't). Log-and-continue is accepted; recovery is manual if it happens.
