# Thread Memory & Roleplay Mode — Implementation Plan

**Status:** phases 1–4 shipped; phase 5 in progress
**Last updated:** 2026-04-18

## Goals

Two supported use modes:

1. **Chatbot** — conversational, cross-thread continuity. The current persona memory system works well here.
2. **Roleplay** — scenario-driven, single-thread coherence, no contamination of persona memory with fictional events.

## Problems being solved

- Initial scenario prompt scrolls out of context in long roleplay threads once `context_history_limit * 2` messages accumulate.
- No per-thread state tracking for how the scene/story has evolved.
- Persona memory conflates real-user facts with fictional events in roleplay threads. The current `ROLEPLAY AWARENESS` block in `_merge_memory` is a soft heuristic.
- No per-thread configuration surface — mode, memory cadence, scenario, etc. all have to live elsewhere.

## Core design

### New layers in the system prompt

Current assembly order (persona-scoped only):
1. Persona identity
2. Persona-scoped context files
3. Global context files
4. Persona memory

New assembly order (adds a thread layer between persona context and persona memory):
1. Persona identity
2. Persona-scoped context files
3. Global context files
4. **NEW: Scenario** — static, per-session, user-provided (uploaded file or inline text). Never scrolls out.
5. **NEW: Thread memory** — dynamic, per-session, LLM-maintained summary of what has happened in this thread.
6. Persona memory (existing; excluded or filtered for roleplay threads — see below)

### Per-thread mode

New `mode` field on the session JSON. Values: `chatbot` (default) | `roleplay`.

**Mode is immutable.** It is chosen at thread creation and cannot be changed. No toggle in the control panel. Rationale: allowing mid-thread flips creates the flip-back pollution scenario (chat → roleplay → chat re-exposes fictional messages to persona memory aggregation) with no clean resolution. The two modes are distinct products rather than two states of one feature; a thread's identity is its mode.

Mode-aware behaviors:
- Summarizer uses a mode-specific prompt variant. Chatbot mode preserves decisions and facts; roleplay mode preserves scene state, character positions, and emotional beats.
- Roleplay threads are excluded from cross-persona memory aggregation (retires the `ROLEPLAY AWARENESS` heuristic in `_merge_memory`).
- Open question: does roleplay mode also suppress persona memory in the system prompt for immersion, or keep it for persona-knows-the-user continuity?

### Scope principle: persona vs. scenario

- **Persona level** holds character/world continuity — identity, worldbuilding, voice, cross-thread character facts. Lives in persona identity and persona-scoped context files.
- **Scenario (thread level)** holds one-story continuity — the specific setup for this thread only. References the persona-level material but does not duplicate it.
- **Thread memory (thread level)** tracks what has happened *in this story*.

This separation is what makes immutable modes workable: starting a fresh roleplay with the same persona is cheap because the character definition is already durable at the persona level. Only the scenario is new.

### Fork to roleplay thread

Available on chatbot-mode threads only. Roleplay threads have no fork action.

**Fork does not edit memory.** Neither persona memory nor thread memory is modified by the fork action. Auto-updates at both levels remain valid elsewhere (configured by the user in phase 5); fork itself is strictly a new-session creation that copies memory state forward. The user can manually clear, regenerate, or purge afterwards if they want.

Behavior:
- Creates a new session with `mode=roleplay`, same persona, duplicated message history.
- **Thread memory (`thread_memory` and `thread_memory_updated_at`) is copied over intact.** Users can regenerate it under the roleplay prompt from the thread memory modal if they want a scene-shaped summary.
- Draft cleared. Pinned state reset. Title reset (summarizer will regenerate).
- Scenario starts empty (chatbot-mode sources have no scenario to copy); user fills it in from the control panel.
- Original thread is untouched and continues contributing to persona memory normally.
- **Persona memory is not touched** by the fork action.

Forking a roleplay thread into another roleplay thread is **not supported** — it would carry story A's message history into story B's scenario, which is exactly the kind of narrative pollution the scope principle rejects. A fresh roleplay thread is the clean path.

### Persona memory contamination — existing defenses

No dedicated "purge this thread from persona memory" action. See the rejected-features section at the bottom for the reasoning. The existing defenses cover the real use cases:

- **Roleplay mode filter** (phase 3) keeps fictional content out of persona memory automatically by excluding roleplay sessions from `aggregate_all_sessions_messages`. This is the primary defense and handles the most common contamination scenario.
- **`modify_memory_with_command`** (existing) — the user can issue a natural-language correction ("forget what I said about X"). Surgical, user-described, scoped to what the user actually wants removed.
- **Wipe memory** (existing) — the nuclear option when the user wants a clean slate.

### Unified auto-memory trigger shape

Both persona memory and thread memory auto-updates use the same trigger logic: run when **(time since last update ≥ `interval_minutes`) AND (new message count since last update ≥ `message_floor`)**. Either condition alone is insufficient. Setting `interval_minutes = 0` disables the auto-trigger at that level (user can still invoke updates manually).

This unification happens as part of phase 5: persona memory currently uses interval-only with "any new activity" as the message threshold. Phase 5 backports the explicit message floor to persona memory so both pipelines have identical shape and predictable behavior.

**Why message floor:** prevents wasteful LLM calls on trivial updates (a single new message triggering a full memory regeneration). The floor says "accumulate at least N messages of activity before it's worth burning a summarization call."

Both run in background daemon threads via the existing `MemoryWorker` pattern (per-target locks, status polling). Neither blocks the send path.

## Data model changes

### Session JSON (`data/sessions/session_*.json`)

New fields:
- `mode`: `"chatbot"` | `"roleplay"`
- `scenario`: `{ "type": "inline" | "file", "content": "..." }`
- `thread_memory`: string (LLM-maintained summary)
- `thread_memory_updated_at`: ISO timestamp of the last thread memory update
- `thread_memory_settings`: object — overrides for `interval_minutes`, `message_floor`, `size_limit`. Inherits from persona defaults when absent.

### Per-persona config (`data/personas/{name}/config.json`)

New keys:
- `default_mode`: `"chatbot"` | `"roleplay"` — writeable via UI in phase 5d; honored by the home-page mode picker.
- `default_thread_memory_settings`: object with `interval_minutes`, `message_floor`, `size_limit`. Used when a session has no per-thread override.
- `auto_memory_message_floor`: int (flat key, matching the existing `auto_memory_interval` convention). Backport from phase 5a. Minimum number of new messages across the persona's non-roleplay sessions before an auto-update fires.

### Naming conventions

- **Persona config keys stay flat** (`auto_memory_interval`, `auto_memory_message_floor`, `memory_size_limit`) — no migration of existing files.
- **Session thread-memory keys nested** under `thread_memory_settings: { interval_minutes, message_floor, size_limit }` — keeps session JSON tidy and avoids polluting the top-level namespace.

### Scenario storage

Recommend starting with **inline content** in the session JSON. Add file upload later if users want to reuse long scenarios across threads. A persona-level "scenario template" could eventually feed into new sessions of that persona.

## Service-layer changes

- Extend `context_manager.load_context()` to accept session data and inject the scenario + thread memory layers.
- New `ThreadMemoryManager` (or extend `MemoryManager`) — mode-aware merge prompts, per-session memory I/O.
- Extend `MemoryWorker` with per-session locks, a separate scheduler path for thread-memory auto-updates, and a status channel.
- New helpers in `session_manager`: save scenario, save thread memory, fork-to-roleplay (duplicate session with `mode=roleplay`; thread memory copied; draft/title/pinned reset).
- `aggregate_all_sessions_messages` (in `utils.py`) filters out sessions whose mode is `roleplay` when building cross-persona memory.
- **Phase 5a backport:** extend persona-memory auto-update (`memory_worker._auto_update_loop`) with a message-floor gate. Fire only when interval elapsed AND new message count across the persona's non-roleplay sessions ≥ `auto_memory_message_floor`. Reference point for "new" = persona memory file mtime.
- **Phase 5b additions:** new `thread_memory_worker` module (or new section of `memory_worker.py`) with a parallel daemon loop iterating sessions instead of personas. Settings resolver that merges per-thread override → persona default → global fallback.

## New URL routes (draft)

```
/session/scenario/save/
/session/scenario/upload/
/session/thread-memory/update/
/session/thread-memory/regenerate/
/session/thread-memory/status/
/session/fork-to-roleplay/
/session/settings/save/
```

Mode is chosen at creation through the existing new-chat flow; no separate toggle route.

## UI changes

### New chat flow

Mode is selected when creating a new thread, alongside persona. Chatbot is the default; roleplay is the alternative choice. Once created, the thread's mode is fixed.

### Thread control panel

New button in the chat header (next to title). Opens a modal or side panel. Contents depend on mode.

**Always visible:**
- Mode indicator (read-only).
- Thread memory view (read-only) with **Update now** and **Regenerate** buttons and a status indicator.
- Thread memory settings — interval, message floor, size limit; inherits from persona defaults.

**Roleplay threads also show:**
- Scenario editor (textarea, with file upload in a later phase).

**Chatbot threads also show:**
- **Fork to roleplay thread** action.

### Persona settings

- Default mode picker for new threads with this persona (writes `default_mode`).
- Default thread memory settings (`default_thread_memory_settings`: interval, message floor, size limit).
- The existing persona memory settings section gains a new field for `auto_memory_message_floor` (phase 5a backport).

## Implementation phases

Each phase is independently shippable.

1. **Scenario layer** — session JSON field + scenario editor in thread header + inject into system prompt. Biggest immediate win, lowest cost. (Scenario is only meaningful for roleplay-mode threads; early phases can ship with mode hard-defaulted to `chatbot` and the scenario UI hidden until phase 3.)
2. **Thread memory, chatbot mode only** — summarizer, background worker, inject into system prompt, manual **Update now** button.
3. **Mode at creation + mode-aware behavior** — new-chat flow picks mode, mode-aware summarizer prompts, persona-memory aggregator filters out roleplay sessions, scenario editor becomes visible for roleplay threads.
4. **Fork to roleplay** — fork button on chatbot threads. Creates a new roleplay session with messages and thread memory copied; no memory is edited anywhere else.
5. **Automatic triggering + advanced settings** — unified trigger shape (time + message floor) for both persona and thread memory. Split into four sub-phases for reviewability:
   - **5a.** Backport message floor to persona memory auto-update. Add `auto_memory_message_floor` to persona config. Scheduler now requires interval AND floor. Expose the new field in the existing persona memory settings UI.
   - **5b.** Thread memory settings data model + resolver + daemon scheduler. `thread_memory_settings` on session JSON, `default_thread_memory_settings` on persona config, global fallback constants. New daemon loop iterating sessions. No UI yet — verify via direct JSON edit.
   - **5c.** Thread memory modal settings UI. Form for interval/message floor/size limit with save-inline and "reset to persona defaults."
   - **5d.** Persona settings page additions — writeable `default_mode` picker and `default_thread_memory_settings` section.
6. **Control panel consolidation** — unify scattered UI into a single thread control panel.

## Open questions

- Does persona memory appear in roleplay-mode threads, or is it suppressed for immersion?
- Scenario: inline-only to start, or ship file upload in phase 1?

## Resolved decisions

- **Fork does not edit memory.** Thread memory copies over intact on fork; persona memory is untouched. Auto-memory updates (configured by the user in phase 5) remain valid elsewhere; fork itself is strictly a new-session creation.

## Rejected features

- **Purge-from-persona-memory action.** Originally scoped for phase 4 (two-effect action: set an exclusion flag + LLM-driven surgical edit of existing memory). Rejected because for long threads the LLM can't reliably distinguish content that originated from the thread vs. content that was reinforced across many interactions — a bad purge could quietly wreck months of accumulated context. The risk outweighs the convenience. The existing defenses cover the real use cases:
  - Roleplay-mode filter (phase 3) prevents future contamination automatically.
  - `modify_memory_with_command` lets users make precise, user-described corrections.
  - Wipe memory is available for a clean slate.

## Deferred features

- **On-rollover trigger mode.** Originally scoped for phase 5 as an alternative to the hybrid trigger — fire exactly when the oldest message in the sliding window is about to drop out. Deferred because the value is marginal in practice: most users will pick a large window (100+ messages) and a moderate refresh interval (10–15 min), making the hybrid trigger's redundancy negligible and its gaps tiny. The summarizer reads all messages regardless of the sliding window, so neither mode actually loses information. Revisit if users report stale-memory symptoms.

## Related fix (addressed separately)

**Auto-update scheduler: per-persona intervals were not respected.**

Previously, `memory_worker._auto_update_loop` slept for `min_interval` across all personas, and fired any persona with new activity on every wakeup. A persona with a 60-minute interval would update whenever activity existed and any *other* persona had a shorter interval.

Fix applied: track per-persona next-fire time. A persona fires only when both (a) its own interval has elapsed since its last fire, and (b) there is new session activity. The loop sleeps until the soonest next-due persona.
