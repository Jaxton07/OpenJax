"""事件处理适配器测试。"""

from __future__ import annotations

import time
import unittest
from unittest.mock import patch

from openjax_tui.event_handlers import (
    create_emit_ui_line_fn,
    create_finalize_stream_line_if_turn_fn,
    create_record_tool_completed_fn,
    create_record_tool_started_fn,
    create_render_assistant_delta_fn,
    create_render_assistant_message_fn,
    create_status_bullet_fn,
)
from openjax_tui.state import AppState


class EventHandlerFactoryTest(unittest.TestCase):
    """测试事件处理函数工厂。"""

    def test_create_emit_ui_line_fn(self) -> None:
        """测试 emit_ui_line 闭包函数创建。"""
        state = AppState()
        fn = create_emit_ui_line_fn(state)
        self.assertTrue(callable(fn))

    def test_create_status_bullet_fn(self) -> None:
        """测试 status_bullet 闭包函数创建。"""
        state = AppState()
        fn = create_status_bullet_fn(state)
        self.assertTrue(callable(fn))

        # 测试返回字符串
        result = fn(True)
        self.assertIsInstance(result, str)

    def test_create_render_assistant_delta_fn(self) -> None:
        """测试 render_assistant_delta 闭包函数创建。"""
        state = AppState()
        fn = create_render_assistant_delta_fn(state)
        self.assertTrue(callable(fn))

    def test_create_render_assistant_message_fn(self) -> None:
        """测试 render_assistant_message 闭包函数创建。"""
        state = AppState()
        fn = create_render_assistant_message_fn(state)
        self.assertTrue(callable(fn))

    def test_create_finalize_stream_line_if_turn_fn(self) -> None:
        """测试 finalize_stream_line_if_turn 闭包函数创建。"""
        state = AppState()
        fn = create_finalize_stream_line_if_turn_fn(state)
        self.assertTrue(callable(fn))

    def test_create_record_tool_started_fn(self) -> None:
        """测试 record_tool_started 闭包函数创建。"""
        state = AppState()
        fn = create_record_tool_started_fn(state)
        self.assertTrue(callable(fn))

    def test_create_record_tool_completed_fn(self) -> None:
        """测试 record_tool_completed 闭包函数创建。"""
        state = AppState()
        fn = create_record_tool_completed_fn(state)
        self.assertTrue(callable(fn))

        # 测试返回 int (elapsed_ms)
        state.active_tool_starts[("turn1", "test_tool")] = [time.monotonic()]
        result = fn("turn1", "test_tool", True)
        self.assertIsInstance(result, int)


class EventHandlerIntegrationTest(unittest.TestCase):
    """测试事件处理函数集成行为。"""

    def test_status_bullet_with_ansi(self) -> None:
        """测试带 ANSI 颜色的状态子弹。"""
        state = AppState()
        fn = create_status_bullet_fn(state)

        with patch("openjax_tui.event_handlers._supports_ansi_color", return_value=True):
            result_ok = fn(True)
            result_fail = fn(False)

        self.assertIn("⏺", result_ok)
        self.assertIn("⏺", result_fail)

    def test_status_bullet_without_ansi(self) -> None:
        """测试不带 ANSI 颜色的状态子弹。"""
        state = AppState()
        fn = create_status_bullet_fn(state)

        with patch("openjax_tui.event_handlers._supports_ansi_color", return_value=False):
            result_ok = fn(True)
            result_fail = fn(False)

        # 无 ANSI 时返回表情符号
        self.assertIn("🟢", result_ok)
        self.assertIn("🔴", result_fail)


if __name__ == "__main__":
    unittest.main()
