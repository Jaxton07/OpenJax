# Streaming Render Spec

## Scope
This spec covers assistant text streaming only. Markdown/tool cards/approval UI are out of scope.

## State Model
Per active turn snapshot:
- `turnId?: string`
- `content: string`
- `lastEventSeq: number`
- `isActive: boolean`
- `version: number`

## Event State Machine
1. `response_started`
- Create or resume stream entry for `turnId`.
- Set `isActive=true`.

2. `response_text_delta`
- Append `payload.content_delta` into pending buffer.
- Defer visible commit via `requestAnimationFrame`.

3. `response_completed`
- Resolve final content from `payload.content` when provided; otherwise use buffered content.
- Set `isActive=false`.
- Emit one final snapshot update.

4. `assistant_message`
- Treated as completion fallback if `response_completed` is absent.

5. `response_error`
- Mark stream inactive.
- Do not clear existing rendered text.
- Surface error message to page-level status.

## Sequence Rules
- Drop events where `event_seq <= lastEventSeq` unless it is a recognized reset boundary.
- Reset boundary:
- `event_seq == 1`, or
- `turn_seq == 1` with `type=response_started`

## Rendering Rules
- Stream updates must not re-render parent chat page.
- `AssistantStreamPane` is the only component subscribed to stream snapshots.
- Per frame: max one content commit per turn key.
