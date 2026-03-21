# Python GLM Streaming Backend (for WebUI A/B testing)

A minimal backend compatible with `ui/webui` protocol to isolate frontend streaming behavior from `openjax-gateway`.

## Features
- `POST /api/v1/auth/login`
- `POST /api/v1/sessions`
- `POST /api/v1/sessions/{session_id}/turns`
- `GET /api/v1/sessions/{session_id}/events` (SSE with replay via `after_event_seq`)
- Event types: `response_started`, `response_text_delta`, `response_completed`, `assistant_message`, `turn_completed`, `response_error`
- `assistant_message` is legacy compatibility only; authoritative finalization should follow `response_completed`.

## Run
```bash
cd python/backend
python3 -m venv .venv
source .venv/bin/activate
pip install -r requirements.txt
uvicorn app:app --host 127.0.0.1 --port 8780 --reload
```

## Use with WebUI
- WebUI Base URL: `http://127.0.0.1:8780`
- Owner Key input: your GLM API key

## Notes
- Login bearer key is treated as GLM provider API key.
- Data is stored in memory only.
- This backend is for debugging/perf comparison, not production.
