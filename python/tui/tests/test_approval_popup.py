"""Tests for approval popup widget."""

from __future__ import annotations

import unittest

from openjax_tui.widgets.approval_popup import ApprovalPopup


class TestApprovalPopup(unittest.TestCase):
    """Test approval popup interactions."""

    def test_default_selection_is_approve(self) -> None:
        popup = ApprovalPopup()
        self.assertEqual(popup.selected_index, 0)
        self.assertEqual(popup.options[0].name, "approve")
        self.assertEqual(popup.options[0].label, "Approve")
        self.assertEqual(popup.options[1].label, "Deny")
        self.assertEqual(popup.options[2].label, "Cancel or decide later")

    def test_move_selection_bounds(self) -> None:
        popup = ApprovalPopup()

        popup.move_selection(-1)
        self.assertEqual(popup.selected_index, 0)

        popup.move_selection(10)
        self.assertEqual(popup.selected_index, 2)

        popup.move_selection(1)
        self.assertEqual(popup.selected_index, 2)

    def test_confirm_selection_posts_event(self) -> None:
        popup = ApprovalPopup()
        posted = []
        popup.post_message = lambda msg: posted.append(msg)  # type: ignore[assignment]

        popup.confirm_selection()

        self.assertEqual(len(posted), 1)
        self.assertIsInstance(posted[0], ApprovalPopup.SelectionConfirmed)
        self.assertEqual(posted[0].option_name, "approve")

    def test_dismiss_posts_event(self) -> None:
        popup = ApprovalPopup()
        posted = []
        popup.post_message = lambda msg: posted.append(msg)  # type: ignore[assignment]

        popup.dismiss()

        self.assertEqual(len(posted), 1)
        self.assertIsInstance(posted[0], ApprovalPopup.Dismissed)

    def test_format_summary_is_natural_language(self) -> None:
        summary = ApprovalPopup.format_summary(
            approval_id="14136c07-eeda-4a1d-a36a-f6ef64b305cf",
            action="apply_patch",
            turn_id="turn-3",
            reason="tool call requires approval by policy",
        )

        self.assertIn("Approval required (14136c07)", summary)
        self.assertIn("allow apply patch", summary)
        self.assertIn("for turn-3?", summary)
        self.assertIn("Reason: tool call requires approval by policy", summary)


if __name__ == "__main__":
    unittest.main()
