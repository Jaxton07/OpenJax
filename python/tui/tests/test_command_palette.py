"""Tests for command palette widget."""

from __future__ import annotations

import unittest
from unittest.mock import MagicMock

from openjax_tui.widgets.command_palette import Command, CommandPalette


class TestCommand(unittest.TestCase):
    """Test Command dataclass."""

    def test_command_creation(self) -> None:
        """Test creating a Command."""
        handler = MagicMock()
        cmd = Command(name="test", description="Test command", handler=handler)

        self.assertEqual(cmd.name, "test")
        self.assertEqual(cmd.description, "Test command")
        self.assertEqual(cmd.handler, handler)
        self.assertIsNone(cmd.shortcut)

    def test_command_with_shortcut(self) -> None:
        """Test creating a Command with shortcut."""
        handler = MagicMock()
        cmd = Command(
            name="exit",
            description="Exit program",
            handler=handler,
            shortcut="ctrl+q",
        )

        self.assertEqual(cmd.shortcut, "ctrl+q")


class TestCommandPalette(unittest.TestCase):
    """Test CommandPalette widget."""

    def test_palette_creation_empty(self) -> None:
        """Test creating palette with no commands."""
        palette = CommandPalette()
        self.assertEqual(palette.commands, [])
        self.assertEqual(palette.filtered_commands, [])
        self.assertEqual(palette.query, "")

    def test_palette_creation_with_commands(self) -> None:
        """Test creating palette with commands."""
        commands = [
            Command(name="help", description="Show help", handler=MagicMock()),
            Command(name="exit", description="Exit", handler=MagicMock()),
        ]
        palette = CommandPalette(commands=commands)

        self.assertEqual(len(palette.commands), 2)
        self.assertEqual(len(palette.filtered_commands), 2)

    def test_filter_commands_empty_query(self) -> None:
        """Test filtering with empty query returns all commands."""
        commands = [
            Command(name="help", description="Show help", handler=MagicMock()),
            Command(name="exit", description="Exit", handler=MagicMock()),
        ]
        palette = CommandPalette(commands=commands)

        palette.filter_commands("")
        self.assertEqual(len(palette.filtered_commands), 2)

    def test_filter_commands_by_name(self) -> None:
        """Test filtering commands by name."""
        commands = [
            Command(name="help", description="Show help", handler=MagicMock()),
            Command(name="exit", description="Exit", handler=MagicMock()),
            Command(name="clear", description="Clear screen", handler=MagicMock()),
        ]
        palette = CommandPalette(commands=commands)

        palette.filter_commands("he")
        self.assertEqual(len(palette.filtered_commands), 1)
        self.assertEqual(palette.filtered_commands[0].name, "help")

    def test_filter_commands_by_description(self) -> None:
        """Test filtering commands by description."""
        commands = [
            Command(name="help", description="Show help", handler=MagicMock()),
            Command(name="exit", description="Exit program", handler=MagicMock()),
        ]
        palette = CommandPalette(commands=commands)

        palette.filter_commands("program")
        self.assertEqual(len(palette.filtered_commands), 1)
        self.assertEqual(palette.filtered_commands[0].name, "exit")

    def test_filter_commands_case_insensitive(self) -> None:
        """Test filtering is case insensitive."""
        commands = [
            Command(name="Help", description="Show Help", handler=MagicMock()),
        ]
        palette = CommandPalette(commands=commands)

        palette.filter_commands("help")
        self.assertEqual(len(palette.filtered_commands), 1)

        palette.filter_commands("HELP")
        self.assertEqual(len(palette.filtered_commands), 1)

    def test_filter_commands_no_match(self) -> None:
        """Test filtering with no matches."""
        commands = [
            Command(name="help", description="Show help", handler=MagicMock()),
        ]
        palette = CommandPalette(commands=commands)

        palette.filter_commands("xyz")
        self.assertEqual(len(palette.filtered_commands), 0)

    def test_filter_commands_partial_match(self) -> None:
        """Test filtering with partial matches."""
        commands = [
            Command(name="help", description="Show help", handler=MagicMock()),
            Command(name="hello", description="Say hello", handler=MagicMock()),
            Command(name="exit", description="Exit", handler=MagicMock()),
        ]
        palette = CommandPalette(commands=commands)

        palette.filter_commands("he")
        self.assertEqual(len(palette.filtered_commands), 2)
        self.assertIn("help", [cmd.name for cmd in palette.filtered_commands])
        self.assertIn("hello", [cmd.name for cmd in palette.filtered_commands])

    def test_execute_command_valid_index(self) -> None:
        """Test executing command with valid index."""
        handler = MagicMock()
        commands = [
            Command(name="test", description="Test command", handler=handler),
        ]
        palette = CommandPalette(commands=commands)

        # Mock dismiss method
        palette.dismiss = MagicMock()

        palette.execute_command(0)
        handler.assert_called_once()
        palette.dismiss.assert_called_once()

    def test_execute_command_invalid_index(self) -> None:
        """Test executing command with invalid index."""
        handler = MagicMock()
        commands = [
            Command(name="test", description="Test command", handler=handler),
        ]
        palette = CommandPalette(commands=commands)

        # Mock dismiss method
        palette.dismiss = MagicMock()

        palette.execute_command(5)  # Out of range
        handler.assert_not_called()
        palette.dismiss.assert_not_called()

    def test_execute_command_negative_index(self) -> None:
        """Test executing command with negative index."""
        handler = MagicMock()
        commands = [
            Command(name="test", description="Test command", handler=handler),
        ]
        palette = CommandPalette(commands=commands)

        # Mock dismiss method
        palette.dismiss = MagicMock()

        palette.execute_command(-1)
        handler.assert_not_called()
        palette.dismiss.assert_not_called()

    def test_dismiss_message(self) -> None:
        """Test that dismiss posts Dismissed message."""
        palette = CommandPalette()

        # Track posted messages
        posted_messages = []
        original_post = palette.post_message

        def mock_post_message(msg):
            posted_messages.append(type(msg).__name__)
            return original_post(msg)

        palette.post_message = mock_post_message

        palette.dismiss()

        self.assertIn("Dismissed", posted_messages)


if __name__ == "__main__":
    unittest.main()
