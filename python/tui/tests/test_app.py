"""Tests for the main application."""

from __future__ import annotations

import unittest
from unittest.mock import AsyncMock, MagicMock, patch

from openjax_tui.app import OpenJaxApp
from openjax_tui.event_mapper import UiOperation
from openjax_tui.screens.chat import ChatScreen
from openjax_tui.state import Message, TurnPhase
from openjax_tui.widgets.approval_popup import ApprovalPopup
from rich.markdown import Markdown
from textual.widgets import Input


class TestOpenJaxApp(unittest.TestCase):
    """Test the main application."""

    def test_app_can_be_instantiated(self) -> None:
        """Test that the app can be created."""
        app = OpenJaxApp()
        self.assertIsInstance(app, OpenJaxApp)
        self.assertEqual(app.TITLE, "OpenJax")
        self.assertEqual(app.SUB_TITLE, "AI Agent Framework")
        self.assertFalse(app.ENABLE_COMMAND_PALETTE)

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

    def test_action_approve_no_approvals(self) -> None:
        """Test approve action when no pending approvals."""
        app = OpenJaxApp()
        app._render_state = MagicMock()

        app.action_approve()

        self.assertIn("没有", app.state.messages[-1].content)

    def test_action_approve_submits_resolution(self) -> None:
        """Approve action should submit focused approval to runtime."""
        app = OpenJaxApp()
        app._render_state = MagicMock()
        app.state.add_approval("app-1", "write_file", turn_id="turn-1")
        runtime = MagicMock()
        runtime.resolve_approval = AsyncMock(return_value=True)
        app._runtime = runtime

        app.action_approve()

        runtime.resolve_approval.assert_awaited_once_with(
            turn_id="turn-1",
            request_id="app-1",
            approved=True,
        )

    def test_action_deny_submits_resolution(self) -> None:
        """Deny action should submit focused approval to runtime."""
        app = OpenJaxApp()
        app._render_state = MagicMock()
        app.state.add_approval("app-1", "write_file", turn_id="turn-1")
        runtime = MagicMock()
        runtime.resolve_approval = AsyncMock(return_value=True)
        app._runtime = runtime

        app.action_deny()

        runtime.resolve_approval.assert_awaited_once_with(
            turn_id="turn-1",
            request_id="app-1",
            approved=False,
        )

    def test_apply_ui_operations_rerenders_on_tool_call_completed(self) -> None:
        app = OpenJaxApp()
        app._render_state = MagicMock()

        app._apply_ui_operations([UiOperation(kind="tool_call_completed")])

        app._render_state.assert_called_once()

    def test_apply_ui_operations_syncs_popup_on_approval_change(self) -> None:
        app = OpenJaxApp()
        app._render_state = MagicMock()
        app._sync_approval_popup = MagicMock()

        app._apply_ui_operations([UiOperation(kind="approval_added", request_id="r1")])

        app._sync_approval_popup.assert_called_once()

    def test_turn_completed_message_defaults_to_markdown_render_kind(self) -> None:
        app = OpenJaxApp()
        app._render_state = MagicMock()
        app.state.turn_render_kind_by_turn["t1"] = "markdown"

        app._apply_ui_operations([UiOperation(kind="turn_completed", turn_id="t1", text="# title")])

        self.assertEqual(app.state.messages[-1].role, "assistant")
        self.assertEqual(app.state.messages[-1].metadata["render_kind"], "markdown")

    def test_handle_popup_selection_cancel_does_not_call_runtime(self) -> None:
        app = OpenJaxApp()
        app._render_state = MagicMock()
        app._hide_approval_popup = MagicMock()
        runtime = MagicMock()
        runtime.resolve_approval = AsyncMock(return_value=True)
        app._runtime = runtime

        app.handle_approval_popup_selection("cancel")

        app._hide_approval_popup.assert_called_once()
        runtime.resolve_approval.assert_not_called()

    def test_show_approval_popup_uses_focus_request(self) -> None:
        app = OpenJaxApp()
        screen = MagicMock()
        app._get_chat_screen = MagicMock(return_value=screen)
        app.state.add_approval("a1", "shell", turn_id="t1")
        app.state.add_approval("a2", "read_file", turn_id="t2")
        app.state.approval_focus_id = "a1"

        app._show_approval_popup_for_focus()

        shown = screen.show_approval_popup.call_args[0][0]
        self.assertEqual(shown.id, "a1")


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
        self.assertTrue(hasattr(screen, "focus_chat_input"))

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

    def test_on_key_prioritizes_approval_popup(self) -> None:
        """Approval popup should consume up/down before command palette."""
        screen = ChatScreen()
        popup = MagicMock(spec=ApprovalPopup)

        def query_one(selector, *_args, **_kwargs):
            if selector == "#approval-popup":
                return popup
            raise Exception("not found")

        screen.query_one = MagicMock(side_effect=query_one)
        event = MagicMock()
        event.key = "down"

        screen.on_key(event)

        popup.move_selection.assert_called_once_with(1)
        event.stop.assert_called_once()

    def test_write_message_renders_failed_tool_status_in_red(self) -> None:
        log = MagicMock()
        msg = Message(
            role="tool",
            content="Run shell command",
            metadata={
                "ok": False,
                "tool_name": "shell",
                "output_preview": "permission denied",
                "render_kind": "plain",
            },
        )

        ChatScreen._write_message(log, msg)

        log.write.assert_any_call("[bold red]⏺[/bold red] Run shell command")

    def test_write_message_renders_tool_target_suffix(self) -> None:
        log = MagicMock()
        msg = Message(
            role="tool",
            content="Update 1 file",
            metadata={"ok": True, "target": "test.txt", "render_kind": "plain"},
        )

        ChatScreen._write_message(log, msg)

        log.write.assert_any_call("[bold green]⏺[/bold green] Update 1 file (test.txt)")

    def test_focus_chat_input_focuses_input_widget(self) -> None:
        screen = ChatScreen()
        chat_input = MagicMock(spec=Input)
        screen.has_approval_popup = MagicMock(return_value=False)
        screen.query_one = MagicMock(return_value=chat_input)

        screen.focus_chat_input()

        chat_input.focus.assert_called_once()

    def test_show_command_palette_requires_slash_mode(self) -> None:
        screen = ChatScreen()
        chat_input = MagicMock(spec=Input)
        chat_input.value = "hello"
        screen.has_approval_popup = MagicMock(return_value=False)
        screen.dismiss_command_palette = MagicMock()

        def query_one(selector, *_args, **_kwargs):
            if selector == "#chat-input":
                return chat_input
            raise Exception("not found")

        screen.query_one = MagicMock(side_effect=query_one)

        with self.assertRaises(RuntimeError):
            screen.show_command_palette()

        screen.dismiss_command_palette.assert_called_once()

    def test_write_message_renders_assistant_markdown(self) -> None:
        log = MagicMock()
        msg = Message(role="assistant", content="# Heading", metadata={"render_kind": "markdown"})

        ChatScreen._write_message(log, msg)

        self.assertIsInstance(log.write.call_args_list[0].args[0], Markdown)


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
        self.assertIn("approve", command_names)
        self.assertIn("deny", command_names)

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
