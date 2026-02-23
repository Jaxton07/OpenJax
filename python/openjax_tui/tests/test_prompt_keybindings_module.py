from __future__ import annotations

import unittest

from openjax_tui import prompt_keybindings
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


if __name__ == "__main__":
    unittest.main()
