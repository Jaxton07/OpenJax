# OpenJax Web

Phase 7 React Web frontend (Vite + React + TypeScript).

## Run

```bash
cd ui/web
pnpm install
pnpm dev
```

Default URL: `http://127.0.0.1:5173`

## Build

```bash
cd ui/web
pnpm build
```

## Test

```bash
cd ui/web
pnpm test
```

## Settings

Open settings from the left-bottom button and configure:

- `Gateway Base URL` (default: `http://127.0.0.1:8765`)
- `API Key`
- `Output Mode` (`sse` or `polling`)

## Gateway Compatibility

This client follows phase-2 contracts:

- `POST /api/v1/sessions`
- `POST /api/v1/sessions/{session_id}/turns`
- `GET /api/v1/sessions/{session_id}/turns/{turn_id}`
- `GET /api/v1/sessions/{session_id}/events`
- `POST /api/v1/sessions/{session_id}/approvals/{approval_id}:resolve`
- `POST /api/v1/sessions/{session_id}:clear`
- `POST /api/v1/sessions/{session_id}:compact`
- `DELETE /api/v1/sessions/{session_id}`
