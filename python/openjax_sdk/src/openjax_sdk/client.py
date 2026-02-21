from __future__ import annotations

import asyncio
import json
import os
import uuid
from collections import defaultdict
from typing import Any, AsyncIterator

from .exceptions import OpenJaxProtocolError, OpenJaxResponseError
from .models import EventEnvelope, ResponseEnvelope


class OpenJaxAsyncClient:
    def __init__(
        self,
        daemon_cmd: list[str] | None = None,
        protocol_version: str = "v1",
    ) -> None:
        self._daemon_cmd = daemon_cmd or [
            "cargo",
            "run",
            "-q",
            "-p",
            "openjaxd",
        ]
        self._protocol_version = protocol_version
        self._proc: asyncio.subprocess.Process | None = None
        self._read_task: asyncio.Task[None] | None = None
        self._pending: dict[str, asyncio.Future[ResponseEnvelope]] = {}
        self._event_queue: asyncio.Queue[EventEnvelope] = asyncio.Queue()
        self._session_id: str | None = None
        self._assistant_delta_buffer: dict[str, list[str]] = defaultdict(list)
        self._stream_closed = False
        self._stream_closed_reason: Exception | None = None

    @property
    def session_id(self) -> str | None:
        return self._session_id

    async def start(self) -> None:
        if self._proc is not None:
            return
        env = os.environ.copy()
        self._proc = await asyncio.create_subprocess_exec(
            *self._daemon_cmd,
            stdin=asyncio.subprocess.PIPE,
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.PIPE,
            env=env,
        )
        self._read_task = asyncio.create_task(self._read_loop(), name="openjaxd-read-loop")
        self._stream_closed = False
        self._stream_closed_reason = None

    async def stop(self) -> None:
        if self._proc is None:
            return
        if self._proc.stdin:
            self._proc.stdin.close()
        if self._read_task:
            try:
                await asyncio.wait_for(self._read_task, timeout=2)
            except asyncio.TimeoutError:
                self._read_task.cancel()
        try:
            await asyncio.wait_for(self._proc.wait(), timeout=2)
        except asyncio.TimeoutError:
            self._proc.kill()
            await self._proc.wait()
        self._proc = None
        self._read_task = None
        self._pending.clear()
        self._stream_closed = True

    async def start_session(self, metadata: dict[str, Any] | None = None) -> str:
        result = await self._call("start_session", params={"metadata": metadata or {}})
        session_id = str(result["session_id"])
        self._session_id = session_id
        return session_id

    async def stream_events(self, from_seq: int | None = None) -> bool:
        self._require_session()
        params: dict[str, Any] = {}
        if from_seq is not None:
            params["from_seq"] = from_seq
        result = await self._call("stream_events", params=params, session_id=self._session_id)
        return bool(result.get("subscribed", False))

    async def submit_turn(
        self, input_text: str, metadata: dict[str, Any] | None = None
    ) -> str:
        self._require_session()
        result = await self._call(
            "submit_turn",
            params={"input": input_text, "metadata": metadata or {}},
            session_id=self._session_id,
        )
        return str(result["turn_id"])

    async def resolve_approval(
        self, turn_id: str, request_id: str, approved: bool, reason: str | None = None
    ) -> bool:
        self._require_session()
        params: dict[str, Any] = {
            "turn_id": turn_id,
            "request_id": request_id,
            "approved": approved,
        }
        if reason:
            params["reason"] = reason
        result = await self._call(
            "resolve_approval",
            params=params,
            session_id=self._session_id,
        )
        return bool(result.get("resolved", False))

    async def shutdown_session(self) -> bool:
        self._require_session()
        result = await self._call(
            "shutdown_session",
            params={},
            session_id=self._session_id,
        )
        self._session_id = None
        return bool(result.get("closed", False))

    async def next_event(self, timeout: float | None = None) -> EventEnvelope:
        loop = asyncio.get_event_loop()
        deadline = (loop.time() + timeout) if timeout is not None else None

        while True:
            if self._stream_closed and self._event_queue.empty():
                if self._stream_closed_reason:
                    raise self._stream_closed_reason
                raise OpenJaxProtocolError("daemon stream closed")

            wait_timeout = 0.2
            if deadline is not None:
                remaining = deadline - loop.time()
                if remaining <= 0:
                    raise TimeoutError("timed out waiting for event")
                wait_timeout = min(wait_timeout, remaining)
            try:
                return await asyncio.wait_for(self._event_queue.get(), timeout=wait_timeout)
            except asyncio.TimeoutError:
                continue

    async def iter_events(self) -> AsyncIterator[EventEnvelope]:
        while True:
            yield await self.next_event()

    async def collect_turn_events(
        self, turn_id: str, timeout: float = 30.0
    ) -> list[EventEnvelope]:
        events: list[EventEnvelope] = []
        deadline = asyncio.get_event_loop().time() + timeout
        while True:
            remaining = deadline - asyncio.get_event_loop().time()
            if remaining <= 0:
                raise TimeoutError(f"timed out waiting turn {turn_id}")
            evt = await self.next_event(timeout=remaining)
            if evt.turn_id != turn_id:
                continue
            events.append(evt)
            if evt.event_type == "turn_completed":
                return events

    def assistant_text_for_turn(self, turn_id: str) -> str:
        return "".join(self._assistant_delta_buffer.get(turn_id, []))

    async def _call(
        self, method: str, params: dict[str, Any], session_id: str | None = None
    ) -> dict[str, Any]:
        if self._proc is None or self._proc.stdin is None:
            raise OpenJaxProtocolError("daemon is not started")
        request_id = f"req_{uuid.uuid4().hex}"
        loop = asyncio.get_event_loop()
        fut: asyncio.Future[ResponseEnvelope] = loop.create_future()
        self._pending[request_id] = fut

        envelope: dict[str, Any] = {
            "protocol_version": self._protocol_version,
            "kind": "request",
            "request_id": request_id,
            "method": method,
            "params": params,
        }
        if session_id is not None:
            envelope["session_id"] = session_id

        raw = json.dumps(envelope, ensure_ascii=False).encode("utf-8") + b"\n"
        self._proc.stdin.write(raw)
        await self._proc.stdin.drain()
        resp = await fut
        if not resp.ok:
            assert resp.error is not None
            raise OpenJaxResponseError(
                code=resp.error.code,
                message=resp.error.message,
                retriable=resp.error.retriable,
                details=resp.error.details,
            )
        return resp.result or {}

    async def _read_loop(self) -> None:
        assert self._proc is not None and self._proc.stdout is not None
        while True:
            line = await self._proc.stdout.readline()
            if not line:
                self._stream_closed = True
                self._stream_closed_reason = OpenJaxProtocolError("daemon stream closed")
                for fut in self._pending.values():
                    if not fut.done():
                        fut.set_exception(self._stream_closed_reason)
                self._pending.clear()
                return
            text = line.decode("utf-8").strip()
            if not text:
                continue
            payload = json.loads(text)
            kind = payload.get("kind")
            if kind == "response":
                resp = ResponseEnvelope.from_dict(payload)
                fut = self._pending.pop(resp.request_id, None)
                if fut and not fut.done():
                    fut.set_result(resp)
            elif kind == "event":
                evt = EventEnvelope.from_dict(payload)
                if evt.turn_id and evt.event_type == "assistant_delta":
                    delta = str(evt.payload.get("content_delta", ""))
                    self._assistant_delta_buffer[evt.turn_id].append(delta)
                if evt.turn_id and evt.event_type == "assistant_message":
                    content = str(evt.payload.get("content", ""))
                    self._assistant_delta_buffer[evt.turn_id].append(content)
                await self._event_queue.put(evt)

    def _require_session(self) -> None:
        if not self._session_id:
            raise OpenJaxProtocolError("session is not started")
