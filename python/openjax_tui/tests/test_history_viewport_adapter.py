from __future__ import annotations

import unittest

from openjax_tui.app import (
    PilotHistoryViewportAdapter,
    TextAreaHistoryViewportAdapter,
)
from openjax_tui.state import AppState


class _FakeDocument:
    def __init__(self, *, text: str, cursor_position: int) -> None:
        self.text: str = text
        self.cursor_position: int = cursor_position


class _FakeBuffer:
    def __init__(self) -> None:
        self.cursor_position: int = 0
        self.last_document: _FakeDocument | None = None
        self.bypass_readonly: bool | None = None

    def set_document(self, document: _FakeDocument, bypass_readonly: bool = False) -> None:
        self.last_document = document
        self.cursor_position = document.cursor_position
        self.bypass_readonly = bypass_readonly


class _FakeRenderInfo:
    def __init__(self, *, vertical_scroll: int = 0, content_height: int = 20, window_height: int = 10) -> None:
        self.vertical_scroll: int = vertical_scroll
        self.content_height: int = content_height
        self.window_height: int = window_height


class _FakeWindow:
    def __init__(self) -> None:
        self.vertical_scroll: int = 0
        self.render_info: _FakeRenderInfo = _FakeRenderInfo()


class _FakeTextArea:
    def __init__(self) -> None:
        self.buffer: _FakeBuffer = _FakeBuffer()
        self.window: _FakeWindow = _FakeWindow()


class HistoryViewportAdapterTest(unittest.TestCase):
    def test_textarea_adapter_append_update_refresh_follow(self) -> None:
        state = AppState()
        view = _FakeTextArea()
        adapter = TextAreaHistoryViewportAdapter(state, view, document_cls=_FakeDocument)

        index = adapter.append_block("A")
        adapter.update_block(index, "B")
        self.assertEqual(state.history_blocks, ["B"])

        adapter.refresh("hello")
        self.assertIsNotNone(view.buffer.last_document)
        assert view.buffer.last_document is not None
        self.assertEqual(view.buffer.last_document.text, "hello")
        self.assertEqual(view.buffer.last_document.cursor_position, len("hello"))
        self.assertTrue(view.buffer.bypass_readonly)
        self.assertEqual(view.window.vertical_scroll, 10**9)

        adapter.set_manual_scroll(3)
        self.assertFalse(state.history_auto_follow)
        self.assertEqual(view.window.vertical_scroll, 3)

        view.window.render_info = _FakeRenderInfo(vertical_scroll=10, content_height=20, window_height=10)
        adapter.sync_manual_scroll_from_render()
        self.assertTrue(state.history_auto_follow)

    def test_pilot_adapter_append_update_refresh_follow(self) -> None:
        state = AppState()
        window = _FakeWindow()
        rendered: list[str] = []
        adapter = PilotHistoryViewportAdapter(
            state,
            window,
            set_text_fn=lambda text: rendered.append(text),
        )

        index = adapter.append_block("one")
        adapter.update_block(index, "two")
        self.assertEqual(state.history_blocks, ["two"])

        adapter.refresh("pilot")
        self.assertEqual(rendered[-1], "pilot")
        self.assertEqual(window.vertical_scroll, 10**9)

        adapter.set_manual_scroll(7)
        self.assertFalse(state.history_auto_follow)
        self.assertEqual(window.vertical_scroll, 7)

        window.render_info = _FakeRenderInfo(vertical_scroll=5, content_height=20, window_height=10)
        self.assertEqual(adapter.max_scroll(), 10)
        self.assertEqual(adapter.current_scroll(), 5)

        window.render_info = _FakeRenderInfo(vertical_scroll=10, content_height=20, window_height=10)
        adapter.sync_manual_scroll_from_render()
        self.assertTrue(state.history_auto_follow)


if __name__ == "__main__":
    _ = unittest.main()
