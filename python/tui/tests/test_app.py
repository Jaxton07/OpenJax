"""Tests for the main application."""

from __future__ import annotations

import unittest

from textual.widgets import Input, RichLog

from openjax_tui.app import OpenJaxApp
from openjax_tui.screens.chat import ChatScreen


class TestOpenJaxApp(unittest.TestCase):
    """Test the main application."""

    def test_app_can_be_instantiated(self) -> None:
        """Test that the app can be created."""
        app = OpenJaxApp()
        self.assertIsInstance(app, OpenJaxApp)
        self.assertEqual(app.TITLE, "OpenJax")
        self.assertEqual(app.SUB_TITLE, "AI Agent Framework")


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


if __name__ == "__main__":
    unittest.main()
