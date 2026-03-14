# Test Plan

## Unit Tests
1. SSE parsing and event JSON extraction.
2. Stream store event sequencing and dedup behavior.
3. Stream store RAF batching commit behavior.
4. Completion merge (`response_completed` over buffered text).
5. Error parsing and error classification.

## Integration Tests (Frontend)
1. Login -> create session -> submit turn happy path (mocked fetch).
2. Stream resume uses `after_event_seq`.
3. `REPLAY_WINDOW_EXCEEDED` stops reconnect loop and sets blocking error.
4. Auth error surfaces actionable message.

## Manual Acceptance
1. Continuous long output keeps input smooth.
2. User message region does not reflow on each delta.
3. Assistant text grows monotonically without flicker.
4. Network interruption reconnects with replay window when possible.
