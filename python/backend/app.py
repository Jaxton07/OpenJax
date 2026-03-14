from __future__ import annotations

import asyncio
import json
import uuid
from dataclasses import dataclass, field
from datetime import UTC, datetime
from typing import Any

import httpx
from fastapi import Depends, FastAPI, Header, Query, Request
from fastapi.middleware.cors import CORSMiddleware
from fastapi.responses import JSONResponse, Response
from sse_starlette.sse import EventSourceResponse

GLM_CHAT_COMPLETIONS_URL = "https://open.bigmodel.cn/api/paas/v4/chat/completions"
DEFAULT_MODEL = "glm-4.7-flash"
DEFAULT_REPLAY_LIMIT = 2048


@dataclass
class EventEnvelope:
    request_id: str
    session_id: str
    turn_id: str | None
    event_seq: int
    turn_seq: int
    timestamp: str
    event_type: str
    stream_source: str
    payload: dict[str, Any]

    def to_wire(self) -> dict[str, Any]:
        return {
            "request_id": self.request_id,
            "session_id": self.session_id,
            "turn_id": self.turn_id,
            "event_seq": self.event_seq,
            "turn_seq": self.turn_seq,
            "timestamp": self.timestamp,
            "type": self.event_type,
            "stream_source": self.stream_source,
            "payload": self.payload,
        }


@dataclass
class SessionRuntime:
    session_id: str
    owner_token: str
    glm_api_key: str
    next_event_seq: int = 1
    turn_seq_counter: dict[str, int] = field(default_factory=dict)
    event_log: list[EventEnvelope] = field(default_factory=list)
    replay_limit: int = DEFAULT_REPLAY_LIMIT
    condition: asyncio.Condition = field(default_factory=asyncio.Condition)

    async def publish(
        self,
        *,
        request_id: str,
        turn_id: str | None,
        event_type: str,
        payload: dict[str, Any],
        stream_source: str,
    ) -> EventEnvelope:
        async with self.condition:
            if turn_id:
                current_turn_seq = self.turn_seq_counter.get(turn_id, 0) + 1
                self.turn_seq_counter[turn_id] = current_turn_seq
            else:
                current_turn_seq = 0

            envelope = EventEnvelope(
                request_id=request_id,
                session_id=self.session_id,
                turn_id=turn_id,
                event_seq=self.next_event_seq,
                turn_seq=current_turn_seq,
                timestamp=now_rfc3339(),
                event_type=event_type,
                stream_source=stream_source,
                payload=payload,
            )
            self.next_event_seq += 1
            self.event_log.append(envelope)
            if len(self.event_log) > self.replay_limit:
                self.event_log = self.event_log[-self.replay_limit :]
            self.condition.notify_all()
            return envelope

    async def replay_from(self, after_event_seq: int | None) -> list[EventEnvelope]:
        async with self.condition:
            if not after_event_seq:
                return list(self.event_log)

            min_allowed = self.event_log[0].event_seq - 1 if self.event_log else 0
            if after_event_seq < min_allowed:
                raise ValueError(f"REPLAY_WINDOW_EXCEEDED:min_allowed={min_allowed}")

            return [event for event in self.event_log if event.event_seq > after_event_seq]

    async def wait_new_events(self, after_event_seq: int, timeout_sec: float = 20.0) -> list[EventEnvelope]:
        async with self.condition:
            await asyncio.wait_for(
                self.condition.wait_for(lambda: any(event.event_seq > after_event_seq for event in self.event_log)),
                timeout=timeout_sec,
            )
            return [event for event in self.event_log if event.event_seq > after_event_seq]


app = FastAPI(title="OpenJax Python Backend", version="0.1.0")
app.add_middleware(
    CORSMiddleware,
    allow_origins=["http://127.0.0.1:5173", "http://localhost:5173", "http://127.0.0.1:5174", "http://localhost:5174"],
    allow_methods=["*"],
    allow_headers=["*"],
    allow_credentials=True,
)

TOKENS: dict[str, str] = {}
SESSIONS: dict[str, SessionRuntime] = {}


def now_rfc3339() -> str:
    return datetime.now(tz=UTC).isoformat().replace("+00:00", "Z")


def request_id_of(request: Request) -> str:
    header = request.headers.get("x-request-id", "").strip()
    return header or f"req_{uuid.uuid4().hex}"


def parse_bearer(authorization: str | None) -> str | None:
    if not authorization:
        return None
    value = authorization.strip()
    if not value.startswith("Bearer "):
        return None
    token = value[len("Bearer ") :].strip()
    return token if token else None


def error_response(
    *,
    request_id: str,
    status: int,
    code: str,
    message: str,
    retryable: bool,
    details: dict[str, Any] | None = None,
) -> JSONResponse:
    return JSONResponse(
        status_code=status,
        content={
            "request_id": request_id,
            "timestamp": now_rfc3339(),
            "error": {
                "code": code,
                "message": message,
                "retryable": retryable,
                "details": details or {},
            },
        },
    )


async def require_access_token(request: Request, authorization: str | None = Header(default=None)) -> str:
    token = parse_bearer(authorization)
    if not token or token not in TOKENS:
        response = error_response(
            request_id=request_id_of(request),
            status=401,
            code="UNAUTHENTICATED",
            message="access token is invalid or expired",
            retryable=False,
        )
        raise_http_response(response)
    return token


class HttpResponseException(Exception):
    def __init__(self, response: JSONResponse) -> None:
        self.response = response


def raise_http_response(response: JSONResponse) -> None:
    raise HttpResponseException(response)


@app.exception_handler(HttpResponseException)
async def http_response_exception_handler(_: Request, exc: HttpResponseException) -> JSONResponse:
    return exc.response


@app.get("/healthz")
async def healthz() -> dict[str, str]:
    return {"status": "ok"}


@app.post("/api/v1/auth/login")
async def login(request: Request, authorization: str | None = Header(default=None)) -> JSONResponse:
    req_id = request_id_of(request)
    owner_key = parse_bearer(authorization)
    if not owner_key:
        return error_response(
            request_id=req_id,
            status=401,
            code="UNAUTHENTICATED",
            message="missing owner Authorization header",
            retryable=False,
        )

    access_token = f"atk_{uuid.uuid4().hex}"
    TOKENS[access_token] = owner_key

    return JSONResponse(
        {
            "request_id": req_id,
            "access_token": access_token,
            "access_expires_in": 86400,
            "session_id": f"auth_{uuid.uuid4().hex}",
            "scope": "owner",
            "timestamp": now_rfc3339(),
        }
    )


@app.post("/api/v1/sessions")
async def create_session(request: Request, token: str = Depends(require_access_token)) -> JSONResponse:
    req_id = request_id_of(request)
    session_id = f"sess_{uuid.uuid4().hex}"
    SESSIONS[session_id] = SessionRuntime(
        session_id=session_id,
        owner_token=token,
        glm_api_key=TOKENS[token],
    )
    return JSONResponse(
        {
            "request_id": req_id,
            "session_id": session_id,
            "timestamp": now_rfc3339(),
        }
    )


@app.post("/api/v1/sessions/{session_id}/turns")
async def submit_turn(
    session_id: str,
    request: Request,
    token: str = Depends(require_access_token),
) -> JSONResponse:
    req_id = request_id_of(request)
    runtime = SESSIONS.get(session_id)
    if runtime is None:
        return error_response(
            request_id=req_id,
            status=404,
            code="NOT_FOUND",
            message="session not found",
            retryable=False,
            details={"session_id": session_id},
        )
    if runtime.owner_token != token:
        return error_response(
            request_id=req_id,
            status=403,
            code="FORBIDDEN",
            message="session does not belong to token",
            retryable=False,
        )

    body = await request.json()
    user_input = str(body.get("input", "")).strip()
    if not user_input:
        return error_response(
            request_id=req_id,
            status=400,
            code="INVALID_ARGUMENT",
            message="input is required",
            retryable=False,
            details={"field": "input"},
        )

    turn_id = f"turn_{uuid.uuid4().hex[:10]}"
    asyncio.create_task(run_turn(runtime, turn_id, req_id, user_input))

    return JSONResponse(
        {
            "request_id": req_id,
            "session_id": session_id,
            "turn_id": turn_id,
            "timestamp": now_rfc3339(),
        }
    )


@app.get("/api/v1/sessions/{session_id}/events", response_model=None)
async def stream_events(
    session_id: str,
    request: Request,
    after_event_seq: int | None = Query(default=None),
    token: str = Depends(require_access_token),
) -> Response:
    req_id = request_id_of(request)
    runtime = SESSIONS.get(session_id)
    if runtime is None:
        return error_response(
            request_id=req_id,
            status=404,
            code="NOT_FOUND",
            message="session not found",
            retryable=False,
            details={"session_id": session_id},
        )
    if runtime.owner_token != token:
        return error_response(
            request_id=req_id,
            status=403,
            code="FORBIDDEN",
            message="session does not belong to token",
            retryable=False,
        )

    try:
        replay = await runtime.replay_from(after_event_seq)
    except ValueError as err:
        return error_response(
            request_id=req_id,
            status=400,
            code="INVALID_ARGUMENT",
            message="replay point is outside retention window",
            retryable=False,
            details={"reason": str(err)},
        )

    async def event_generator() -> Any:
        cursor = after_event_seq or 0

        for event in replay:
            cursor = max(cursor, event.event_seq)
            yield {
                "id": str(event.event_seq),
                "event": event.event_type,
                "data": json.dumps(event.to_wire(), ensure_ascii=False),
            }

        while True:
            if await request.is_disconnected():
                break
            try:
                events = await runtime.wait_new_events(cursor)
            except TimeoutError:
                continue
            except asyncio.TimeoutError:
                continue

            for event in events:
                if event.event_seq <= cursor:
                    continue
                cursor = event.event_seq
                yield {
                    "id": str(event.event_seq),
                    "event": event.event_type,
                    "data": json.dumps(event.to_wire(), ensure_ascii=False),
                }

    return EventSourceResponse(event_generator(), ping=15)


async def run_turn(runtime: SessionRuntime, turn_id: str, request_id: str, user_input: str) -> None:
    await runtime.publish(
        request_id=request_id,
        turn_id=turn_id,
        event_type="response_started",
        payload={},
        stream_source="model_live",
    )

    full_text = ""
    try:
        async for delta in stream_glm(runtime.glm_api_key, user_input):
            if not delta:
                continue
            full_text += delta
            await runtime.publish(
                request_id=request_id,
                turn_id=turn_id,
                event_type="response_text_delta",
                payload={"content_delta": delta},
                stream_source="model_live",
            )

        await runtime.publish(
            request_id=request_id,
            turn_id=turn_id,
            event_type="response_completed",
            payload={"content": full_text},
            stream_source="model_live",
        )
        await runtime.publish(
            request_id=request_id,
            turn_id=turn_id,
            event_type="assistant_message",
            payload={"content": full_text},
            stream_source="synthetic",
        )
        await runtime.publish(
            request_id=request_id,
            turn_id=turn_id,
            event_type="turn_completed",
            payload={},
            stream_source="synthetic",
        )
    except Exception as err:
        await runtime.publish(
            request_id=request_id,
            turn_id=turn_id,
            event_type="response_error",
            payload={
                "code": "UPSTREAM_ERROR",
                "message": f"glm stream failed: {err}",
                "retryable": True,
            },
            stream_source="synthetic",
        )


async def stream_glm(api_key: str, user_input: str) -> Any:
    headers = {
        "Authorization": f"Bearer {api_key}",
        "Content-Type": "application/json",
    }
    payload = {
        "model": DEFAULT_MODEL,
        "messages": [{"role": "user", "content": user_input}],
        "thinking": {"type": "enabled"},
        "stream": True,
        "max_tokens": 65536,
        "temperature": 1.0,
    }

    timeout = httpx.Timeout(connect=10.0, read=120.0, write=20.0, pool=10.0)
    async with httpx.AsyncClient(timeout=timeout) as client:
        async with client.stream("POST", GLM_CHAT_COMPLETIONS_URL, headers=headers, json=payload) as response:
            if response.status_code != 200:
                body = await response.aread()
                raise RuntimeError(f"status={response.status_code}, body={body.decode(errors=ignore)}")

            async for line in response.aiter_lines():
                if not line:
                    continue
                if not line.startswith("data:"):
                    continue
                data = line[len("data:") :].strip()
                if data == "[DONE]":
                    break

                try:
                    parsed = json.loads(data)
                except json.JSONDecodeError:
                    continue

                delta = extract_delta_text(parsed)
                if delta:
                    yield delta


def extract_delta_text(payload: dict[str, Any]) -> str:
    choices = payload.get("choices")
    if not isinstance(choices, list) or not choices:
        return ""

    first = choices[0]
    if not isinstance(first, dict):
        return ""

    delta = first.get("delta")
    if isinstance(delta, dict):
        content = delta.get("content")
        if isinstance(content, str):
            return content
        if isinstance(content, list):
            return "".join(item.get("text", "") for item in content if isinstance(item, dict))

    message = first.get("message")
    if isinstance(message, dict):
        content = message.get("content")
        if isinstance(content, str):
            return content

    return ""
