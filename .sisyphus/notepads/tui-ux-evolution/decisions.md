2026-02-24
- Chose enum-backed contracts rather than loose strings for new state surfaces to make downstream view/animation logic safer and more discoverable.
- Kept defaults aligned with existing behavior (`session` view, idle animation, no live viewport owner) to avoid runtime behavior drift before later tasks.
- Skill selection rationale: omitted `local-commit`, `mpush`, `git-master`, `playwright`, `dev-browser`, and `frontend-ui-ux` because this task was limited to local Python state/test contract updates with no git operation, browser workflow, or UI styling work.
- Chose adapter-first integration inside `_input_loop_prompt_toolkit` with runtime selection: pilot non-TextArea viewport only for `live_viewport` mode (default impl `pilot`), and TextArea kept as explicit fallback (`OPENJAX_TUI_HISTORY_VIEWPORT_IMPL=textarea`).
- Chose state-preserving fallback strategy: keep `assistant_render` event flow unchanged and implement live retention/flush in viewport refresh path so downstream tasks can iterate without reworking render dispatch semantics.
- Chose one ticker source of truth on `AppState` (`animation_task`) and a fixed bounded interval (`_STATUS_ANIMATION_INTERVAL_S = 1/7`) instead of per-phase task spawning, to prevent parallel ticker races.
- Kept status animation rendering inside prompt_toolkit layout as a conditional row (`status_panel`) and did not change tool timeline formatting in this task.
- Chose completion-time timeline emission using existing `tool_call_started`/`tool_call_completed` events and runtime start timestamps, so lifecycle rows can include accurate duration without introducing new event names.

2026-02-24 (Task 5)
- Kept scope strictly test-harness level (no production logic changes) and validated invariants through state-driven unit tests against  and .
- Did not modify ; existing typed callback wiring remained compatible and unaffected by the new invariants.
- Preserved prior task semantics by asserting only behavior-level contracts (phase transitions, retention/flush outcomes, ticker spawn constraints) and not internal implementation details beyond exposed state fields.

2026-02-24 (Task 5, correction)
- Explicitly anchored harness coverage to retain_live_viewport_blocks and _apply_event_state_updates contracts.
- Confirmed test_app_event_wiring.py remains unchanged for this checklist item.

2026-02-24 (Task 6)
- Kept `event_dispatch.py` unchanged and concentrated deterministic state transitions in `app.py` so event routing semantics and timeline formatting remain stable.
- Chose event-local tool completion evaluation (`_has_active_tool_calls_after_event`) instead of ordering-sensitive assumptions about when runtime tool markers are mutated.
- Chose explicit fallback sanitization (clear live viewport ownership + prompt/history callbacks) before entering basic backend to guarantee viewport-safe continuation after prompt-toolkit failure.

2026-02-24 (Task 7)
- Kept scope in `tool_runtime.py` only (no timeline lifecycle redesign): `started -> running -> completed/failed` semantics remain unchanged while snippet formatting became Unicode display-width-safe.
- Chose stdlib `unicodedata`-based width estimation and grapheme-like clustering instead of adding external `wcwidth` dependency to satisfy no-new-dependency constraints.
- Added dedicated regression coverage in `test_timeline_unicode_width.py` and left `assistant_render.py` unchanged because formatting risk was isolated to tool runtime summarization.

2026-02-24 (Task 8)
- Kept implementation scope to regression tests and notepads; no production logic changes were introduced.
- Chose state/assertion-driven stress patterns (burst loops, event interleaving, patched monotonic clock) over timing sleeps so failures remain deterministic in CI.
- Anchored authoritative-final-content guarantees in both stream rendering and interleaved tool-event scenarios, matching the turn-upsert contract in `assistant_render.render_assistant_message`.

2026-02-24 (Task 8 follow-up)
- Kept this pass strictly within the requested regression scope: only `test_scrollback_live_mode.py`, `test_stream_render.py`, and `test_tool_summary.py` changed.
- Modeled resize-adjacent risk deterministically as repeated retain/update cycles around a long active mixed-width tail block, instead of introducing timing or terminal-dependent assertions.
- For strict timeline duration assertions, used binary-exact monotonic timestamp deltas to avoid float-rounding jitter while preserving deterministic started/running/completed/failed ordering guarantees.

- 2026-02-24: Kept mux probe checks non-blocking in `python_tui_mux_check.sh` (`if ! print_mux_version ...; then ...; fi`) so missing or failing tmux/zellij version probes never prevent the base smoke run from executing.
