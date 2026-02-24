2026-02-24
- Added explicit state contracts in `AppState` via enums (`ViewMode`, `AnimationLifecycle`, `LiveViewportOwnership`) plus deterministic `normalize_view_mode`/`set_view_mode` behavior.
- Deterministic invalid mode handling is implemented as normalization to `ViewMode.SESSION` for stable defaults before future feature-flag flips.
- No additional dependency is needed: Python stdlib `enum` and existing `unittest` already cover the contract and test requirements.
- Introduced `HistoryViewportAdapter` boundary in `app.py` with `TextAreaHistoryViewportAdapter` (legacy fallback) and `PilotHistoryViewportAdapter` (non-buffer live viewport path) without changing the basic backend.
- Live viewport retention now keeps only the active streaming turn block in-memory and flushes finalized blocks to terminal scrollback through the existing run-in-terminal path.
- Added a single status animation controller (`state.animation_task`) with bounded frame cadence (`1/7s`) and phase-gated activation for `thinking` and `tool_wait` only.
- prompt_toolkit lifecycle now drives animation start/stop via explicit sync points (submit, tool events, approval transitions, turn completion, fallback, and shutdown), avoiding orphan invalidation loops.
- Tool lifecycle rendering now emits deterministic timeline rows (`started` -> `running` -> `completed/failed`) per completed call, with terminal rows carrying non-negative elapsed milliseconds and compact output snippets.

2026-02-24 (Task 5)
- RED->GREEN harness now covers live viewport invalid stream markers: out-of-range  or missing  deterministically flushes all blocks instead of retaining stale history.
- Added animation controller guards for negative paths: basic backend never spawns ticker, and non-waiting-turn tool events do not mutate active turn phase/animation state.
- Added tool-wait transition invariant check:  remains in  while any active tool-call start markers remain, then returns to  only after all markers are cleared.

2026-02-24 (Task 5, correction)
- Clarified invariant identifiers: out-of-range stream_block_index or missing stream_turn_id must force full live-mode flush.
- Clarified phase invariant: _apply_event_state_updates stays in tool_wait while any active marker exists and returns to thinking only when all are cleared.

2026-02-24 (Task 6)
- Added explicit live-viewport ownership transitions in `_apply_event_state_updates`: `assistant_delta` marks turn `ACTIVE`, while `assistant_message`/`turn_completed` release ownership deterministically.
- Tool completion phase resolution now uses `_has_active_tool_calls_after_event`, making `tool_wait` vs `thinking` transitions deterministic for the current event without relying on callback timing.
- Prompt-toolkit fallback now sanitizes viewport/stream state (`_finalize_stream_line`, ownership clear, history/prompt callbacks detached) before entering basic backend so continued streaming cannot reuse stale viewport markers.

2026-02-24 (Task 7)
- Timeline snippet truncation in `tool_runtime._summarize_tool_output` now uses display width instead of codepoint count, preventing false truncation for combining sequences (for example `Cafe\u0301`).
- Added grapheme-like cluster slicing for truncation boundaries so ZWJ emoji and regional-indicator pairs are less likely to be split into unreadable fragments.
- Regression tests now cover mixed CJK, emoji, combining marks, and multiline normalization to keep timeline rows readable under multilingual output.

2026-02-24 (Task 8)
- Added deterministic regression bursts for live viewport retention (`test_scrollback_live_mode.py`) that keep only the active turn block while preserving long mixed-width/CJK multiline content losslessly.
- Added stream rendering stress coverage (`test_stream_render.py`) with 120 delta chunks and authoritative final-message override assertions to ensure draft stream content never survives as final UI truth.
- Added interleaved delta+tool lifecycle burst coverage (`test_tool_summary.py`) with strict ordering assertions for started/running/completed rows and deterministic elapsed milliseconds under patched monotonic clock.

2026-02-24 (Task 9)
- Smoke coverage now runs explicit live view cases for both `OPENJAX_TUI_HISTORY_VIEWPORT_IMPL=pilot` and `OPENJAX_TUI_HISTORY_VIEWPORT_IMPL=textarea` with `OPENJAX_TUI_VIEW_MODE=live`.
- Timeline/status assertions in shell smoke were hardened to stable plain-text lifecycle tokens (`[started]`, `[running]`, `[completed]`) plus success footer checks, avoiding brittle ANSI sequence matching.
- Mux wrapper now tolerates missing/broken multiplexer version probes while still executing the base smoke gate.

2026-02-24 (Task 10)
- Operator docs now pin rollout semantics for `OPENJAX_TUI_VIEW_MODE`: `session` as compatibility default and `live` as scrollback-first trial mode.
- Operator docs now pin viewport adapter policy for `OPENJAX_TUI_HISTORY_VIEWPORT_IMPL`: `pilot` default path with `textarea` as first rollback step.
- Troubleshooting flow is now explicit and command-driven: `pilot -> textarea -> session -> basic`, while preserving TTY `prompt_toolkit` vs non-TTY/basic backend guidance.

2026-02-24 (Final-wave conformance blockers)
- Added explicit alias handling in `AppState.normalize_view_mode`: `OPENJAX_TUI_VIEW_MODE=live` now deterministically maps to `ViewMode.LIVE_VIEWPORT` while preserving `live_viewport` compatibility.
- Shell smoke tool-turn cases now drive a pseudo-tty session with `OPENJAX_TUI_INPUT_BACKEND=prompt_toolkit`, so `pilot` and `textarea` cases execute the prompt_toolkit input/runtime path instead of basic fallback.
- Smoke verification now also checks `.openjax` TUI startup log content (`backend=prompt_toolkit`) to confirm backend selection with stable plain-text assertions.

2026-02-24 (Evidence Backfill)
- Created verification evidence artifacts under .sisyphus/evidence/ per plan QA scenario requirements.
- Evidence files follow deterministic naming: task-{N}-{slug}.txt for happy path, task-{N}-{slug}-error.txt for error paths.
- All 20 evidence files (10 tasks x 2 scenarios) successfully generated with actual command outputs and timestamps.
- Commands used include unittest modules (test_state, test_input_backend, test_history_viewport_adapter, test_scrollback_live_mode, test_status_animation, test_tool_summary, test_app_event_wiring, test_timeline_unicode_width, test_stream_render) and shell smoke scripts (python_tui_smoke.sh, python_tui_mux_check.sh).
- All tests pass; smoke tests exit code 0; grep documentation checks confirm env flags are documented.
2026-02-24 (F3 rerun)
- Prompt-toolkit path was re-validated with pseudo-TTY manual runs for both live+pilot and live+textarea; both showed tool timeline tokens in order and clean `/exit` shutdown.
- Current smoke script now drives prompt_toolkit explicitly via PTY harness and verifies `backend=prompt_toolkit` in per-case TUI logs.

2026-02-24 (Compliance closure)
- Promoted validated default to live viewport at state layer: `AppState` now initializes `view_mode=LIVE_VIEWPORT`, and `normalize_view_mode(None)` resolves to live while keeping `live`/`live_viewport` compatibility aliases.
- Session fallback remains explicitly available and covered: `session` mode path is now set directly in regression tests that validate non-live retention behavior.
- Operator docs now align with runtime truth: `OPENJAX_TUI_VIEW_MODE` default is documented as `live` with `session`/`textarea`/`basic` fallback escape hatches.
- Task-5 evidence now includes explicit `RED STEP` and `GREEN STEP` sections with real failing/passing command outputs for audit-friendly reproducibility.

2026-02-24 (Task 8 follow-up)
- Added a bottom-of-viewport stress regression in `test_scrollback_live_mode.py` that seeds many legacy turn blocks, then verifies retain cycles plus resize-adjacent active-turn delta updates keep a single live block losslessly.
- Added a `>=180` chunk rapid-delta regression in `test_stream_render.py` that proves final-message authority overwrites draft burst content (including CJK/emoji/multiline fragments) without duplication.
- Added strict ordered timeline regression in `test_tool_summary.py` with interleaved delta/tool lifecycle events and patched monotonic timestamps to lock deterministic started/running/completed/failed order.

- 2026-02-24: `python_tui_smoke.sh` now writes a stable plain-text `SMOKE_CASE_CONFIG view_mode=... viewport_impl=...` marker into each PTY transcript, so live+pilot and live+textarea rollout paths are explicitly asserted without ANSI-dependent matching.
