# Python TUI Scrollback-First UX Evolution

## TL;DR
> **Summary**: Migrate Python TUI to a scrollback-first live viewport, add bounded status animations, and redesign tool lifecycle rendering to timeline rows while eliminating the historical render/scroll truncation risk class.
> **Deliverables**:
> - Live viewport mode with viewport adapter + compatibility fallback
> - Thinking/tool-wait animation controller with redraw guardrails
> - Timeline-style tool status rendering with unicode-safe formatting
> - Deterministic regression coverage for scroll, animation, and long-output integrity
> **Effort**: Large
> **Parallel**: YES - 2 waves
> **Critical Path**: 1 -> 2 -> 5 -> 8 -> 9

## Context
### Original Request
User asks whether TUI UX evolution needs new technology/package, whether to move to Codex-like scrollback-first model, and how to fundamentally resolve earlier final-response truncation while improving tool status aesthetics and waiting-state feedback.

### Interview Summary
- Chosen interaction model: `Scrollback-first` (UI keeps in-progress content; history relies on terminal scrollback).
- Chosen tool status style: `Timeline rows` (started/running/completed/failed with better readability).
- Chosen test strategy: `TDD`.
- Direction constraint: avoid dependency churn unless technically required.
- Chosen architecture depth: `Option B partial migration now` (adapter + pilot non-TextArea history viewport + TextArea fallback).
- Default applied: animation cadence policy `6-8 FPS` with strict redraw throttling.
- Default applied: keyboard-first navigation scope (no mouse-support expansion in this phase).

### Metis Review (gaps addressed)
- Incorporated redraw/ticker guardrails: single animation controller, bounded cadence, cancellation on completion/fallback/shutdown.
- Added unicode-width safety requirement for timeline rendering (CJK/emoji/combining chars).
- Added deterministic prompt_toolkit pipe/dummy-output style verification targets; avoid ANSI snapshot-only assertions.
- Incorporated architecture review synthesis: avoid full migration now; implement adapter first and pilot history viewport replacement with rollback gate.

## Work Objectives
### Core Objective
Deliver a robust scrollback-first Python TUI UX that reduces waiting anxiety, improves tool-call readability, and prevents recurrence of viewport-related response-loss behavior.

### Deliverables
- Runtime-configurable view mode with `live` (new primary) and `history` (temporary compatibility) behavior.
- Runtime-configurable history viewport implementation via adapter (`textarea` fallback, pilot non-buffer history view).
- Event-driven animation state model for thinking/tool wait with strict redraw bounds.
- Timeline tool lifecycle renderer replacing current simplistic bullet labels.
- Regression suite covering long text, CJK width, multiline blocks, burst tool events, backend fallback, and multiplexer smoke runs.

### Definition of Done (verifiable conditions with commands)
- `PYTHONPATH=python/openjax_sdk/src:python/openjax_tui/src python3 -m unittest python/openjax_tui/tests/test_status_animation.py -v`
- `PYTHONPATH=python/openjax_sdk/src:python/openjax_tui/src python3 -m unittest python/openjax_tui/tests/test_timeline_unicode_width.py -v`
- `PYTHONPATH=python/openjax_sdk/src:python/openjax_tui/src python3 -m unittest python/openjax_tui/tests/test_scrollback_live_mode.py -v`
- `PYTHONPATH=python/openjax_sdk/src:python/openjax_tui/src python3 -m unittest discover -s python/openjax_tui/tests -v`
- `zsh -lc "zsh smoke_test/python_tui_smoke.sh"`
- `zsh -lc "zsh smoke_test/python_tui_mux_check.sh"`

### Must Have
- Scrollback-first `live` mode implemented behind explicit config and promoted to default after validation.
- `HistoryViewportAdapter` introduced with two implementations: legacy TextArea and pilot non-buffer history viewport.
- Compatibility `history` mode retained one release as escape hatch.
- Animation redraw cadence bounded (target 6-8 FPS max) and cancellation-safe.
- Tool statuses rendered as structured timeline rows with durations and fail markers.
- All acceptance and QA checks agent-executable (no manual-only gate).

### Must NOT Have (guardrails, AI slop patterns, scope boundaries)
- No new UI dependency (e.g., rich/textual) in this plan.
- No unbounded invalidate loops or orphan background tasks.
- No reliance on manual tmux/zellij checks as sole verification.
- No regression of approval commands, input backend fallback, or assistant final-message authority semantics.
- No full all-surface migration in one step; keep reversible pilot gate.

## Verification Strategy
> ZERO HUMAN INTERVENTION - all verification is agent-executed.
- Test decision: `TDD` with Python `unittest` + shell smoke scripts.
- QA policy: Every task contains explicit happy and failure scenarios.
- Evidence: `.sisyphus/evidence/task-{N}-{slug}.{ext}`

## Execution Strategy
### Parallel Execution Waves
> Target: 5-8 tasks per wave.

Wave 1: Foundations and contract changes
- Task 1-5 (state model, viewport adapter abstraction, animation controller contract, timeline schema, test scaffolding)

Wave 2: Integration and hardening
- Task 6-10 (event wiring, renderer integration, regression tests, smoke updates, rollout flag/default plan)

### Dependency Matrix (full, all tasks)
| Task | Depends On |
|---|---|
| 1 | - |
| 2 | 1 |
| 3 | 1 |
| 4 | 1 |
| 5 | 1,2,3,4 |
| 6 | 2,3,4 |
| 7 | 4,6 |
| 8 | 2,6,7 |
| 9 | 6,8 |
| 10 | 8,9 |

### Agent Dispatch Summary (wave -> task count -> categories)
- Wave 1 -> 5 tasks -> `deep`, `unspecified-high`
- Wave 2 -> 5 tasks -> `deep`, `unspecified-high`, `writing`

## TODOs
> Implementation + Test = ONE task. Never separate.
> EVERY task includes Agent Profile + Parallelization + QA Scenarios.

- [x] 1. Establish view-mode and animation state contracts

  **What to do**: Define explicit state fields and enums for `view_mode`, animation lifecycle, and per-turn live viewport ownership; preserve existing default behavior until flag flip. Update state defaults/tests first (RED->GREEN).
  **Must NOT do**: Do not change rendering output yet; do not remove existing `history_auto_follow/history_manual_scroll` compatibility fields.

  **Recommended Agent Profile**:
  - Category: `unspecified-high` - Reason: cross-module contract design with compatibility constraints.
  - Skills: `[]` - No specialized external skill needed.
  - Omitted: `playwright` - Not a browser task.

  **Parallelization**: Can Parallel: YES | Wave 1 | Blocks: 2,3,4,5 | Blocked By: -

  **References** (executor has NO interview context - be exhaustive):
  - Pattern: `python/openjax_tui/src/openjax_tui/state.py` - canonical app state shape.
  - Pattern: `python/openjax_tui/tests/test_state.py` - state default assertions.
  - API/Type: `python/openjax_tui/src/openjax_tui/app.py` - uses `turn_phase`, `input_backend`, `history_setter`.
  - External: `https://python-prompt-toolkit.readthedocs.io/en/stable/pages/reference.html#module-prompt_toolkit.application` - redraw/task lifecycle primitives.

  **Acceptance Criteria** (agent-executable only):
  - [ ] `PYTHONPATH=python/openjax_sdk/src:python/openjax_tui/src python3 -m unittest python/openjax_tui/tests/test_state.py -v` passes with new contract fields asserted.
  - [ ] Existing backend-selection tests remain green: `PYTHONPATH=python/openjax_sdk/src:python/openjax_tui/src python3 -m unittest python/openjax_tui/tests/test_input_backend.py -v`.

  **QA Scenarios** (MANDATORY - task incomplete without these):
  ```bash
  Scenario: Contract bootstrap happy path
    Tool: Bash
    Steps: Run state and backend unit tests listed in acceptance criteria.
    Expected: Both test commands finish with OK and no skipped/error cases.
    Evidence: .sisyphus/evidence/task-1-state-contract.txt

  Scenario: Invalid mode guard
    Tool: Bash
    Steps: Add/execute a unit test that sets unsupported view-mode input and asserts fallback/default behavior.
    Expected: Unsupported value is rejected or normalized deterministically; test passes.
    Evidence: .sisyphus/evidence/task-1-state-contract-error.txt
  ```

  **Commit**: YES | Message: `feat(tui): define live view and animation state contracts` | Files: `python/openjax_tui/src/openjax_tui/state.py`, `python/openjax_tui/tests/test_state.py`

- [x] 2. Implement `HistoryViewportAdapter` and pilot non-TextArea history viewport

  **What to do**: Add adapter boundary for history rendering (`HistoryViewportAdapter`) and implement pilot non-buffer history viewport for live path while preserving TextArea-based implementation as fallback. Live mode still keeps only in-progress/current-turn content and flushes finalized blocks to terminal scrollback.
  **Must NOT do**: Do not remove current TextArea implementation; do not change basic backend behavior.

  **Recommended Agent Profile**:
  - Category: `deep` - Reason: high-risk render/scroll control-flow refactor.
  - Skills: `[]` - Uses internal codebase patterns.
  - Omitted: `frontend-ui-ux` - terminal runtime logic, not web UI styling.

  **Parallelization**: Can Parallel: NO | Wave 1 | Blocks: 5,6,8 | Blocked By: 1

  **References** (executor has NO interview context - be exhaustive):
  - Pattern: `python/openjax_tui/src/openjax_tui/app.py` - `_input_loop_prompt_toolkit`, `_compact_history_window`, `_schedule_scrollback_flush`, pageup/pagedown wiring.
  - Pattern: `python/openjax_tui/src/openjax_tui/prompt_ui.py` - `history_text`, refresh invalidation helpers.
  - Pattern: `python/openjax_tui/src/openjax_tui/assistant_render.py` - turn-authoritative block upsert semantics.
  - Test: `python/openjax_tui/tests/test_prompt_ui.py` - history text behavior pattern.
  - External: `https://python-prompt-toolkit.readthedocs.io/en/stable/pages/full_screen_apps.html`
  - External: `https://python-prompt-toolkit.readthedocs.io/en/stable/pages/reference.html#prompt_toolkit.layout.controls.FormattedTextControl`

  **Acceptance Criteria** (agent-executable only):
  - [ ] Add and pass adapter-focused tests: `PYTHONPATH=python/openjax_sdk/src:python/openjax_tui/src python3 -m unittest python/openjax_tui/tests/test_history_viewport_adapter.py -v`.
  - [ ] Add and pass live mode history retention tests: `PYTHONPATH=python/openjax_sdk/src:python/openjax_tui/src python3 -m unittest python/openjax_tui/tests/test_scrollback_live_mode.py -v`.
  - [ ] Existing stream render tests remain green: `PYTHONPATH=python/openjax_sdk/src:python/openjax_tui/src python3 -m unittest python/openjax_tui/tests/test_stream_render.py -v`.

  **QA Scenarios** (MANDATORY - task incomplete without these):
  ```bash
  Scenario: Adapter pilot happy path
    Tool: Bash
    Steps: Execute test_history_viewport_adapter with pilot implementation enabled and stream/final updates.
    Expected: Adapter contract passes for append/update/scroll-follow; no direct TextArea-only assumptions leak.
    Evidence: .sisyphus/evidence/task-2-live-viewport.txt

  Scenario: TextArea fallback still works
    Tool: Bash
    Steps: Run tests with fallback implementation selection and simulate pageup/pagedown transitions.
    Expected: Legacy manual scroll behavior remains unchanged and assertions pass under fallback adapter.
    Evidence: .sisyphus/evidence/task-2-live-viewport-error.txt
  ```

  **Commit**: YES | Message: `feat(tui): add history viewport adapter with pilot implementation` | Files: `python/openjax_tui/src/openjax_tui/app.py`, `python/openjax_tui/src/openjax_tui/prompt_ui.py`, `python/openjax_tui/tests/test_history_viewport_adapter.py`, `python/openjax_tui/tests/test_scrollback_live_mode.py`

- [x] 3. Add bounded animation controller for thinking and tool-wait phases

  **What to do**: Implement a single animation ticker controller tied to prompt_toolkit app lifecycle; animate thinking/tool-wait indicators at bounded cadence and cancel ticker on turn completion, backend fallback, and shutdown.
  **Must NOT do**: Do not create multiple parallel ticker tasks; do not call unbounded `invalidate()` loops.

  **Recommended Agent Profile**:
  - Category: `unspecified-high` - Reason: async lifecycle correctness and redraw performance constraints.
  - Skills: `[]` - Internal async/prompt_toolkit logic.
  - Omitted: `dev-browser` - irrelevant runtime.

  **Parallelization**: Can Parallel: NO | Wave 1 | Blocks: 5,6,8 | Blocked By: 1

  **References** (executor has NO interview context - be exhaustive):
  - Pattern: `python/openjax_tui/src/openjax_tui/app.py` - event loop and fallback path (`prompt_toolkit_exited_early`).
  - Pattern: `python/openjax_tui/src/openjax_tui/prompt_ui.py` - prompt redraw helper.
  - Pattern: `python/openjax_tui/src/openjax_tui/state.py` - lifecycle/turn phase fields.
  - External: `https://python-prompt-toolkit.readthedocs.io/en/stable/pages/advanced_topics/asyncio.html`
  - External: `https://python-prompt-toolkit.readthedocs.io/en/stable/pages/reference.html#module-prompt_toolkit.application`

  **Acceptance Criteria** (agent-executable only):
  - [ ] `PYTHONPATH=python/openjax_sdk/src:python/openjax_tui/src python3 -m unittest python/openjax_tui/tests/test_status_animation.py -v` passes.
  - [ ] Test includes teardown assertions for fallback path and turn completion.

  **QA Scenarios** (MANDATORY - task incomplete without these):
  ```bash
  Scenario: Animation ticker happy path
    Tool: Bash
    Steps: Run status animation tests with mocked clock/frame source and controlled turn_phase transitions.
    Expected: Frame index advances only while thinking/tool-wait is active and redraw calls stay within configured cadence.
    Evidence: .sisyphus/evidence/task-3-animation.txt

  Scenario: Fallback cancellation
    Tool: Bash
    Steps: Run test path that simulates prompt_toolkit failure and backend switch to basic.
    Expected: Ticker task is cancelled and no further redraw requests occur.
    Evidence: .sisyphus/evidence/task-3-animation-error.txt
  ```

  **Commit**: YES | Message: `feat(tui): add bounded status animation controller` | Files: `python/openjax_tui/src/openjax_tui/app.py`, `python/openjax_tui/src/openjax_tui/state.py`, `python/openjax_tui/tests/test_status_animation.py`

- [x] 4. Replace simple tool labels with timeline lifecycle rows

  **What to do**: Redesign tool-call rendering from simple one-line success/fail labels to timeline rows (`started`, `running`, `completed/failed`) with duration and concise result text; maintain compact terminal-friendly text format.
  **Must NOT do**: Do not drop failure visibility; do not remove per-turn summary plumbing until replacement is fully covered by tests.

  **Recommended Agent Profile**:
  - Category: `deep` - Reason: event semantics + render output redesign with backward compatibility.
  - Skills: `[]` - internal patterns sufficient.
  - Omitted: `artistry` - correctness over novelty.

  **Parallelization**: Can Parallel: NO | Wave 1 | Blocks: 6,7,8 | Blocked By: 1

  **References** (executor has NO interview context - be exhaustive):
  - Pattern: `python/openjax_tui/src/openjax_tui/tool_runtime.py` - current result/summary rendering.
  - Pattern: `python/openjax_tui/src/openjax_tui/event_dispatch.py` - event ordering for tool started/completed.
  - Pattern: `python/openjax_tui/src/openjax_tui/assistant_render.py` - label helper and UI line emission.
  - Test: `python/openjax_tui/tests/test_tool_summary.py` - baseline expectations to migrate.
  - External: `https://python-prompt-toolkit.readthedocs.io/en/stable/pages/printing_text.html`

  **Acceptance Criteria** (agent-executable only):
  - [ ] Updated tool timeline tests pass: `PYTHONPATH=python/openjax_sdk/src:python/openjax_tui/src python3 -m unittest python/openjax_tui/tests/test_tool_summary.py -v`.
  - [ ] Timeline format includes explicit fail marker and non-negative duration assertion in tests.

  **QA Scenarios** (MANDATORY - task incomplete without these):
  ```bash
  Scenario: Timeline lifecycle happy path
    Tool: Bash
    Steps: Run test sequence tool_call_started -> tool_call_completed(ok=True) -> turn_completed.
    Expected: Output/history contains ordered lifecycle rows with success state and duration.
    Evidence: .sisyphus/evidence/task-4-tool-timeline.txt

  Scenario: Failed tool lifecycle
    Tool: Bash
    Steps: Run test sequence with tool_call_completed(ok=False).
    Expected: Timeline row displays failed state and keeps turn completion behavior intact.
    Evidence: .sisyphus/evidence/task-4-tool-timeline-error.txt
  ```

  **Commit**: YES | Message: `feat(tui): introduce timeline tool lifecycle rendering` | Files: `python/openjax_tui/src/openjax_tui/tool_runtime.py`, `python/openjax_tui/src/openjax_tui/event_dispatch.py`, `python/openjax_tui/tests/test_tool_summary.py`

- [x] 5. Build TDD harness for live-mode and animation invariants

  **What to do**: Create failing-first test modules for live viewport behavior, animation cadence bounds, and lifecycle teardown; then implement minimum code to pass while preserving existing tests.
  **Must NOT do**: Do not merge code changes without corresponding RED->GREEN history in test additions.

  **Recommended Agent Profile**:
  - Category: `unspecified-high` - Reason: non-trivial test architecture and deterministic async testing.
  - Skills: `[]` - no external skill required.
  - Omitted: `quick` - scope exceeds trivial patch.

  **Parallelization**: Can Parallel: YES | Wave 1 | Blocks: 8 | Blocked By: 1,2,3,4

  **References** (executor has NO interview context - be exhaustive):
  - Pattern: `python/openjax_tui/tests/test_stream_render.py` - stream/final dedupe test style.
  - Pattern: `python/openjax_tui/tests/test_tool_summary.py` - event simulation and monotonic mocking.
  - Pattern: `python/openjax_tui/tests/test_app_event_wiring.py` - app/event integration style.
  - Pattern: `python/openjax_tui/tests/test_prompt_ui.py` - prompt/history helpers.

  **Acceptance Criteria** (agent-executable only):
  - [ ] New tests initially fail on old behavior and pass after implementation in same branch.
  - [ ] `PYTHONPATH=python/openjax_sdk/src:python/openjax_tui/src python3 -m unittest discover -s python/openjax_tui/tests -v` remains fully green.

  **QA Scenarios** (MANDATORY - task incomplete without these):
  ```bash
  Scenario: RED->GREEN verification
    Tool: Bash
    Steps: Run targeted new tests before and after implementation; capture both outputs.
    Expected: Pre-implementation failure is reproducible; post-implementation all pass.
    Evidence: .sisyphus/evidence/task-5-tdd-harness.txt

  Scenario: Regression safety
    Tool: Bash
    Steps: Run full test discovery after targeted tests pass.
    Expected: No legacy test regressions introduced.
    Evidence: .sisyphus/evidence/task-5-tdd-harness-error.txt
  ```

  **Commit**: YES | Message: `test(tui): add tdd harness for live mode and animation invariants` | Files: `python/openjax_tui/tests/test_scrollback_live_mode.py`, `python/openjax_tui/tests/test_status_animation.py`, `python/openjax_tui/tests/test_app_event_wiring.py`

- [x] 6. Integrate event flow with adapter-based viewport and animation lifecycle

  **What to do**: Wire `assistant_delta`, `assistant_message`, `tool_call_started/completed`, `turn_completed`, and fallback paths so adapter-based viewport and animation controller transition states deterministically.
  **Must NOT do**: Do not rely on implicit side effects; all transitions must be explicit and test-covered.

  **Recommended Agent Profile**:
  - Category: `deep` - Reason: cross-cutting state machine behavior across app/event/renderer modules.
  - Skills: `[]` - internal event model.
  - Omitted: `oracle` - implementation task, not advisory.

  **Parallelization**: Can Parallel: NO | Wave 2 | Blocks: 7,8,9 | Blocked By: 2,3,4

  **References** (executor has NO interview context - be exhaustive):
  - Pattern: `python/openjax_tui/src/openjax_tui/event_dispatch.py` - event routing order.
  - Pattern: `python/openjax_tui/src/openjax_tui/app.py` - `_dispatch_event`, `_apply_event_state_updates`, fallback behavior.
  - Pattern: `python/openjax_tui/src/openjax_tui/assistant_render.py` - final message authoritative update.
  - Test: `python/openjax_tui/tests/test_app_event_wiring.py` - event wiring assertion style.

  **Acceptance Criteria** (agent-executable only):
  - [ ] Event wiring tests pass: `PYTHONPATH=python/openjax_sdk/src:python/openjax_tui/src python3 -m unittest python/openjax_tui/tests/test_app_event_wiring.py -v`.
  - [ ] Adapter mode + timeline + animation integration tests pass without flaky timing sleeps.

  **QA Scenarios** (MANDATORY - task incomplete without these):
  ```bash
  Scenario: Integrated happy path turn
    Tool: Bash
    Steps: Simulate turn_started -> assistant_delta burst -> tool start/done -> assistant_message -> turn_completed.
    Expected: Final content is retained in scrollback route; animation stops; timeline rows ordered correctly.
    Evidence: .sisyphus/evidence/task-6-event-integration.txt

  Scenario: Prompt_toolkit failure path
    Tool: Bash
    Steps: Simulate prompt_toolkit loop exception path with state fallback to basic.
    Expected: Live mode state resets safely and animation tasks are cancelled.
    Evidence: .sisyphus/evidence/task-6-event-integration-error.txt
  ```

  **Commit**: YES | Message: `refactor(tui): unify event lifecycle for adapter viewport and animations` | Files: `python/openjax_tui/src/openjax_tui/app.py`, `python/openjax_tui/src/openjax_tui/event_dispatch.py`, `python/openjax_tui/tests/test_app_event_wiring.py`

- [x] 7. Implement unicode-width-safe timeline formatting

  **What to do**: Ensure timeline row layout/truncation uses display-width-safe logic for CJK/emoji/combining marks, and preserve readable alignment in wrapped/multiline output.
  **Must NOT do**: Do not use naive `len()` for visual alignment-sensitive operations.

  **Recommended Agent Profile**:
  - Category: `unspecified-high` - Reason: text rendering edge cases with terminal width semantics.
  - Skills: `[]` - use existing dependency set; keep package footprint unchanged.
  - Omitted: `artistry` - deterministic formatting is priority.

  **Parallelization**: Can Parallel: YES | Wave 2 | Blocks: 8 | Blocked By: 4,6

  **References** (executor has NO interview context - be exhaustive):
  - Pattern: `python/openjax_tui/src/openjax_tui/assistant_render.py` - multiline alignment helper.
  - Pattern: `python/openjax_tui/src/openjax_tui/tool_runtime.py` - row label/status composition.
  - Test: `python/openjax_tui/tests/test_stream_render.py` - multiline assertion style.
  - External: `https://python-prompt-toolkit.readthedocs.io/en/stable/pages/advanced_topics/rendering_pipeline.html`

  **Acceptance Criteria** (agent-executable only):
  - [ ] `PYTHONPATH=python/openjax_sdk/src:python/openjax_tui/src python3 -m unittest python/openjax_tui/tests/test_timeline_unicode_width.py -v` passes.
  - [ ] Test cases include mixed CJK, emoji, and combining-character strings.

  **QA Scenarios** (MANDATORY - task incomplete without these):
  ```bash
  Scenario: Unicode width happy path
    Tool: Bash
    Steps: Run timeline unicode width tests with mixed-language labels and outputs.
    Expected: Alignment/truncation assertions pass for all fixtures.
    Evidence: .sisyphus/evidence/task-7-unicode-width.txt

  Scenario: Width regression guard
    Tool: Bash
    Steps: Execute failure fixture using intentionally naive formatter path (test double).
    Expected: Guard test fails naive path and passes production formatter.
    Evidence: .sisyphus/evidence/task-7-unicode-width-error.txt
  ```

  **Commit**: YES | Message: `fix(tui): make timeline rendering unicode-width safe` | Files: `python/openjax_tui/src/openjax_tui/tool_runtime.py`, `python/openjax_tui/src/openjax_tui/assistant_render.py`, `python/openjax_tui/tests/test_timeline_unicode_width.py`

- [ ] 8. Add truncation-class regression suite and stress scenarios

  **What to do**: Add deterministic regression tests targeting former failure class: long content near viewport bottom, CJK+multiline mix, rapid delta bursts, rapid tool events, and resize-adjacent rendering updates.
  **Must NOT do**: Do not rely solely on manual reproduction; test suite must encode reproduction patterns.

  **Recommended Agent Profile**:
  - Category: `deep` - Reason: failure-class recreation and reliability hardening.
  - Skills: `[]` - internal test stack.
  - Omitted: `playwright` - terminal app verification is non-browser.

  **Parallelization**: Can Parallel: NO | Wave 2 | Blocks: 9,10 | Blocked By: 2,6,7

  **References** (executor has NO interview context - be exhaustive):
  - Pattern: `docs/plan/bugfix/tui-final-response-truncation-investigation-2026-02-23.md` - historical symptoms and validation signals.
  - Pattern: `python/openjax_tui/src/openjax_tui/app.py` - compaction/follow scroll logic currently linked to risk.
  - Test: `python/openjax_tui/tests/test_stream_render.py`, `python/openjax_tui/tests/test_tool_summary.py` - event-driven test style.
  - Smoke: `smoke_test/python_tui_smoke.sh`

  **Acceptance Criteria** (agent-executable only):
  - [ ] `PYTHONPATH=python/openjax_sdk/src:python/openjax_tui/src python3 -m unittest python/openjax_tui/tests/test_scrollback_live_mode.py -v` includes stress cases and passes.
  - [ ] Full test discovery passes after stress additions.

  **QA Scenarios** (MANDATORY - task incomplete without these):
  ```bash
  Scenario: Historical failure-class happy path
    Tool: Bash
    Steps: Run stress test generating long mixed-width assistant output with final message assertion.
    Expected: Final assistant content matches authoritative message and is fully visible in output route.
    Evidence: .sisyphus/evidence/task-8-truncation-regression.txt

  Scenario: Burst tool + stream edge case
    Tool: Bash
    Steps: Run stress test that interleaves frequent deltas and tool completions.
    Expected: No content loss, no ordering corruption, and no uncaught exceptions.
    Evidence: .sisyphus/evidence/task-8-truncation-regression-error.txt
  ```

  **Commit**: YES | Message: `test(tui): add stress regressions for truncation risk class` | Files: `python/openjax_tui/tests/test_scrollback_live_mode.py`, `python/openjax_tui/tests/test_stream_render.py`, `python/openjax_tui/tests/test_tool_summary.py`

- [ ] 9. Update smoke scripts for pilot/fallback viewport modes and multiplexer reliability gates

  **What to do**: Extend smoke scripts to run explicit pilot viewport and fallback viewport cases and assert timeline/status output invariants in terminal, then reuse mux wrapper for tmux/zellij environment checks.
  **Must NOT do**: Do not introduce brittle assertions against raw ANSI escape sequences.

  **Recommended Agent Profile**:
  - Category: `unspecified-high` - Reason: end-to-end reliability checks and shell harness hardening.
  - Skills: `[]` - existing shell + unittest pipeline.
  - Omitted: `quick` - requires careful non-flaky assertions.

  **Parallelization**: Can Parallel: YES | Wave 2 | Blocks: 10 | Blocked By: 6,8

  **References** (executor has NO interview context - be exhaustive):
  - Pattern: `smoke_test/python_tui_smoke.sh` - base smoke structure and grep assertions.
  - Pattern: `smoke_test/python_tui_mux_check.sh` - multiplexer wrapper.
  - Pattern: `python/openjax_tui/README.md` - documented run and env variables.

  **Acceptance Criteria** (agent-executable only):
  - [ ] `zsh -lc "zsh smoke_test/python_tui_smoke.sh"` passes with pilot viewport enabled case.
  - [ ] `zsh -lc "zsh smoke_test/python_tui_mux_check.sh"` passes in local environment.

  **QA Scenarios** (MANDATORY - task incomplete without these):
  ```bash
  Scenario: Pilot viewport smoke happy path
    Tool: Bash
    Steps: Run python_tui_smoke.sh with pilot viewport implementation flag and scripted input covering tool call + assistant final.
    Expected: Script exits 0 and output assertions for timeline/live status pass.
    Evidence: .sisyphus/evidence/task-9-smoke-live.txt

  Scenario: Multiplexer wrapper failure guard
    Tool: Bash
    Steps: Run python_tui_mux_check.sh in environment missing tmux/zellij binaries.
    Expected: Script reports not installed but still executes base smoke and exits successfully.
    Evidence: .sisyphus/evidence/task-9-smoke-live-error.txt
  ```

  **Commit**: YES | Message: `test(tui): strengthen viewport pilot/fallback smoke and mux checks` | Files: `smoke_test/python_tui_smoke.sh`, `smoke_test/python_tui_mux_check.sh`

- [ ] 10. Rollout defaults, compatibility window, and operator-facing docs

  **What to do**: Finalize rollout behavior: keep fallback viewport env flag, document pilot->default switch conditions, and update Python TUI README/startup status messaging for new UX semantics and troubleshooting.
  **Must NOT do**: Do not remove fallback switch in the same change that flips default without passing all gates.

  **Recommended Agent Profile**:
  - Category: `writing` - Reason: precise operator/developer documentation and rollout policy communication.
  - Skills: `[]` - repo-native documentation style.
  - Omitted: `deep` - implementation already complete; this is policy/documentation finalization.

  **Parallelization**: Can Parallel: NO | Wave 2 | Blocks: - | Blocked By: 8,9

  **References** (executor has NO interview context - be exhaustive):
  - Pattern: `python/openjax_tui/README.md` - env vars and behavior docs.
  - Pattern: `docs/plan/bugfix/tui-final-response-truncation-investigation-2026-02-23.md` - historical context to reference in migration notes.
  - Pattern: `AGENTS.md` (project) - stable approval/input behavior guardrails.

  **Acceptance Criteria** (agent-executable only):
  - [ ] README includes `OPENJAX_TUI_VIEW_MODE` and viewport-implementation semantics, fallback notes, and verification commands.
  - [ ] Full verification command bundle in plan Definition of Done executes successfully.

  **QA Scenarios** (MANDATORY - task incomplete without these):
  ```bash
  Scenario: Rollout doc happy path
    Tool: Bash
    Steps: Run grep checks to ensure README contains live/history mode, viewport implementation mode, and troubleshooting sections; then run full unittest discover.
    Expected: Required sections present and tests pass.
    Evidence: .sisyphus/evidence/task-10-rollout-docs.txt

  Scenario: Missing flag documentation guard
    Tool: Bash
    Steps: Add/execute a doc test or script assertion that fails if OPENJAX_TUI_VIEW_MODE or viewport implementation mode is undocumented.
    Expected: Guard fails when key section absent and passes when present.
    Evidence: .sisyphus/evidence/task-10-rollout-docs-error.txt
  ```

  **Commit**: YES | Message: `docs(tui): document viewport pilot rollout and fallback policy` | Files: `python/openjax_tui/README.md`, `docs/plan/bugfix/tui-final-response-truncation-investigation-2026-02-23.md`

## Final Verification Wave (4 parallel agents, ALL must APPROVE)
- [ ] F1. Plan Compliance Audit - oracle
- [ ] F2. Code Quality Review - unspecified-high
- [ ] F3. Real Manual QA - unspecified-high (+ playwright if UI)
- [ ] F4. Scope Fidelity Check - deep

## Commit Strategy
- Prefer 3 atomic commits: (1) view-mode/state foundation, (2) renderer/timeline integration, (3) tests/smoke/docs.
- Message style: emoji + conventional commit aligned with repository history.

## Success Criteria
- No reproducible case where daemon/tui logs show full final content but viewport/scrollback output is incomplete.
- Timeline rows improve readability over prior `🟢/🔴 + short label` output while preserving compactness.
- Waiting-state animations are perceptible, non-jittery, and do not degrade input responsiveness.
- Compatibility mode remains available and functional during rollout window.
