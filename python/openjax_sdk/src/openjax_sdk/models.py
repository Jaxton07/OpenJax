from __future__ import annotations

from dataclasses import dataclass
from typing import Any


@dataclass
class ErrorBody:
    code: str
    message: str
    retriable: bool
    details: dict[str, Any]

    @classmethod
    def from_dict(cls, payload: dict[str, Any]) -> "ErrorBody":
        return cls(
            code=str(payload.get("code", "INTERNAL_ERROR")),
            message=str(payload.get("message", "")),
            retriable=bool(payload.get("retriable", False)),
            details=dict(payload.get("details", {})),
        )


@dataclass
class ResponseEnvelope:
    protocol_version: str
    kind: str
    request_id: str
    ok: bool
    result: dict[str, Any] | None
    error: ErrorBody | None

    @classmethod
    def from_dict(cls, payload: dict[str, Any]) -> "ResponseEnvelope":
        error_payload = payload.get("error")
        return cls(
            protocol_version=str(payload.get("protocol_version", "")),
            kind=str(payload.get("kind", "")),
            request_id=str(payload.get("request_id", "")),
            ok=bool(payload.get("ok", False)),
            result=payload.get("result"),
            error=ErrorBody.from_dict(error_payload)
            if isinstance(error_payload, dict)
            else None,
        )


@dataclass
class EventEnvelope:
    protocol_version: str
    kind: str
    session_id: str
    turn_id: str | None
    event_type: str
    payload: dict[str, Any]

    @classmethod
    def from_dict(cls, payload: dict[str, Any]) -> "EventEnvelope":
        return cls(
            protocol_version=str(payload.get("protocol_version", "")),
            kind=str(payload.get("kind", "")),
            session_id=str(payload.get("session_id", "")),
            turn_id=payload.get("turn_id"),
            event_type=str(payload.get("event_type", "")),
            payload=dict(payload.get("payload", {})),
        )
