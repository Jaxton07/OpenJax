# Performance Budget

## Runtime Goals
- Maintain smooth typing while streaming long assistant responses.
- Keep streaming rendering isolated from non-streaming UI.

## Budgets (Dev Validation)
- Delta commit cadence: at most once per animation frame per active turn.
- Chat page renders: should not scale with delta count.
- User input latency: no obvious stutter under continuous stream.

## Instrumentation
Enabled by env flag:
- `VITE_OPENJAX_WEBUI_STREAM_PERF=1`

Metrics logged per second:
- `delta_recv_count`
- `delta_commit_count`
- `commit_avg_ms`
- `assistant_pane_renders`

## Validation Checklist
- High-frequency stream: assistant pane updates smoothly.
- UserMessageList render count remains low during stream.
- Composer interaction remains responsive while stream is active.
