# WebUI v2 Architecture

## Goals
- Keep streaming output smooth under high delta throughput.
- Limit rendering scope to AI response area only.
- Keep transport resilient with replay-aware reconnect.

## Layering
`Transport -> Stream Store -> UI Islands`

1. `Transport`
- `GatewayClient` owns HTTP + SSE access to `openjax-gateway`.
- Uses `fetch` for REST and `@microsoft/fetch-event-source` for SSE.
- Applies bearer token auth and `after_event_seq` resume.

2. `Stream Store`
- External store keyed by `sessionId + turnId`.
- Merges `response_text_delta` in a pending buffer.
- Commits buffer with `requestAnimationFrame` (at most one UI commit per frame).
- Exposes `subscribe/getSnapshot` for React `useSyncExternalStore`.

3. `UI Islands`
- `ChatPage`: auth/session/send orchestration only.
- `UserMessageList`: renders user bubbles only.
- `AssistantStreamPane`: independent island for assistant history + active stream text.
- `Composer`: input and send action only.

## Rendering Boundary
Only `AssistantStreamPane` subscribes to streaming snapshots. Parent page and user message list do not receive per-delta state updates.

## Data Flow
1. Login with owner key => access token.
2. Create single session.
3. Start SSE stream loop.
4. Submit turn for each user message.
5. Process events:
- `response_started`: initialize stream entry.
- `response_text_delta`: append buffered delta.
- `response_completed|assistant_message`: finalize assistant text.
- `response_error`: surface model-side error.

## Reliability Strategy
- Track `lastEventSeq` in memory.
- Reconnect with exponential backoff and `after_event_seq=lastEventSeq`.
- Stop automatic replay when receiving `REPLAY_WINDOW_EXCEEDED` and ask user to rebuild session.
