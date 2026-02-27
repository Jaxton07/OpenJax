"""Chat screen for OpenJax TUI."""

from __future__ import annotations

import logging
import sys
from typing import TYPE_CHECKING

from textual.app import ComposeResult
from textual.containers import Vertical
from textual.events import Key
from textual.screen import Screen
from textual.widgets import Input, RichLog

from ..commands import create_commands
from ..state import TurnPhase
from ..widgets.approval_popup import ApprovalPopup
from ..widgets.chat_input import ChatInput
from ..widgets.command_palette import CommandPalette
from ..widgets.markdown_message import MarkdownMessage
from ..widgets.thinking_status import ThinkingStatus

if TYPE_CHECKING:
    from ..state import AppState, ApprovalRequest, Message

logger = logging.getLogger("openjax_tui")


def get_quit_key() -> str:
    """Get platform-specific quit key."""
    return "ctrl+c" if sys.platform == "darwin" else "ctrl+q"


class ChatScreen(Screen):
    """Main chat interface screen."""

    # Platform-specific bindings
    BINDINGS = [
        (get_quit_key(), "quit", "退出"),
    ]

    def compose(self) -> ComposeResult:
        """Compose the chat screen layout."""
        with Vertical(id="chat-container"):
            yield RichLog(id="chat-log", markup=True, wrap=True)
            yield ChatInput(
                placeholder="输入消息按回车发送，/ 打开命令面板...",
                id="chat-input",
            )

    def on_mount(self) -> None:
        """Called when screen is mounted."""
        logger.info("chat_screen mounted")
        log = self.query_one("#chat-log", RichLog)
        log.write("[bold green]欢迎使用 OpenJax TUI![/bold green]")
        # Show platform-specific quit key
        quit_key = "Ctrl+C" if sys.platform == "darwin" else "Ctrl+Q"
        log.write(f"输入消息按回车发送，输入 / 打开命令面板，{quit_key} 退出。\n")
        self.call_after_refresh(self.focus_chat_input)

    def focus_chat_input(self) -> None:
        """Set focus to chat input when no approval popup is active."""
        if self.has_approval_popup():
            return
        try:
            self.query_one("#chat-input", Input).focus()
        except Exception:
            return

    def on_input_changed(self, event: Input.Changed) -> None:
        """Handle input changes to detect '/' for command palette."""
        if self.has_approval_popup():
            return

        value = event.value
        if value.startswith("/"):
            logger.info("input_changed slash query=%s", value)
            palette = self.show_command_palette()
            palette.refresh_commands(value[1:])
            return
        self.dismiss_command_palette()

    def on_input_submitted(self, event: Input.Submitted) -> None:
        """Handle input submission."""
        if self.has_approval_popup():
            return

        value = event.value.strip()
        if not value:
            return

        if value.startswith("/"):
            logger.info("slash_command submitted query=%s", value)
            try:
                palette = self.query_one("#command-palette", CommandPalette)
            except Exception:
                palette = self.show_command_palette()
                palette.refresh_commands(value[1:])

            try:
                if palette.execute_best_match():
                    self.query_one("#chat-input", Input).value = ""
                    logger.info("slash_command executed query=%s", value)
                else:
                    self.add_system_message(f"[yellow]未找到命令: {value}[/yellow]")
                    logger.info("slash_command no_match query=%s", value)
            except Exception:
                logger.exception("slash_command execution failed query=%s", value)
                self.add_system_message("[red]命令执行失败，请查看日志[/red]")
            return

        logger.info("chat submit text_len=%s", len(value))
        self.app.submit_message(value)
        self.query_one("#chat-input", Input).value = ""

    def on_key(self, event: Key) -> None:
        """Handle approval popup and command candidate keyboard navigation."""
        if self.has_approval_popup():
            popup = self.query_one("#approval-popup", ApprovalPopup)
            if event.key == "up":
                popup.move_selection(-1)
                event.stop()
                return
            if event.key == "down":
                popup.move_selection(1)
                event.stop()
                return
            if event.key == "enter":
                popup.confirm_selection()
                event.stop()
                return
            if event.key == "escape":
                popup.dismiss()
                event.stop()
                return

        if event.key not in {"up", "down"}:
            return

        try:
            chat_input = self.query_one("#chat-input", Input)
            palette = self.query_one("#command-palette", CommandPalette)
        except Exception:
            return

        if not chat_input.value.startswith("/"):
            return

        direction = 1 if event.key == "down" else -1
        palette.move_selection(direction)
        event.stop()

    def show_command_palette(self) -> CommandPalette:
        """Show the command palette overlay."""
        if self.has_approval_popup():
            raise RuntimeError("approval popup is active")
        if not self._is_slash_mode():
            self.dismiss_command_palette()
            raise RuntimeError("command palette requires slash input mode")

        try:
            existing = self.query_one("#command-palette", CommandPalette)
            self.dismiss_thinking_status()
            return existing
        except Exception:
            pass

        commands = create_commands(self.app)
        palette = CommandPalette(commands=commands, id="command-palette")
        container = self.query_one("#chat-container", Vertical)
        input_widget = self.query_one("#chat-input", Input)
        container.mount(palette, before=input_widget)
        self.dismiss_thinking_status()
        logger.info("command_palette shown")
        return palette

    def dismiss_command_palette(self) -> None:
        """Dismiss command palette if visible."""
        try:
            palette = self.query_one("#command-palette", CommandPalette)
        except Exception:
            return
        palette.remove()
        phase = self._current_turn_phase()
        if phase is not None:
            self.sync_thinking_status(phase)
        logger.info("command_palette dismissed")

    def _is_slash_mode(self) -> bool:
        """Return true when input starts with slash command trigger."""
        try:
            chat_input = self.query_one("#chat-input", Input)
        except Exception:
            return False
        return chat_input.value.startswith("/")

    def has_approval_popup(self) -> bool:
        """Return whether approval popup is currently mounted."""
        try:
            self.query_one("#approval-popup", ApprovalPopup)
            return True
        except Exception:
            return False

    def has_command_palette(self) -> bool:
        """Return whether command palette is currently mounted."""
        try:
            self.query_one("#command-palette", CommandPalette)
            return True
        except Exception:
            return False

    def show_approval_popup(self, approval: "ApprovalRequest") -> ApprovalPopup:
        """Show or update approval popup above the input widget."""
        summary = ApprovalPopup.format_summary(
            approval_id=approval.id,
            action=approval.action,
            turn_id=approval.turn_id,
            reason=approval.reason,
        )

        self.dismiss_thinking_status()
        self.dismiss_command_palette()
        self.set_input_enabled(False)
        try:
            popup = self.query_one("#approval-popup", ApprovalPopup)
            popup.set_summary(summary)
            popup.focus()
            return popup
        except Exception:
            pass

        popup = ApprovalPopup(id="approval-popup")
        popup.set_summary(summary)
        container = self.query_one("#chat-container", Vertical)
        input_widget = self.query_one("#chat-input", Input)
        container.mount(popup, before=input_widget)
        popup.focus()
        logger.info("approval_popup shown approval_id=%s", approval.id)
        return popup

    def dismiss_approval_popup(self) -> None:
        """Dismiss approval popup and restore input state."""
        try:
            popup = self.query_one("#approval-popup", ApprovalPopup)
        except Exception:
            self.set_input_enabled(True)
        else:
            popup.remove()
            self.set_input_enabled(True)
        phase = self._current_turn_phase()
        if phase is not None:
            self.sync_thinking_status(phase)
        logger.info("approval_popup dismissed")

    def set_input_enabled(self, enabled: bool) -> None:
        """Enable or disable chat input and restore focus when enabled."""
        chat_input = self.query_one("#chat-input", Input)
        chat_input.disabled = not enabled
        if enabled:
            self.focus_chat_input()

    def on_command_palette_dismissed(self, event: CommandPalette.Dismissed) -> None:
        """Handle command palette dismissal."""
        self.dismiss_command_palette()
        phase = self._current_turn_phase()
        if phase is not None:
            self.sync_thinking_status(phase)
        self.query_one("#chat-input", Input).focus()

    def on_approval_popup_selection_confirmed(
        self, event: ApprovalPopup.SelectionConfirmed
    ) -> None:
        """Handle approval popup selection confirmation."""
        self.app.handle_approval_popup_selection(event.option_name)

    def on_approval_popup_dismissed(self, event: ApprovalPopup.Dismissed) -> None:
        """Treat popup dismiss as cancel."""
        self.app.handle_approval_popup_selection("cancel")

    def add_user_message(self, text: str) -> None:
        """Add a user message to the chat log.

        Args:
            text: The message text
        """
        log = self.query_one("#chat-log", RichLog)
        self._write_spaced_line(log, f"[on #3a3a3a][bold blue]❯[/bold blue] {text}[/on #3a3a3a]")

    def add_assistant_message(self, text: str) -> None:
        """Add an assistant message to the chat log.

        Args:
            text: The message text
        """
        log = self.query_one("#chat-log", RichLog)
        self._write_spaced_line(log, MarkdownMessage(text).to_renderable())

    def add_system_message(self, text: str) -> None:
        """Add a system message to the chat log.

        Args:
            text: The message text (can include Rich markup)
        """
        log = self.query_one("#chat-log", RichLog)
        self._write_spaced_line(log, text)

    def clear_messages(self) -> None:
        """Clear all messages from the chat log."""
        log = self.query_one("#chat-log", RichLog)
        log.clear()

    def show_thinking_status(self) -> ThinkingStatus:
        """Show or return thinking indicator above chat input."""
        try:
            existing = self.query_one("#thinking-status", ThinkingStatus)
            return existing
        except Exception:
            pass

        status = ThinkingStatus(id="thinking-status")
        container = self.query_one("#chat-container", Vertical)
        input_widget = self.query_one("#chat-input", Input)
        container.mount(status, before=input_widget)
        return status

    def dismiss_thinking_status(self) -> None:
        """Dismiss thinking indicator if visible."""
        try:
            status = self.query_one("#thinking-status", ThinkingStatus)
        except Exception:
            return
        status.remove()

    def sync_thinking_status(self, phase: TurnPhase) -> None:
        """Sync thinking indicator visibility with current UI state."""
        if phase != TurnPhase.THINKING:
            self.dismiss_thinking_status()
            return
        if self.has_approval_popup() or self.has_command_palette():
            self.dismiss_thinking_status()
            return
        self.show_thinking_status()

    def _current_turn_phase(self) -> TurnPhase | None:
        """Best-effort turn phase lookup from app context."""
        try:
            phase = getattr(self.app, "turn_phase", None)
        except Exception:
            return None
        return phase

    def render_state(self, state: "AppState") -> None:
        """Render full chat view from application state."""
        try:
            log = self.query_one("#chat-log", RichLog)
        except Exception:
            # During early mount/unmount cycles the log widget may not exist yet.
            return
        log.clear()

        if not state.messages:
            log.write("[bold green]欢迎使用 OpenJax TUI![/bold green]")
            quit_key = "Ctrl+C" if sys.platform == "darwin" else "Ctrl+Q"
            log.write(f"输入消息按回车发送，输入 / 打开命令面板，{quit_key} 退出。\n")
            self.sync_thinking_status(state.turn_phase)
            return

        for msg in state.messages:
            self._write_message(log, msg)

        if state.active_turn_id:
            stream_text = state.stream_text_by_turn.get(state.active_turn_id, "")
            if stream_text:
                self._write_spaced_line(log, f"[bold green]⏺[/bold green] {stream_text}")

        self.sync_thinking_status(state.turn_phase)

    @staticmethod
    def _write_message(log: RichLog, msg: "Message") -> None:
        if msg.role == "user":
            ChatScreen._write_spaced_line(
                log,
                f"[on #3a3a3a][bold blue]❯[/bold blue] {msg.content}[/on #3a3a3a]",
            )
        elif msg.role == "assistant":
            render_kind = str(msg.metadata.get("render_kind", "plain"))
            if render_kind == "markdown":
                ChatScreen._write_spaced_line(log, MarkdownMessage(msg.content).to_renderable())
            else:
                ChatScreen._write_spaced_line(log, f"[bold green]⏺[/bold green] {msg.content}")
        elif msg.role == "tool":
            ok = bool(msg.metadata.get("ok", False))
            color = "green" if ok else "red"
            target = msg.metadata.get("target")
            suffix = f" ({target})" if target else ""
            ChatScreen._write_spaced_line(
                log,
                f"[bold {color}]⏺[/bold {color}] {msg.content}{suffix}",
            )
        else:
            ChatScreen._write_spaced_line(log, msg.content)

    @staticmethod
    def _write_spaced_line(log: RichLog, value) -> None:
        """Write one line and add a blank line after it for readability."""
        log.write(value)
        log.write("")
