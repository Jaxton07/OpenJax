"""Tests for the main application."""

from __future__ import annotations

import unittest
from unittest.mock import MagicMock, patch

from openjax_tui.app import OpenJaxApp
from openjax_tui.screens.chat import ChatScreen
from openjax_tui.state import TurnPhase


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

    @patch.object(OpenJaxApp, 'screen', new_callable=lambda: MagicMock(spec=ChatScreen))
    def test_submit_message_updates_state(self, mock_screen) -> None:
        """Test that submit_message updates state."""
        app = OpenJaxApp()
        mock_screen.add_user_message = MagicMock()
        mock_screen.add_assistant_message = MagicMock()

        app.submit_message("Hello")

        # Check state was updated
        self.assertEqual(len(app.state.messages), 2)  # user + assistant
        self.assertEqual(app.state.messages[0].role, "user")
        self.assertEqual(app.state.messages[0].content, "Hello")
        self.assertEqual(app.state.messages[1].role, "assistant")

    @patch.object(OpenJaxApp, 'screen', new_callable=lambda: MagicMock(spec=ChatScreen))
    def test_action_clear(self, mock_screen) -> None:
        """Test the clear action."""
        app = OpenJaxApp()
        # Add some messages
        app.state.add_message("user", "Hello")
        app.state.add_message("assistant", "Hi")
        self.assertEqual(len(app.state.messages), 2)

        mock_screen.clear_messages = MagicMock()
        mock_screen.add_system_message = MagicMock()

        app.action_clear()

        # Check messages cleared
        self.assertEqual(len(app.state.messages), 0)
        mock_screen.clear_messages.assert_called_once()

    @patch.object(OpenJaxApp, 'screen', new_callable=lambda: MagicMock(spec=ChatScreen))
    def test_action_pending_no_approvals(self, mock_screen) -> None:
        """Test the pending action with no approvals."""
        app = OpenJaxApp()
        mock_screen.add_system_message = MagicMock()

        app.action_pending()

        mock_screen.add_system_message.assert_called_once()
        call_args = mock_screen.add_system_message.call_args[0][0]
        self.assertIn("没有", call_args)

    @patch.object(OpenJaxApp, 'screen', new_callable=lambda: MagicMock(spec=ChatScreen))
    def test_action_pending_with_approvals(self, mock_screen) -> None:
        """Test the pending action with approvals."""
        app = OpenJaxApp()
        app.state.add_approval("app-1", "write_file")

        mock_screen.add_system_message = MagicMock()

        app.action_pending()

        call_args = mock_screen.add_system_message.call_args[0][0]
        self.assertIn("1", call_args)


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


class TestCommands(unittest.TestCase):
    """Test the commands module."""

    def test_create_commands(self) -> None:
        """Test that create_commands returns expected commands."""
        from openjax_tui.commands import create_commands

        app = OpenJaxApp()
        commands = create_commands(app)

        # Check we have the expected commands
        command_names = [cmd.name for cmd in commands]
        self.assertIn("help", command_names)
        self.assertIn("clear", command_names)
        self.assertIn("exit", command_names)
        self.assertIn("pending", command_names)

    @patch.object(OpenJaxApp, 'screen', new_callable=lambda: MagicMock(spec=ChatScreen))
    def test_command_handlers(self, mock_screen) -> None:
        """Test that command handlers work."""
        from openjax_tui.commands import create_commands

        app = OpenJaxApp()
        app.exit = MagicMock()
        mock_screen.clear_messages = MagicMock()
        mock_screen.add_system_message = MagicMock()

        commands = create_commands(app)

        # Find and test exit command
        exit_cmd = next(cmd for cmd in commands if cmd.name == "exit")
        exit_cmd.handler()
        app.exit.assert_called_once()


if __name__ == "__main__":
    unittest.main()
