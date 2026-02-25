from __future__ import annotations

import unittest
from types import SimpleNamespace

from openjax_tui import prompt_ui as prompt_keybindings
from openjax_tui.state import AppState


class PromptKeybindingsModuleTest(unittest.TestCase):
    def test_build_returns_none_without_keybindings_cls(self) -> None:
        state = AppState()
        kb = prompt_keybindings.build_prompt_key_bindings(
            key_bindings_cls=None,
            state=state,
            approval_mode_active_fn=lambda _s: False,
            toggle_approval_selection_fn=lambda _s: None,
            on_tab_non_approval_fn=lambda _evt: None,
        )
        self.assertIsNone(kb)

    def test_yes_no_are_not_bound_as_custom_shortcuts(self) -> None:
        state = AppState()

        class _FakeKeyBindings:
            def __init__(self) -> None:
                self.handlers: dict[str, object] = {}

            def add(self, key: str, eager: bool = False):  # noqa: ANN001
                _ = eager

                def _decorator(fn):  # noqa: ANN001
                    self.handlers[key] = fn
                    return fn

                return _decorator

        class _FakeBuffer:
            def __init__(self) -> None:
                self.text = ""

            def insert_text(self, text: str) -> None:
                self.text += text

        buffer = _FakeBuffer()
        event = SimpleNamespace(app=SimpleNamespace(current_buffer=buffer))
        kb = prompt_keybindings.build_prompt_key_bindings(
            key_bindings_cls=_FakeKeyBindings,
            state=state,
            approval_mode_active_fn=lambda _s: False,
            toggle_approval_selection_fn=lambda _s: None,
            on_tab_non_approval_fn=lambda _evt: None,
        )
        self.assertIsNotNone(kb)
        assert kb is not None
        self.assertNotIn("y", kb.handlers)
        self.assertNotIn("n", kb.handlers)
        self.assertEqual(buffer.text, "")

    def test_enter_and_escape_resolve_in_approval_mode_preserves_existing_input(self) -> None:
        state = AppState()
        state.turn_phase = "approval"
        state.approval_selected_action = "allow"

        class _FakeKeyBindings:
            def __init__(self) -> None:
                self.handlers: dict[str, object] = {}

            def add(self, key: str, eager: bool = False):  # noqa: ANN001
                _ = eager

                def _decorator(fn):  # noqa: ANN001
                    self.handlers[key] = fn
                    return fn

                return _decorator

        class _FakeBuffer:
            def __init__(self) -> None:
                self.text = "pending text"
                self.validated = 0

            def insert_text(self, text: str) -> None:
                self.text += text

            def validate_and_handle(self) -> None:
                self.validated += 1

        buffer = _FakeBuffer()
        event = SimpleNamespace(app=SimpleNamespace(current_buffer=buffer))
        kb = prompt_keybindings.build_prompt_key_bindings(
            key_bindings_cls=_FakeKeyBindings,
            state=state,
            approval_mode_active_fn=lambda _s: True,
            toggle_approval_selection_fn=lambda _s: None,
            on_tab_non_approval_fn=lambda _evt: None,
        )
        self.assertIsNotNone(kb)
        assert kb is not None

        kb.handlers["escape"](event)
        self.assertEqual(state.approval_selected_action, "deny")
        self.assertEqual(buffer.text, "pending text")
        self.assertEqual(buffer.validated, 1)
        self.assertEqual(state.approval_flash_message, "Rejected")

        buffer.text = "more text"
        state.approval_selected_action = "allow"
        kb.handlers["enter"](event)
        self.assertEqual(state.approval_selected_action, "allow")
        self.assertEqual(buffer.text, "more text")
        self.assertEqual(buffer.validated, 2)
        self.assertEqual(state.approval_flash_message, "Approved")

        buffer.text = "typed but should reject"
        state.approval_selected_action = "deny"
        kb.handlers["enter"](event)
        self.assertEqual(buffer.text, "typed but should reject")
        self.assertEqual(buffer.validated, 3)
        self.assertEqual(state.approval_flash_message, "Rejected")

    def test_shift_enter_inserts_newline_in_non_approval_mode(self) -> None:
        state = AppState()

        class _FakeKeyBindings:
            def __init__(self) -> None:
                self.handlers: dict[str, object] = {}

            def add(self, key: str, eager: bool = False):  # noqa: ANN001
                _ = eager

                def _decorator(fn):  # noqa: ANN001
                    self.handlers[key] = fn
                    return fn

                return _decorator

        class _FakeBuffer:
            def __init__(self) -> None:
                self.text = "line1"

            def insert_text(self, text: str) -> None:
                self.text += text

        buffer = _FakeBuffer()
        event = SimpleNamespace(app=SimpleNamespace(current_buffer=buffer))
        kb = prompt_keybindings.build_prompt_key_bindings(
            key_bindings_cls=_FakeKeyBindings,
            state=state,
            approval_mode_active_fn=lambda _s: False,
            toggle_approval_selection_fn=lambda _s: None,
            on_tab_non_approval_fn=lambda _evt: None,
        )
        assert kb is not None
        self.assertIn("c-j", kb.handlers)
        kb.handlers["s-enter"](event)
        self.assertEqual(buffer.text, "line1\n")

    def test_shift_enter_falls_back_to_ctrl_j_when_key_is_unsupported(self) -> None:
        state = AppState()

        class _FakeKeyBindings:
            def __init__(self) -> None:
                self.handlers: dict[str, object] = {}

            def add(self, key: str, eager: bool = False):  # noqa: ANN001
                _ = eager
                if key == "s-enter":
                    raise ValueError("Invalid key: s-enter")

                def _decorator(fn):  # noqa: ANN001
                    self.handlers[key] = fn
                    return fn

                return _decorator

        class _FakeBuffer:
            def __init__(self) -> None:
                self.text = "line1"

            def insert_text(self, text: str) -> None:
                self.text += text

        buffer = _FakeBuffer()
        event = SimpleNamespace(app=SimpleNamespace(current_buffer=buffer))
        kb = prompt_keybindings.build_prompt_key_bindings(
            key_bindings_cls=_FakeKeyBindings,
            state=state,
            approval_mode_active_fn=lambda _s: False,
            toggle_approval_selection_fn=lambda _s: None,
            on_tab_non_approval_fn=lambda _evt: None,
        )
        assert kb is not None
        self.assertNotIn("s-enter", kb.handlers)
        self.assertIn("c-j", kb.handlers)
        kb.handlers["c-j"](event)
        self.assertEqual(buffer.text, "line1\n")


if __name__ == "__main__":
    unittest.main()
