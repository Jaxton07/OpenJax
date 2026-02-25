"""Tests for SDK runtime wrapper."""

from __future__ import annotations

import asyncio
from dataclasses import dataclass
import unittest

from openjax_tui.sdk_runtime import SdkRuntime


@dataclass
class DummyEvent:
    event_type: str
    turn_id: str | None
    payload: dict


class FakeClient:
    """Minimal fake SDK client for runtime tests."""

    def __init__(self) -> None:
        self.session_id: str | None = None
        self.started = False
        self.stream_subscribed = False
        self.submitted: list[str] = []
        self.resolved: list[tuple[str, str, bool]] = []
        self.stopped = False
        self.shutdown_called = False
        self.events: asyncio.Queue[DummyEvent] = asyncio.Queue()

    async def start(self) -> None:
        self.started = True

    async def start_session(self) -> str:
        self.session_id = "s-1"
        return self.session_id

    async def stream_events(self) -> bool:
        self.stream_subscribed = True
        return True

    async def submit_turn(self, text: str) -> str:
        self.submitted.append(text)
        return "turn-1"

    async def resolve_approval(self, *, turn_id: str, request_id: str, approved: bool, reason: str | None = None) -> bool:
        self.resolved.append((turn_id, request_id, approved))
        return True

    async def next_event(self, timeout: float | None = None) -> DummyEvent:
        try:
            return await asyncio.wait_for(self.events.get(), timeout=timeout)
        except asyncio.TimeoutError as exc:
            raise TimeoutError() from exc

    async def shutdown_session(self) -> bool:
        self.shutdown_called = True
        self.session_id = None
        return True

    async def stop(self) -> None:
        self.stopped = True


class TestSdkRuntime(unittest.IsolatedAsyncioTestCase):
    """Test runtime lifecycle and callbacks."""

    async def test_start_submit_resolve_and_stop(self) -> None:
        events: list[DummyEvent] = []
        errors: list[Exception] = []
        client = FakeClient()
        runtime = SdkRuntime(on_event=events.append, on_error=errors.append, client=client)

        session_id = await runtime.start()
        self.assertEqual(session_id, "s-1")
        self.assertTrue(client.started)
        self.assertTrue(client.stream_subscribed)

        turn_id = await runtime.submit_turn("hello")
        self.assertEqual(turn_id, "turn-1")
        self.assertEqual(client.submitted, ["hello"])

        resolved = await runtime.resolve_approval(turn_id="turn-1", request_id="req-1", approved=True)
        self.assertTrue(resolved)
        self.assertEqual(client.resolved, [("turn-1", "req-1", True)])

        await runtime.stop(graceful=True)
        self.assertTrue(client.shutdown_called)
        self.assertTrue(client.stopped)
        self.assertEqual(errors, [])

    async def test_event_loop_dispatches_events(self) -> None:
        events: list[DummyEvent] = []
        client = FakeClient()
        runtime = SdkRuntime(on_event=events.append, on_error=None, client=client)

        await runtime.start()
        await client.events.put(DummyEvent(event_type="assistant_delta", turn_id="t1", payload={"content_delta": "x"}))
        await asyncio.sleep(0.05)

        self.assertEqual(len(events), 1)
        self.assertEqual(events[0].event_type, "assistant_delta")

        await runtime.stop(graceful=False)

    async def test_event_loop_reports_error(self) -> None:
        class ErrorClient(FakeClient):
            async def next_event(self, timeout: float | None = None) -> DummyEvent:
                raise RuntimeError("stream boom")

        errors: list[Exception] = []
        runtime = SdkRuntime(on_event=None, on_error=errors.append, client=ErrorClient())

        await runtime.start()
        await asyncio.sleep(0.05)

        self.assertEqual(len(errors), 1)
        self.assertIn("stream boom", str(errors[0]))
        await runtime.stop(graceful=False)


if __name__ == "__main__":
    unittest.main()
