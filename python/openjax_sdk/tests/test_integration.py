import unittest
from pathlib import Path

from openjax_sdk import OpenJaxAsyncClient
from openjax_sdk.exceptions import OpenJaxResponseError


def _daemon_cmd() -> list[str]:
    root = Path(__file__).resolve().parents[3]
    bin_path = root / "target" / "debug" / "openjaxd"
    if bin_path.exists():
        return [str(bin_path)]
    return ["cargo", "run", "-q", "-p", "openjaxd"]


class OpenJaxSdkIntegrationTest(unittest.IsolatedAsyncioTestCase):
    async def asyncSetUp(self) -> None:
        self.client = OpenJaxAsyncClient(daemon_cmd=_daemon_cmd())
        await self.client.start()
        await self.client.start_session()
        await self.client.stream_events()

    async def asyncTearDown(self) -> None:
        if self.client.session_id:
            await self.client.shutdown_session()
        await self.client.stop()

    async def test_submit_turn_and_receive_events(self) -> None:
        turn_id = await self.client.submit_turn("tool:list_dir dir_path=.")
        events = await self.client.collect_turn_events(turn_id, timeout=20)
        event_types = [e.event_type for e in events]
        self.assertIn("turn_started", event_types)
        self.assertIn("tool_call_started", event_types)
        self.assertIn("tool_call_completed", event_types)
        self.assertIn("turn_completed", event_types)

    async def test_invalid_approval_id_returns_error(self) -> None:
        with self.assertRaises(OpenJaxResponseError) as ctx:
            await self.client.resolve_approval(
                turn_id="1",
                request_id="not_found",
                approved=True,
            )
        self.assertEqual(ctx.exception.code, "APPROVAL_NOT_FOUND")


if __name__ == "__main__":
    unittest.main()
