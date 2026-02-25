"""Tests for thinking status widget."""

from __future__ import annotations

import unittest
from unittest.mock import MagicMock

from openjax_tui.widgets.thinking_status import ThinkingStatus


class TestThinkingStatus(unittest.TestCase):
    """Test ThinkingStatus animation behavior."""

    def test_render_contains_thinking_label(self) -> None:
        widget = ThinkingStatus()
        self.assertIn("Thinking", widget.render_text())

    def test_tick_advances_frame_and_updates(self) -> None:
        widget = ThinkingStatus()
        before = widget.render_text()
        widget.update = MagicMock()
        start_phase = widget._phase_position

        widget._tick()
        self.assertNotEqual(start_phase, widget._phase_position)
        widget.update.assert_called_once_with(widget.render_text())

        # High-FPS small-step animation may need a few ticks before visible bucket changes.
        after = widget.render_text()
        for _ in range(6):
            if after != before:
                break
            widget._tick()
            after = widget.render_text()
        self.assertNotEqual(before, after)

    def test_on_mount_starts_interval(self) -> None:
        widget = ThinkingStatus()
        timer = MagicMock()
        widget.set_interval = MagicMock(return_value=timer)
        widget.update = MagicMock()

        widget.on_mount()

        self.assertIs(widget._timer, timer)
        self.assertEqual(widget.set_interval.call_args[0][0], widget.TICK_SECONDS)
        widget.update.assert_called_once_with(widget.render_text())

    def test_on_unmount_stops_timer(self) -> None:
        widget = ThinkingStatus()
        timer = MagicMock()
        widget._timer = timer

        widget.on_unmount()

        timer.stop.assert_called_once()
        self.assertIsNone(widget._timer)


if __name__ == "__main__":
    unittest.main()
