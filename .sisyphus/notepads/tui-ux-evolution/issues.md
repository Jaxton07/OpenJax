2026-02-24
- No functional blockers during Task 1 implementation.
- basedpyright warnings surfaced on touched files; resolved by adding missing attribute annotations in `AppState` and removing a private-module import pattern in state tests.
- No runtime blocker in Task 2, but adapter extraction surfaced basedpyright type friction around prompt_toolkit constructor typing; resolved by simplifying constructor calls and keeping diagnostics error-clean on touched files.
- Task 3 introduced potential phase drift risk (tool events vs approval/turn completion); mitigated by central `_sync_status_animation_controller` and explicit event-phase transitions in `_apply_event_state_updates`.
- Task 4 surfaced a typed callback mismatch after making `record_tool_completed` return elapsed milliseconds; resolved by updating event-dispatch test stubs to return `0` when the callback is intentionally inert.

2026-02-24 (Task 5)
- Initial RED assertion for tool completion transition expected  too early because one active tool marker () remained populated; adjusted test setup to clear all active markers before asserting phase recovery.
- No runtime or typing blockers after harness adjustment; targeted and full suites pass.

2026-02-24 (Task 5, correction)
- Prior appended note lost identifier names due shell interpolation; corrected by appending explicit identifier text for stream_block_index/stream_turn_id and tool_wait/thinking transitions.

2026-02-24 (Task 6)
- No blocking implementation issue, but fallback-path safety required explicit stream marker reset before backend switch; otherwise basic backend streaming could inherit stale `stream_turn_id` from prompt-toolkit viewport state.
- Added integration tests for event ownership lifecycle and fallback state sanitization; typed callback signatures remained compatible without changing `event_dispatch.py`.

2026-02-24 (Task 7)
- No runtime blocker; main implementation risk was truncating by naive `len()` and splitting Unicode clusters in the timeline snippet path.
- Mitigated by replacing `len()`-based limits with display-width helpers and adding negative-path tests that demonstrate naive codepoint counting would incorrectly truncate combining-mark strings.

2026-02-24 (Task 8)
- No production blocker encountered; stress coverage was implemented at test harness level only.
- Deterministic elapsed-time assertions required explicit `time.monotonic` patch sequences to avoid floating-point/clock jitter in interleaved tool lifecycle tests.

2026-02-24 (Task 9)
- Existing shell smoke expectation for `thinking` output is no longer stable in the current CLI transcript path; replaced with lifecycle/status token assertions emitted by tool timeline rows.
- No runtime blocker after update; both standalone smoke and mux wrapper smoke complete successfully in local validation.

2026-02-24 (Task 10)
- No doc-authoring blocker; rollout/fallback semantics were already implemented in runtime and smoke coverage, so task stayed documentation-only.
- Full unittest discover remains green; one known non-blocking `ResourceWarning` for an unclosed event loop still appears in `test_tool_summary` path during suite execution.

2026-02-24 (Final-wave conformance blockers)
- Forcing `OPENJAX_TUI_INPUT_BACKEND=prompt_toolkit` under non-TTY heredoc does not reliably produce tool timeline rows; smoke had to switch to pseudo-tty driving to make prompt_toolkit-path assertions deterministic.
- prompt_toolkit pseudo-tty transcript includes control-sequence noise; retained assertions on stable plain-text timeline/status tokens and startup log backend fields only.

2026-02-24 (Evidence Backfill)
- No blockers during evidence generation. All referenced test modules exist and execute successfully.
- Shell arithmetic issue encountered in task-10 error path generation (integer expression expected); resolved by using grep exit status directly.
- All 20 evidence files created with real command output; no fabricated pass results.
2026-02-24 (F3 rerun)
- No functional blocker in rerun; pilot and textarea prompt_toolkit manual/smoke flows passed.
- Non-blocking UX artifact persists under PTY/script capture: WARNING: your terminal does not support cursor position requests (CPR) appears in transcripts.

2026-02-24 (Compliance closure)
- Default flip to live viewport surfaced one regression in `test_scrollback_live_mode`: session-retention case relied on implicit constructor default; fixed by explicitly setting `state.view_mode = ViewMode.SESSION` inside that test.
- No additional runtime blockers after the regression fix; required state tests, smoke scripts, and full unittest discover pass.
- Known non-blocking `ResourceWarning` in `test_tool_summary` full-suite path remains unchanged.
