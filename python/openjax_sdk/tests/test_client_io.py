from __future__ import annotations

import asyncio
import unittest
from unittest.mock import patch

from openjax_sdk.client import OpenJaxAsyncClient


class _FakeStdin:
    def __init__(self) -> None:
        self.closed = False

    def write(self, data: bytes) -> None:
        _ = data

    async def drain(self) -> None:
        return

    def close(self) -> None:
        self.closed = True


class _FakeReader:
    def __init__(self, lines: list[bytes]) -> None:
        self._lines = lines

    async def readline(self) -> bytes:
        if self._lines:
            return self._lines.pop(0)
        return b""


class _FakeProc:
    def __init__(self, stdout_lines: list[bytes] | None = None) -> None:
        self.stdin = _FakeStdin()
        self.stdout = _FakeReader(stdout_lines or [b""])
        self.stderr = _FakeReader([b"log line\n", b""])

    async def wait(self) -> int:
        return 0

    def kill(self) -> None:
        return


class ClientIoTest(unittest.IsolatedAsyncioTestCase):
    async def test_start_spawns_stderr_drain_task(self) -> None:
        fake_proc = _FakeProc()
        client = OpenJaxAsyncClient(daemon_cmd=["true"])

        async def _fake_exec(*args: object, **kwargs: object) -> _FakeProc:
            _ = args
            _ = kwargs
            return fake_proc

        with patch("asyncio.create_subprocess_exec", new=_fake_exec):
            await client.start()
            self.assertIsNotNone(client._read_task)
            self.assertIsNotNone(client._stderr_task)
            await asyncio.sleep(0)
            await client.stop()
            self.assertIsNone(client._stderr_task)

    async def test_read_loop_skips_non_json_and_keeps_events(self) -> None:
        event_line = (
            b'{"protocol_version":"v1","kind":"event","session_id":"s1",'
            b'"turn_id":"1","event_type":"approval_requested","payload":{"request_id":"r1"}}\n'
        )
        fake_proc = _FakeProc(stdout_lines=[b"not-json\n", event_line, b""])
        client = OpenJaxAsyncClient(daemon_cmd=["true"])

        async def _fake_exec(*args: object, **kwargs: object) -> _FakeProc:
            _ = args
            _ = kwargs
            return fake_proc

        with patch("asyncio.create_subprocess_exec", new=_fake_exec):
            await client.start()
            evt = await client.next_event(timeout=1.0)
            self.assertEqual(evt.event_type, "approval_requested")
            await client.stop()


if __name__ == "__main__":
    unittest.main()
