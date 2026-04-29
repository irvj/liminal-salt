# Liminal Salt — Architecture Roadmap

**Updated:** 2026-04-29
**Status:** M1 (Python → Rust) complete at v0.20.1. M2 (multi-provider) shipped. M3 (user-editable prompts) shipped. Architecture is mature; this roadmap tracks the refactors and milestones needed for the architecture to become immaterial to further product work.

---

## Guiding Principle

> The architecture should get out of the way of the features.

Further product development focuses on four directions:

1. **Additional LLM providers** — the app is OpenRouter-only today; trait seam is partial.
2. **User-editable prompts** — prompt engineering is the actual conceit of the app; users should be able to tune it in-app.
3. **Tauri desktop distribution** — M1 was designed around this with `config::data_dir()` as the one-function seam.
4. **Frontend UX refinement**, especially settings pages — organized, clear, obvious to non-technical users.

Architectural work below is ordered to unblock those directions in the sequence that compounds best. Multi-provider and prompt externalization come before Tauri because they reshape service-layer boundaries; Tauri is plumbing once those boundaries are right. UX refinement is a cross-cutting thread, not a discrete milestone.

---

## Where We Are

- Rust + Axum + Tera backend; HTMX + Alpine + Tailwind frontend. Single-process server on port 8420.
- Python/Django codebase removed. `cargo run -p liminal-salt` is the dev loop.
- 176+ integration + unit tests; clippy clean under `-D warnings`.
- Architecture audit (2026-04-23): A on separation of concerns, locking discipline, file I/O durability, documentation; A− on Rust idioms and frontend discipline; polish items from that audit have landed (LlmClient construction out of handlers, scheduler parse warnings, per-error-type status mappers, inline-script removal). Remaining gap: B− on test coverage — services well covered; zero HTTP-layer tests.
- [`CLAUDE.md`](../../CLAUDE.md) holds architecture invariants, service ownership, and code standards. The invariants are load-bearing, not scar tissue — each exists because the alternative breaks under the workload (LLM calls are slow; flat-file storage has no transaction manager; multiple writers touch the same session document).
- [`CHANGELOG.md`](../../CHANGELOG.md) holds commit-level history.

Persistent state lives under `data/`. `config::data_dir()` resolves this path and is the single function that changes for Tauri.

---

## Milestone 2: Multi-provider LLM support — shipped

Landed across six commits (`134ee37` → `40bfbcc`). Chose an **enum-with-match-arm dispatch** over a trait because the set of providers is compile-time-known; this avoids `async_trait` / `trait_variant` dependencies and the dyn-compat limitations of async-fn-in-trait, while the exhaustive `match` on each method gives compile-time coverage when a new variant is added.

**What's in place:**

- `services/providers/mod.rs` — `Provider` enum, `ProviderChatLlm` wrapper impling `ChatLlm`, `ProviderMetadata` (UI-facing), `by_id()`, `metadata_list()`, `ALL` slice.
- `services/providers/openrouter/` — OpenRouter impl split into `catalog.rs` (key validation + model list) and `chat.rs` (`LlmClient` for `/v1/chat/completions`).
- `AppConfig` — provider-neutral fields (`provider`, `api_key`). No migration code (no users to migrate).
- Handlers (`chat`, `api`, `settings`, `setup`) dispatch through `Provider` methods; memory worker's `build_memory_llm` / `build_thread_memory_llm` return `ProviderChatLlm`.
- `handlers/settings.rs`'s "unknown provider" gate now uses `providers::by_id(...).is_none()` — rejects anything not registered rather than hardcoding `"openrouter"`.

**Remaining (future) — not scoped to this milestone:**

- **Anthropic impl** (`providers/anthropic/`). `/v1/messages` request shape differs from `/v1/chat/completions` (top-level `system`, different auth header); the reshape is confined to the `ChatLlm` impl and a new `Provider::Anthropic` variant. No other code changes.
- **OpenAI impl** (`providers/openai/`). Almost a copy of the OpenRouter chat path minus the attribution headers.
- **Setup-wizard UI** for the provider picker (currently renders a picker with OpenRouter as the only option — intentional, since no other providers are registered).
- **Pricing-format policy** for providers that don't expose a pricing endpoint (OpenAI, Anthropic). Empty string is fine short-term.

Adding a provider = new module under `services/providers/<name>/`, enum variant + match arms on `Provider` methods, `ProviderChatLlm` variant, `by_id` entry. The compiler enforces coverage; no other parts of the codebase change.

---

## Milestone 3: User-editable prompts — shipped

Landed across three commits (`85c3493 → fe547b4`) on the `user-editable-prompts` branch. Bundled default prompts (the LLM-facing instructional prose for every memory operation) are now copied into `data/prompts/` on first boot and editable from `/prompts/` in-app. Designed for non-technical users — no variables, no templating, no placeholders in the user-facing surface.

**What's in place:**

- `services/prompts.rs` — owns `data/prompts/**`. Compile-time `PROMPTS` registry of editable prompts; public API: `list`, `load`, `load_default`, `save`, `reset`, `seed_default_prompts`. ID validation is closed-set (only registered IDs are accepted), so handlers can pass POST input straight through.
- `crates/liminal-salt/default_prompts/*.md` — bundled defaults, plain prose, no variables. Source of truth for "reset to default." Embedded for M4 alongside `default_personas/`.
- `data/prompts/{id}.md` — user-editable copies. Seeded on first boot; existing files are never overwritten by seeding (so newly-added prompts in future updates seed cleanly without clobbering user edits).
- **Envelope pattern** in `memory.rs` and `thread_memory.rs`: the `format!(...)` prompt-builder wraps user-editable instructions loaded via `prompts::load(...)`. Persona name, identity, conversation data, existing memory, conditional persona-memory section, and size target stay in the envelope. The user only sees and edits the instructions block; they never see or need to know about the wrapper or the variables it owns.
- Editor page at `/prompts/`: collapsible per-prompt sections with textarea + Save + Reset + View Default. Save and Reset are AJAX (no full-page swap) so editing one prompt doesn't blow away unsaved edits in another. Reset uses the global `confirmModal` for the misclick guard.
- `MergeRequest` struct on `thread_memory::merge` — bundled the 9 args into one named-field request; tests use struct update syntax with a `default_req` helper.
- 17 new tests across `tests/prompts.rs` and `tests/template_render.rs`; existing memory + thread-memory tests adapted to the new prompts-load path.

**Scope shifts from the original design (deliberate):**

- **7 prompts → 5.** `thread_memory_seed` was specced as a separate prompt but the inline code never had distinct seed instructions — both seed and merge use the same prompt with a different `existing_block` placeholder. Collapsed the registry to reflect what's actually editable. `title_summarizer` was dropped from the user-editable surface entirely: it's a system-controlled fallback, not a tuneable knob, and exposing it would let a user break title generation with no recourse short of a code edit.
- **Chatbot's "DO NOT merge pre-existing knowledge" rule was conditional** in the inline code (only included when persona memory was non-empty). Rephrased to be unconditional in the `.md` so a single editable instructions block covers both cases without templating; the conditional persona-memory **data section** in the envelope still gates whether that data appears.
- **`size_instruction` placement in `memory.rs`** shifted from mid-instructions (between FORMAT and CRITICAL PERSPECTIVE CHECK) to envelope-suffix (right before "Return ONLY…"). Functionally a small downward shift; recency-bias preserved or strengthened. `thread_memory.rs` was already at the end, so byte-equivalent there.

**Remaining (future) — not scoped to this milestone:**

- **Per-persona prompt overrides** — power-user feature; adds a resolver cascade (per-persona → global → bundled) not needed in v1.
- **User-facing variables / templating** — target audience is non-technical. All interpolation stays app-side.
- **"Default has changed" diff indicator** — copy-if-missing semantics make it a non-issue until we actually ship a default-prompt update, and even then a manual "view default" comparison is enough.

Adding a new editable prompt = adding a row to `PROMPTS` in `prompts.rs` + shipping the `default_prompts/{id}.md` file + extracting whatever inline instructions the consuming service had. Seeding handles the rest on first boot.

---

## Handler test harness + LLM client tests

Currently zero HTTP-layer tests. As UX churn accelerates (M3 prompt editor, post-M4 settings reorganization), handler tests catch CSRF / form-parsing / error-mapping regressions cheaply. Can run in parallel with M2/M3; depends only on the current-state handlers.

- Add a test module using `tower::ServiceExt::oneshot` against the Axum `Router`.
- Cover critical POST paths: `/chat/send/`, `/memory/update/`, `/session/fork-to-roleplay/`, CSRF token round-trip, multipart upload (`/settings/context/upload/`).
- Target: ~10–15 tests; enough to catch error-mapping drift and middleware regressions, not enough to duplicate service-layer coverage.
- Add `llm.rs` unit tests using `wiremock` — retry, timeout, bad-status, and JSON-parse-error paths are today untested. A network-regression here would be invisible until production.
- Add minimal `config.rs` / `openrouter.rs` / `summarizer.rs` unit coverage (currently zero).

---

## Milestone 4: Tauri Desktop App

Wrap the Rust backend in Tauri. Axum runs in-process — no child process, no bundled runtimes, no IPC bridge. This is plumbing once M2 and M3 have landed; all the service-layer shape should be right by this point.

### Architecture

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

### Implementation scope

| Task | Details |
|------|---------|
| Scaffold | `cargo tauri init` adds `src-tauri/` as a workspace member. Configure `tauri.conf.json` (window size, title, icons, app ID `com.liminalsalt.app`). |
| Axum integration | Start Axum in Tauri's `setup` hook, bind `127.0.0.1:0` to pick a dynamic port, read back the actual port, pass to the window URL. |
| Window management | Single window → `http://127.0.0.1:{port}`. Disable dev tools in release. |
| Lifecycle | Axum task spawned on Tauri setup; abort on window-close event. Clean shutdown via the same `tokio::signal::ctrl_c` equivalent the CLI server uses. |
| Data directory | Swap `config::data_dir()` to return Tauri's `app_data_dir()`. This is the single seam M1 was designed around — no other path literal hard-codes the data root. |
| Asset embedding | Use `rust-embed` (or `include_dir!`) to ship: `crates/liminal-salt/templates/`, `crates/liminal-salt/static/` (including `static/vendor/htmx.min.js`, `static/vendor/alpinejs.min.js`, and the 16 theme JSONs), `crates/liminal-salt/default_personas/`, `crates/liminal-salt/default_prompts/` (new from M3), and `AGREEMENT.md`. On first launch, seed defaults into `app_data_dir()`. |
| App icons | Generate `.icns` / `.ico` / `.png` via `cargo tauri icon`. |
| Build | `cargo tauri build` per target platform. |

### Open questions to resolve during M4

- **Session store persistence.** Current `tower-sessions` `MemoryStore` loses state on restart — fine for a dev server, awkward in a desktop app. Decide: persist session state (file-backed store), or accept session reset on app restart (simpler; the only real cost is re-selecting the current chat).
- **CSRF in a same-origin Tauri context.** Keep it (simpler than conditionally disabling), or make it conditional on "running outside Tauri." Default to keeping it — the overhead is negligible and the code is already there.

### Data directory

`app_data_dir()` resolves to:

| Platform | Path |
|----------|------|
| macOS | `~/Library/Application Support/com.liminalsalt.app/` |
| Windows | `C:\Users\<user>\AppData\Roaming\com.liminalsalt.app\` |
| Linux | `~/.local/share/com.liminalsalt.app/` |

Directory shape inside matches M1 plus M3's additions: `config.json`, `sessions/`, `personas/`, `memory/`, `user_context/`, `prompts/`. Flat files, self-contained; the user can back up by copying the folder.

### Success criteria

- App launches as a native window (no browser, no address bar).
- Single binary, no external dependencies.
- Binary size < 20MB.
- Native window controls; app icon in taskbar / dock.
- Clean shutdown (Axum stops on window close).
- Data persists in platform-appropriate location.
- All functionality identical to the browser M1 version.
- Builds for macOS, Windows, Linux.

---

## Ongoing: UX refinement

Not a milestone — a thread that runs through M2, M3, and M4 and beyond. Driven by actual UI design, not speculation.

- **Component primitives emerge from real surfaces.** M2's provider picker, M3's prompt editor, and M4's settings reorganization all surface common needs: labeled setting rows, collapsible sections, inline validation states, help tooltips, confirm modals. Build these as reusable Alpine components as they're needed — don't spec a library up front.
- **Settings page reorganization** is the main non-techie surface. Group by intent ("conversation behavior," "memory," "appearance") rather than by implementation ("thread memory settings," "persona defaults"). Hide advanced controls behind disclosure. The "Custom" badge / resolver cascade today works correctly but is visually noisy for casual users — reduce the UI real estate it occupies.
- **Frontend discipline rules in CLAUDE.md still apply.** No inline handlers, no inline styles, no business logic in templates. Alpine components registered in `alpine:init`. The two existing violations in `memory_main.html` get cleaned up in the Immediate polish work above.

---

## Ongoing: Prompt engineering

Enabled by M3. Iterate on `default_prompts/*.md` bundled defaults; users' overrides are preserved. No architecture change required — this is the product work the architecture exists to serve.

---

## What we're deliberately NOT doing

Explicit non-goals. Preventing drift toward nice-to-have refactors that don't earn their keep.

- **Database migration.** Flat-files under `data/` are a feature (users back up by copying a folder; no migrations to ship across app versions). The invariants this forces — atomic writes, RMW preserving unknown fields — are cheap and already correct.
- **Per-session actor pattern.** Current per-session `TokioMutex` with "no lock held across LLM `.await`" is correct and well-documented. Actor lifecycle management (spawn on demand, idle timeout, supervisor for cross-session ops) would be *more* complexity, not less.
- **`SessionId` newtype refactor.** Genuinely a minor improvement over the "validate at every entry point" pattern, but not worth the churn unless in `session.rs` for another reason.
- **Trait-object `AppState` for compile-time handler thinness.** Would hurt test ergonomics more than it helps at this size (tests currently call service functions directly, which is a feature).
- **Splitting `memory_worker.rs`.** It's 1,218 lines but cohesive. Splitting would create cross-file coordination seams worse than the file size.
- **Per-persona prompt overrides.** Deferred from M3. Power-user feature; adds a resolver cascade (per-persona → global → bundled) not needed in v1.
- **User-facing variable templating in prompts.** Target audience is non-technical. The app owns all substitution; the user edits pure prose.
- **Additional LLM providers beyond the M2 proof-of-boundary until M2 has landed.** Adding two providers before the trait is settled means doing the refactor three times instead of once.

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
8. **Prompt envelope compatibility (M3 onward).** When a prompt-using service changes what structural values it injects (adds a new field to the envelope, removes one), the user's saved prompt doesn't break — it's pure prose, independent of the envelope. But the *bundled default* should be updated to reflect any new context the envelope exposes, or the LLM won't be instructed how to use it. Keep `default_prompts/*.md` in sync with service-side envelope changes.
