"""Tests for the main application."""

from __future__ import annotations

import unittest
from unittest.mock import MagicMock, patch

from openjax_tui.app import OpenJaxApp
from openjax_tui.screens.chat import ChatScreen
from openjax_tui.state import TurnPhase
from textual.widgets import Input


class TestOpenJaxApp(unittest.TestCase):
    """Test the main application."""

    def test_app_can_be_instantiated(self) -> None:
        """Test that the app can be created."""
        app = OpenJaxApp()
        self.assertIsInstance(app, OpenJaxApp)
        self.assertEqual(app.TITLE, "OpenJax")
        self.assertEqual(app.SUB_TITLE, "AI Agent Framework")

    def test_app_has_state(self) -> None:
        """Test that the app has a state instance."""
        app = OpenJaxApp()
        self.assertIsNotNone(app.state)
        self.assertEqual(app.state.turn_phase, TurnPhase.IDLE)

    def test_app_reactive_state(self) -> None:
        """Test that the app has reactive state variables."""
        app = OpenJaxApp()
        self.assertIsNone(app.session_id)
        self.assertEqual(app.turn_phase, TurnPhase.IDLE)
        self.assertEqual(app.current_input, "")

    def test_action_exit(self) -> None:
        """Test the exit action."""
        app = OpenJaxApp()
        app.exit = MagicMock()
        app.action_exit()
        app.exit.assert_called_once()

    @patch.object(OpenJaxApp, "screen", new_callable=lambda: MagicMock(spec=ChatScreen))
    def test_submit_message_updates_state(self, _mock_screen) -> None:
        """submit_message should append user message and queue async submit."""
        app = OpenJaxApp()
        captured = {"count": 0}

        def consume(coro) -> None:
            captured["count"] += 1
            coro.close()

        app._spawn_task = consume

        app.submit_message("Hello")

        self.assertEqual(len(app.state.messages), 1)
        self.assertEqual(app.state.messages[0].role, "user")
        self.assertEqual(app.state.messages[0].content, "Hello")
        self.assertEqual(app.state.turn_phase, TurnPhase.THINKING)
        self.assertEqual(captured["count"], 1)

    def test_action_pending_no_approvals(self) -> None:
        """Test the pending action with no approvals."""
        app = OpenJaxApp()
        app._render_state = MagicMock()

        app.action_pending()

        self.assertIn("没有", app.state.messages[-1].content)

    def test_action_pending_with_approvals(self) -> None:
        """Test the pending action with approvals."""
        app = OpenJaxApp()
        app._render_state = MagicMock()
        app.state.add_approval("app-1", "write_file", turn_id="turn-1")

        app.action_pending()

        text = app.state.messages[-1].content
        self.assertIn("app-1", text)
        self.assertIn("turn-1", text)


class TestChatScreen(unittest.TestCase):
    """Test the chat screen."""

    def test_screen_can_be_instantiated(self) -> None:
        """Test that the chat screen can be created."""
        screen = ChatScreen()
        self.assertIsInstance(screen, ChatScreen)

    def test_screen_has_bindings(self) -> None:
        """Test that the screen has key bindings."""
        screen = ChatScreen()
        self.assertTrue(hasattr(screen, "BINDINGS"))
        self.assertEqual(len(screen.BINDINGS), 1)

    def test_screen_has_message_methods(self) -> None:
        """Test that the screen has message display methods."""
        screen = ChatScreen()
        self.assertTrue(hasattr(screen, "add_user_message"))
        self.assertTrue(hasattr(screen, "add_assistant_message"))
        self.assertTrue(hasattr(screen, "add_system_message"))
        self.assertTrue(hasattr(screen, "clear_messages"))

    def test_screen_has_command_palette_method(self) -> None:
        """Test that the screen has command palette method."""
        screen = ChatScreen()
        self.assertTrue(hasattr(screen, "show_command_palette"))

    def test_on_key_moves_palette_selection_down(self) -> None:
        """Down key should move command selection when slash mode is active."""
        screen = ChatScreen()
        chat_input = MagicMock(spec=Input)
        chat_input.value = "/he"
        palette = MagicMock()

        def query_one(selector, *_args, **_kwargs):
            if selector == "#chat-input":
                return chat_input
            if selector == "#command-palette":
                return palette
            raise Exception("not found")

        screen.query_one = MagicMock(side_effect=query_one)

        event = MagicMock()
        event.key = "down"

        screen.on_key(event)

        palette.move_selection.assert_called_once_with(1)
        event.stop.assert_called_once()


class TestCommands(unittest.TestCase):
    """Test the commands module."""

    def test_create_commands(self) -> None:
        """Test that create_commands returns expected commands."""
        from openjax_tui.commands import create_commands

        app = OpenJaxApp()
        commands = create_commands(app)

        command_names = [cmd.name for cmd in commands]
        self.assertIn("help", command_names)
        self.assertIn("clear", command_names)
        self.assertIn("exit", command_names)
        self.assertIn("pending", command_names)

    def test_command_handlers(self) -> None:
        """Test that command handlers work."""
        from openjax_tui.commands import create_commands

        app = OpenJaxApp()
        app.exit = MagicMock()
        app._render_state = MagicMock()

        commands = create_commands(app)

        exit_cmd = next(cmd for cmd in commands if cmd.name == "exit")
        exit_cmd.handler()
        app.exit.assert_called_once()


if __name__ == "__main__":
    unittest.main()
