from __future__ import annotations

import unittest

from openjax_tui import app
from openjax_tui.state import AppState, ApprovalRecord


class StateModuleTest(unittest.TestCase):
    def test_appstate_defaults(self) -> None:
        state = AppState()
        self.assertTrue(state.running)
        self.assertEqual(state.input_backend, "basic")
        self.assertEqual(state.turn_phase, "idle")
        self.assertEqual(len(state.pending_approvals), 0)

    def test_app_exports_state_types(self) -> None:
        state = app.AppState()
        self.assertIsInstance(state, AppState)
        record = app.ApprovalRecord(turn_id="t1", target="tool", reason="r")
        self.assertIsInstance(record, ApprovalRecord)


if __name__ == "__main__":
    unittest.main()
