from __future__ import annotations

import unittest

from openjax_sdk.exceptions import OpenJaxResponseError
from openjax_tui import approval
from openjax_tui.state import AppState, ApprovalRecord


class ApprovalModuleTest(unittest.TestCase):
    def test_input_prompt_prefix_for_basic_approval(self) -> None:
        state = AppState()
        state.turn_phase = "approval"
        state.pending_approvals["ap-1"] = ApprovalRecord(
            turn_id="t1",
            target="apply_patch",
            reason="needs approval",
        )
        state.approval_order.append("ap-1")
        state.approval_focus_id = "ap-1"
        self.assertEqual(approval.input_prompt_prefix(state, "❯"), "approval>")

    def test_approval_toolbar_text_has_request(self) -> None:
        state = AppState()
        state.input_backend = "prompt_toolkit"
        state.approval_ui_enabled = True
        state.turn_phase = "approval"
        state.pending_approvals["ap-2"] = ApprovalRecord(
            turn_id="t2",
            target="write_file",
            reason="policy check",
        )
        state.approval_order.append("ap-2")
        state.approval_focus_id = "ap-2"
        text = approval.approval_toolbar_text(state, "----")
        self.assertIn("ap-2", text)
        self.assertIn("write_file", text)

    def test_is_expired_approval_error(self) -> None:
        err = OpenJaxResponseError(
            code="APPROVAL_NOT_FOUND",
            message="approval request not found or already resolved",
            retriable=False,
            details={},
        )
        self.assertTrue(approval.is_expired_approval_error(err))


if __name__ == "__main__":
    unittest.main()
