# Gateway Protocol (WebUI v2)

## Auth
### Login
- Method: `POST /api/v1/auth/login`
- Header: `Authorization: Bearer <owner_key>`
- Body:
```json
{
  "device_name": "openjax-webui",
  "platform": "web",
  "user_agent": "..."
}
```
- Response fields used by WebUI:
- `access_token`
- `session_id` (auth session)

## Chat Session
### Create Session
- Method: `POST /api/v1/sessions`
- Header: `Authorization: Bearer <access_token>`
- Response: `session_id`

### Submit Turn
- Method: `POST /api/v1/sessions/:session_id/turns`
- Header: `Authorization: Bearer <access_token>`
- Body:
```json
{ "input": "user text" }
```
- Response: `turn_id`

## SSE Stream
### Subscribe
- Method: `GET /api/v1/sessions/:session_id/events?after_event_seq=<n>`
- Header: `Authorization: Bearer <access_token>`
- Optional header fallback: `Last-Event-ID`

### Envelope
```json
{
  "request_id": "req_x",
  "session_id": "sess_x",
  "turn_id": "turn_x",
  "event_seq": 10,
  "turn_seq": 3,
  "timestamp": "2026-...Z",
  "type": "response_text_delta",
  "stream_source": "model_live",
  "payload": {"content_delta": "..."}
}
```

## Events Used by v2
- `response_started`
- `response_text_delta`
- `response_completed`
- `assistant_message`
- `response_error`
- `turn_completed`

## Error Handling
Unified API error envelope includes:
- `error.code`
- `error.message`
- `error.retryable`

WebUI handles specially:
- `UNAUTHENTICATED`
- `NOT_FOUND`
- `TIMEOUT`
- `UPSTREAM_UNAVAILABLE`
- `INVALID_ARGUMENT`

SSE payload special case:
- `response_error` with `code=REPLAY_WINDOW_EXCEEDED`: stop replay loop and require new session stream.
