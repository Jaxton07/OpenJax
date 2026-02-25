"""Async runtime wrapper around openjax_sdk for Textual app integration."""

from __future__ import annotations

import asyncio
import inspect
import os
import shlex
from typing import TYPE_CHECKING, Any

from .logging_setup import get_logger

if TYPE_CHECKING:
    from collections.abc import Awaitable, Callable

    from openjax_sdk.models import EventEnvelope

logger = get_logger()

try:
    from openjax_sdk import OpenJaxAsyncClient
    from openjax_sdk.exceptions import OpenJaxProtocolError, OpenJaxResponseError
except Exception:  # pragma: no cover - dependency optional during local unit tests
    OpenJaxAsyncClient = None  # type: ignore[assignment]
    OpenJaxProtocolError = RuntimeError  # type: ignore[assignment]
    OpenJaxResponseError = RuntimeError  # type: ignore[assignment]


class SdkRuntime:
    """Manage SDK client lifecycle and daemon event loop."""

    def __init__(
        self,
        on_event: "Callable[[EventEnvelope], Any] | None" = None,
        on_error: "Callable[[Exception], Any] | None" = None,
        client: OpenJaxAsyncClient | None = None,
    ) -> None:
        self._on_event = on_event
        self._on_error = on_error
        self._client = client
        self._event_task: asyncio.Task[None] | None = None
        self._running = False

    @property
    def client(self) -> OpenJaxAsyncClient:
        """Get the active SDK client."""
        if self._client is None:
            if OpenJaxAsyncClient is None:
                raise RuntimeError(
                    "openjax_sdk is not installed. Run `make setup-new` "
                    "or set PYTHONPATH=python/openjax_sdk/src:python/tui/src"
                )
            self._client = OpenJaxAsyncClient(daemon_cmd=_daemon_cmd_from_env())
        return self._client

    async def start(self) -> str:
        """Start daemon client, session and event stream."""
        await self.client.start()
        session_id = await self.client.start_session()
        await self.client.stream_events()
        self._running = True
        self._event_task = asyncio.create_task(self._event_loop(), name="openjax-tui-event-loop")
        logger.info("sdk_runtime started session_id=%s", session_id)
        return session_id

    async def stop(self, graceful: bool = True) -> None:
        """Stop event loop and shutdown client."""
        self._running = False
        if self._event_task is not None:
            self._event_task.cancel()
            try:
                await self._event_task
            except asyncio.CancelledError:
                pass
            self._event_task = None

        if self._client is None:
            return

        if graceful and self.client.session_id:
            try:
                await asyncio.wait_for(self.client.shutdown_session(), timeout=1.0)
            except (OpenJaxProtocolError, OpenJaxResponseError, TimeoutError):
                logger.exception("sdk_runtime shutdown_session failed")

        try:
            await self.client.stop()
        except (OpenJaxProtocolError, OpenJaxResponseError, TimeoutError):
            logger.exception("sdk_runtime stop failed")

    async def submit_turn(self, text: str) -> str:
        """Submit a user turn to the daemon."""
        return await self.client.submit_turn(text)

    async def resolve_approval(
        self,
        *,
        turn_id: str,
        request_id: str,
        approved: bool,
        reason: str | None = None,
    ) -> bool:
        """Resolve approval request by id."""
        return await self.client.resolve_approval(
            turn_id=turn_id,
            request_id=request_id,
            approved=approved,
            reason=reason,
        )

    async def _event_loop(self) -> None:
        while self._running:
            try:
                evt = await self.client.next_event(timeout=0.5)
            except TimeoutError:
                continue
            except asyncio.CancelledError:
                raise
            except Exception as err:
                await self._emit_error(err)
                return
            await self._emit_event(evt)

    async def _emit_event(self, evt: "EventEnvelope") -> None:
        if self._on_event is None:
            return
        result = self._on_event(evt)
        if inspect.isawaitable(result):
            await result

    async def _emit_error(self, err: Exception) -> None:
        if self._on_error is None:
            return
        result = self._on_error(err)
        if inspect.isawaitable(result):
            await result


def _daemon_cmd_from_env() -> list[str]:
    raw = os.environ.get("OPENJAX_DAEMON_CMD", "").strip()
    if raw:
        parsed = shlex.split(raw)
        if parsed:
            return parsed
    return ["cargo", "run", "-q", "-p", "openjaxd"]
