# Audit Plan — Thread Memory & Roleplay Mode Feature

**Purpose:** a systematic audit of the thread-memory + roleplay-mode feature set shipped in phases 1–5. Intended for a fresh session to review the code with clear eyes, look for bugs, logical issues, rough edges, and design inconsistencies.

**Context:** phases 1–5 were planned in `planning_thread_memory_roleplay.md` (now deleted as the plan is complete). Phase 6 (control-panel consolidation) was intentionally skipped — the header-icon pattern settled well enough that forcing everything behind one modal felt like a regression.

---

## What was built

High-level summary of the feature surface. Detailed file lists below.

### Data model

- **Session JSON** gained: `mode` (`"chatbot"` | `"roleplay"`), `scenario` (string), `thread_memory` (string), `thread_memory_updated_at` (ISO timestamp), `thread_memory_settings` (object: `interval_minutes`, `message_floor`, `size_limit`).
- **Persona config** (`data/personas/{name}/config.json`) gained: `default_mode` (only `"roleplay"` persisted — chatbot is the baseline, see below), `default_thread_memory_settings` (object), `auto_memory_message_floor` (flat key, backported to persona memory).

### System prompt assembly

`context_manager.load_context()` now assembles, in order:
1. Persona identity `.md` files
2. Persona-scoped context files
3. Global user context files
4. **Scenario** (per-thread, roleplay only — chatbot-mode threads silently ignore any scenario field)
5. **Thread memory** (per-thread running summary)
6. Persona memory

### Thread mode

- Set at thread creation via the home-page picker. Immutable afterwards.
- Roleplay threads:
  - Are excluded from persona-memory aggregation (`aggregate_all_sessions_messages`).
  - Use a roleplay-flavored prompt in the thread-memory summarizer (third-person narrative prose, past tense).
  - Show a scenario editor in the header and a drama-mask icon in the sidebar.
- Chatbot threads:
  - Contribute to persona memory normally.
  - Use a neutral third-person thread-memory prompt (flagged below as a design inconsistency).
  - Show a "fork to roleplay" drama icon in the header.

### Fork to roleplay

- One-click, non-destructive. Creates a new session with `mode=roleplay`, same persona, duplicated messages, **thread memory copied intact**, title/draft/pinned reset.
- Does not edit any memory (neither persona nor thread).
- Not available on roleplay threads. Not supported as roleplay → roleplay.

### Auto-update schedulers (unified shape)

Two parallel daemon threads in `memory_worker.py`:
- **Persona memory scheduler** fires when `interval_minutes` elapsed AND new messages ≥ `auto_memory_message_floor`. `interval = 0` disables. Default floor `10`.
- **Thread memory scheduler** fires when `interval_minutes` elapsed AND unsummarized messages ≥ `message_floor`. `interval = 0` disables. Global fallback disables auto by default.

Both use per-target locks and next-fire bookkeeping. Thread-memory scheduler prunes entries for deleted sessions and dispatches updates asynchronously (so a slow LLM call doesn't block checks for other sessions).

### Thread memory settings (per-thread override → persona default → global fallback)

- Resolver `resolve_thread_memory_settings(session_data, persona_config)` in `thread_memory_manager.py`.
- Per-thread override UI in the thread-memory modal (inline form at the top of the modal).
- Per-persona default UI on the persona-settings page ("Thread Defaults" section).
- Scheduler re-anchors next-fire time on save/reset (`reschedule_thread_next_fire`) so a changed interval takes effect at `now + new_interval` rather than waiting out the old one or firing immediately.

### Home-page mode picker

- Dropdown: Chatbot / Roleplay. Chatbot is the baseline.
- Persona's `default_mode = "roleplay"` forces the picker to Roleplay when that persona is selected.
- `default_mode = "chatbot"` is NOT persisted (treated as the implicit baseline).
- Roleplay-mode threads created from home can ship an initial scenario set via a Scenario button on the home page (localStorage-backed before submit, posted with the form).

### Sidebar

- Drama-mask icon inline after the title for roleplay threads (tooltip "Roleplay thread"). Included in both pinned and grouped sections.

---

## Files touched

Main surface. There are more small edits — use `git log --name-only` on the phase commits for the complete list.

### Services
- `chat/services/session_manager.py` — `create_session(mode)`, `save_scenario`, `save_thread_memory`, `save_thread_memory_settings_override`, `reset_thread_memory_settings_override`, `fork_session_to_roleplay`
- `chat/services/thread_memory_manager.py` — new file: `ThreadMemoryManager` class (chatbot + roleplay prompt variants), `filter_new_messages`, `resolve_thread_memory_settings`, `resolve_persona_thread_memory_defaults`, constants
- `chat/services/memory_worker.py` — thread-memory auto-update loop, per-session locks/status, `_count_new_messages_for_persona` (5a backport), `reschedule_thread_next_fire`
- `chat/services/memory_manager.py` — unchanged in structure; message-floor check lives in the worker
- `chat/services/context_manager.py` — `load_context(scenario, thread_memory)`, `get_persona_default_mode`
- `chat/services/chat_core.py` — read-modify-write in `_save_history` so non-core session fields survive (critical bug fix discovered during phase 3)

### Views
- `chat/views/chat.py` — many context splats; new views: `save_scenario`, `update_thread_memory`, `thread_memory_status`, `fork_to_roleplay`, `save_thread_memory_settings`, `reset_thread_memory_settings`; helpers `_build_chat_core` (mode-gated scenario), `_thread_memory_settings_context`, `_build_persona_mode_map` (roleplay-only)
- `chat/views/personas.py` — `_persona_defaults_context`, `save_persona_thread_defaults`, `clear_persona_thread_defaults`, `_persona_defaults_response`
- `chat/views/memory.py` — `auto_memory_message_floor` wired through (5a)

### URLs
- `chat/urls.py` — new routes under `/session/` and `/settings/`

### Templates
- `chat/templates/chat/chat_main.html` — header buttons (fork/scenario/thread memory), mode indicator on persona line, hidden data divs, scenario/thread-memory data sources
- `chat/templates/chat/chat_home.html` — mode picker, scenario button, hidden scenario data div
- `chat/templates/chat/chat.html` — scenario modal, thread-memory modal
- `chat/templates/chat/sidebar_sessions.html` — drama-mask icon inline after title for roleplay threads
- `chat/templates/memory/memory_main.html` — `Message Floor` field in persona memory settings (5a)
- `chat/templates/persona/persona_main.html` — Thread Defaults section
- `chat/templates/icons/drama.html` — new Lucide icon

### Static
- `chat/static/js/components.js` — Alpine components: `scenarioModal`, `threadMemoryModal`, `personaThreadDefaults`, plus extensions to `homePersonaPicker` and `memorySettings`
- `chat/static/js/utils.js` — `clearNewChatScenario`
- `chat/static/css/input.css` — new `--max-width-modal-2xl: 900px`

---

## Known rough edges (deferred)

These were noted during implementation and explicitly deferred for the audit to address.

### 1. Chatbot thread memory perspective (design inconsistency)

`ThreadMemoryManager._build_chatbot_prompt` produces third-person summaries ("the user discussed X with {persona}"). Persona memory is written from the persona's point of view ("you've noticed he tends to…"). Inconsistent voices across the memory system.

**Proposal:** rewrite the chatbot thread-memory prompt to match persona memory's voice — second person for the persona, third person for the user, written as inner-monologue continuity. The roleplay variant stays third-person narrative (intentional).

**Impact:** existing chatbot thread memories are in the old voice; would optionally regenerate.

### 2. "Override always created on save" (both 5c and 5d)

On the thread-memory settings form and the persona thread-defaults form, clicking Save writes the full override even if the values equal the defaults. No functional breakage, but the "Custom override" indicator flips on even when values are identical to upstream. Worth tightening so Save only creates an override when values actually diverge.

### 3. Missing help text after 5c UI compacting

When the thread-memory modal was widened and the settings moved to the top, the "Interval 0 = disabled / Size Limit 0 = unlimited" help paragraph was cut. A user seeing `0` in a field may not know what it means.

### 4. Fork doesn't copy `thread_memory_settings`

Open design question. `fork_session_to_roleplay` copies `thread_memory` and `thread_memory_updated_at` but not the per-thread settings override. Arguments both ways:
- Copy: user's tuning on the source thread likely carries intent.
- Don't copy: fork is a new thread, different mode, different cadence.

Decide and document.

### 5. Empty input NaN handling

On both settings forms (thread memory + persona defaults), clearing a number field and hitting Save produces a `parseInt('')` → `NaN` → `"NaN"` → backend `ValueError` → generic 400. Browsers block letter input via `type=number` but allow empty. Minor but ugly; could either prevent empty inputs in the frontend or map empty → "use current value" (no change).

### 6. Modal doesn't refresh during background auto-update

The thread-memory modal polls for status during manual updates, and refreshes content on open via `_checkStatusOnce`. But if the modal is **open** when a background auto-update fires and completes, the modal content stays stale until the user closes and reopens it. Full fix: continuous polling while modal is open (every ~15s) checks for `updated_at` changes and refreshes.

### 7. Interval clamping discrepancy

Backend clamps interval values 1–4 to 5 (minimum enforced). Frontend accepts 1–4 without warning. User types 3, saves, sees it silently flip to 5 on return. Either document in help text or add frontend validation matching the backend.

### 8. Scheduler tick cost for many sessions

`_thread_memory_auto_update_loop` iterates all `data/sessions/*.json` on every tick and parses each one. For users with hundreds of sessions, this is O(sessions) per tick. The persona-memory scheduler uses an mtime cache (`_session_cache`); the thread-memory scheduler does not. Consider mirroring the cache pattern.

---

## Open questions (deferred from plan)

### Persona memory in roleplay-mode threads

Should persona memory appear in the system prompt for roleplay threads, or be suppressed for immersion? Currently it's included. Arguments:
- **Include (current):** persona still "knows" the real user (enjoys Alice knowing she's talking to Joseph while playing a character).
- **Suppress:** immersion — the fictional persona shouldn't know biographical real-user facts mid-scene.

Could be per-thread toggle, persona toggle, or hardcoded. Worth user-testing before deciding.

---

## Audit focus areas

Things the audit session should systematically investigate beyond the deferred items above.

### Correctness

- **Race conditions** in the two schedulers and the manual/auto interaction (per-persona lock, per-session lock).
- **File I/O atomicity.** `session_manager._write_session` uses `flush + fsync`. Verify all new write paths either go through it or match the pattern.
- **Message timestamp filtering** in `filter_new_messages` and `_count_new_messages_for_persona` — ISO string comparison works only for same-precision UTC. Any drift in timestamp format across write paths would break it silently. Inventory every write of a message timestamp.
- **Error paths**: LLM failures, corrupted session JSON, missing persona config, concurrent writes during a restart. The failure branches exist; check they're actually reachable and produce sensible status.

### Safety

- **XSS in user-provided text.** Scenario and thread memory are rendered into the system prompt (LLM side, not user-visible) but also displayed in the modal (HTML). Confirm every user-text rendering goes through proper escaping (`|escape`, `textContent`, etc.).
- **Path traversal.** Session IDs are server-generated timestamps, but endpoints take them from POST. Verify `get_session_path(session_id)` can't be abused with `../` or absolute paths.
- **CSRF** on every new POST endpoint. All our new ones require it (Django default), but verify none accidentally bypass it.

### Behavioral edge cases

- Forking an empty thread (no messages). Does the fork succeed? Does it matter?
- Forking while an auto-update is in progress on the source.
- Deleting a persona that has sessions with `default_thread_memory_settings`. Persona-rename cascade seems handled by `update_persona_across_sessions` — verify the delete side.
- Restarting the server with both schedulers running and an update in flight.
- A session file with `mode=roleplay` but missing other roleplay fields (corrupted) — does it break anything?
- Old session files without any new fields (pre–phase-1) — do they still render, send, and update cleanly? (Should default to chatbot mode, no scenario, no thread memory.)

### UI / UX

- Visit every new surface on a fresh page load to confirm there's no flash of unstyled or pre-populated state.
- Test with sessions of different sizes (1 message, 100+ messages). Thread memory modal should handle both.
- Keyboard accessibility — tab order, enter-to-save, escape-to-close — matches existing patterns?
- All new icons (`drama.html`) render correctly in both light and dark mode.

### Performance

- Thread-memory scheduler I/O cost (see rough edge #8).
- Memory usage of `_thread_next_fire_time` and `_session_locks` over long runs.
- Frontend: the thread-memory modal's polling loop (every 2s during manual updates). Does it clean up properly on modal close?

---

## Fix-it priorities (suggested)

If the audit surfaces all/most of the above, a reasonable ordering to address:

1. **Chatbot thread memory perspective** (#1) — the most user-visible inconsistency.
2. **Scheduler cache for thread memory** (#8) — scalability cleanup before user base grows.
3. **Override-always-created** (#2) — data hygiene for a commonly used flow.
4. **Missing help text** (#3) — quick UX win.
5. **Modal refresh during background auto-update** (#6) — meaningful but scoped.
6. Smaller items (#4, #5, #7) as pickups.

The persona-memory-in-roleplay open question is product-design, not an audit bug — surface it but don't block on it.
