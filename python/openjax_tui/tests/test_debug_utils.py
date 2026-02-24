"""调试工具函数测试。"""

from __future__ import annotations

import unittest

from openjax_sdk.models import EventEnvelope
from openjax_tui.debug_utils import (
    format_event_debug_line,
    normalize_history_for_prompt_toolkit,
    truncate_debug_preview,
)


class TruncateDebugPreviewTest(unittest.TestCase):
    """测试 truncate_debug_preview 函数。"""

    def test_short_text_not_truncated(self) -> None:
        """短文本不应被截断。"""
        text = "hello world"
        result, truncated = truncate_debug_preview(text, limit=100)
        self.assertEqual(result, "hello world")
        self.assertFalse(truncated)

    def test_long_text_truncated(self) -> None:
        """长文本应被截断。"""
        text = "a" * 200
        result, truncated = truncate_debug_preview(text, limit=100)
        self.assertEqual(result, "a" * 100 + "...")
        self.assertTrue(truncated)

    def test_exact_limit_not_truncated(self) -> None:
        """刚好达到限制长度的文本不应被截断。"""
        text = "a" * 100
        result, truncated = truncate_debug_preview(text, limit=100)
        self.assertEqual(result, "a" * 100)
        self.assertFalse(truncated)

    def test_newlines_normalized(self) -> None:
        """换行符应被标准化为转义序列。"""
        text = "line1\nline2\r\nline3"
        result, _ = truncate_debug_preview(text, limit=100)
        self.assertEqual(result, "line1\\nline2\\r\\nline3")


class FormatEventDebugLineTest(unittest.TestCase):
    """测试 format_event_debug_line 函数。"""

    def _make_event(
        self,
        event_type: str,
        turn_id: str | None = "t1",
        payload: dict[str, object] | None = None,
    ) -> EventEnvelope:
        """创建测试事件。"""
        return EventEnvelope(
            protocol_version="v1",
            kind="event",
            session_id="s1",
            turn_id=turn_id,
            event_type=event_type,
            payload=payload or {},
        )

    def test_simple_event_formatting(self) -> None:
        """测试简单事件格式化。"""
        evt = self._make_event("turn_started", payload={"key": "value"})
        result = format_event_debug_line(evt)
        self.assertIn("event received type=turn_started", result)
        self.assertIn("turn_id=t1", result)
        self.assertIn("payload_keys=['key']", result)

    def test_event_without_turn_id(self) -> None:
        """测试无 turn_id 的事件格式化。"""
        evt = self._make_event("turn_started", turn_id=None)
        result = format_event_debug_line(evt)
        self.assertIn("turn_id=-", result)

    def test_assistant_delta_formatting(self) -> None:
        """测试 assistant_delta 事件格式化。"""
        evt = self._make_event(
            "assistant_delta",
            payload={"content_delta": "hello world", "other_key": 123},
        )
        result = format_event_debug_line(evt)
        self.assertIn("event received type=assistant_delta", result)
        self.assertIn("delta_len=11", result)
        self.assertIn("delta_preview='hello world'", result)
        self.assertIn("delta_truncated=False", result)

    def test_assistant_delta_truncated(self) -> None:
        """测试 assistant_delta 长内容截断。"""
        long_content = "a" * 200
        evt = self._make_event(
            "assistant_delta",
            payload={"content_delta": long_content},
        )
        result = format_event_debug_line(evt)
        self.assertIn("delta_len=200", result)
        self.assertIn("delta_truncated=True", result)
        self.assertIn("...", result)

    def test_assistant_message_formatting(self) -> None:
        """测试 assistant_message 事件格式化。"""
        evt = self._make_event(
            "assistant_message",
            payload={"content": "final message", "turn_complete": True},
        )
        result = format_event_debug_line(evt)
        self.assertIn("event received type=assistant_message", result)
        self.assertIn("content_len=13", result)
        self.assertIn("content_preview='final message'", result)
        self.assertIn("content_truncated=False", result)

    def test_assistant_message_truncated(self) -> None:
        """测试 assistant_message 长内容截断。"""
        long_content = "b" * 200
        evt = self._make_event(
            "assistant_message",
            payload={"content": long_content},
        )
        result = format_event_debug_line(evt)
        self.assertIn("content_len=200", result)
        self.assertIn("content_truncated=True", result)


class NormalizeHistoryForPromptToolkitTest(unittest.TestCase):
    """测试 normalize_history_for_prompt_toolkit 函数。"""

    def test_plain_text_unchanged(self) -> None:
        """纯文本应保持不变。"""
        text = "hello world"
        result = normalize_history_for_prompt_toolkit(text)
        self.assertEqual(result, "hello world")

    def test_green_prefix_replaced(self) -> None:
        """绿色前缀应被替换为无色前缀。"""
        text = "\x1b[32m⏺\x1b[0m message"
        result = normalize_history_for_prompt_toolkit(text)
        self.assertEqual(result, "⏺ message")

    def test_red_prefix_replaced(self) -> None:
        """红色前缀应被替换为无色前缀。"""
        text = "\x1b[31m⏺\x1b[0m message"
        result = normalize_history_for_prompt_toolkit(text)
        self.assertEqual(result, "⏺ message")

    def test_ansi_sequences_removed(self) -> None:
        """ANSI 转义序列应被移除。"""
        text = "\x1b[1m\x1b[33mbold yellow\x1b[0m"
        result = normalize_history_for_prompt_toolkit(text)
        self.assertEqual(result, "bold yellow")

    def test_custom_prefix(self) -> None:
        """测试自定义前缀。"""
        text = "\x1b[32m>>\x1b[0m message"
        result = normalize_history_for_prompt_toolkit(
            text, assistant_prefix=">>", ansi_green="\x1b[32m", ansi_reset="\x1b[0m"
        )
        self.assertEqual(result, ">> message")


if __name__ == "__main__":
    unittest.main()
