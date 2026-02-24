# Draft: Python TUI UX Evolution

## Requirements (confirmed)
- [requirement]: "我其实有考虑后续TUI升级中还要加入一些小的状态动画的，比如thinking 时候，以及tool 调用过程等待中等等，这样减少用户等待焦虑"
- [requirement]: "基于此，我们需要引入新的技术或者包吗"
- [requirement]: "是不是需要改成 Codex 那种纯 scrollback 输出（UI 内仅保留进行中内容），不再依赖长历史视图的形式"
- [requirement]: "之前我们改成纯文本后，原本的tool 调用状态变成了简单的 ... 并不美观"
- [requirement]: "如果我们要持续优化用户体验，这个肯定也得改，那么我们还是得从根本上解决我们之前的问题"

## Technical Decisions
- [decision]: Use `Scrollback-first` interaction model (user confirmed): UI keeps in-progress content; durable history relies on terminal scrollback.
- [decision]: Tool status style is `timeline rows` (user confirmed): started/running/completed/failed lifecycle with better aesthetics and clarity.
- [decision]: Test strategy is `TDD` (user confirmed): write failing tests first, then implementation, then regression expansion.
- [decision]: Dependency strategy default = `prompt_toolkit-only` (supported by librarian/oracle findings); no new package in first phase.
- [decision]: Keep compatibility fallback path for long-history mode during transition window (oracle recommendation).

## Research Findings
- [source]: `docs/plan/bugfix/tui-final-response-truncation-investigation-2026-02-23.md` confirms data path is complete and issue centered on Python TUI rendering/scroll.
- [source]: `python/openjax_tui/src/openjax_tui/state.py` stores stream/final/history state in `stream_text_by_turn`, `assistant_message_by_turn`, `history_blocks`, `turn_block_index`, `history_auto_follow`, `history_manual_scroll`.
- [source]: `python/openjax_tui/src/openjax_tui/assistant_render.py` uses turn-level upsert (`_upsert_turn_block`) and currently maps tool output labels via `tool_result_label` into simple text labels.
- [source]: `python/openjax_tui/src/openjax_tui/app.py` implements history compaction + scrollback flush via `_compact_history_window` and `_schedule_scrollback_flush`, with `OPENJAX_TUI_HISTORY_WINDOW_LINES` as threshold.
- [source]: `python/openjax_tui/src/openjax_tui/tool_runtime.py` renders tool result lines and summaries (`print_tool_call_result_line`, `print_tool_summary_for_turn`) with bullet-only status.
- [source]: Test coverage exists for stream rendering/tool summary/backend selection in `python/openjax_tui/tests/test_stream_render.py`, `python/openjax_tui/tests/test_tool_summary.py`, `python/openjax_tui/tests/test_input_backend.py`; no direct test currently targets PageUp/PageDown scroll state transitions.
- [source]: `librarian` research: prompt_toolkit primitives are sufficient for spinner/ticker/status animation (`Application.refresh_interval`, `create_background_task`, `FormattedTextControl`) and scroll patterns; avoid extra dependency unless proven necessary.
- [source]: `oracle` recommendation: migrate to in-progress-only viewport model with staged rollout and fallback mode; prioritize eliminating scroll-state complexity as root-risk surface.

## Open Questions
- [unanswered]: None currently blocking plan generation.

## Scope Boundaries
- INCLUDE: Python TUI UX architecture strategy for status animations, tool status presentation, and history/scroll model.
- EXCLUDE: Immediate source-code implementation in this planning step.
