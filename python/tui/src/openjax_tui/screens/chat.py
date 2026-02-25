"""Chat screen for OpenJax TUI."""

from __future__ import annotations

import logging
import sys

from textual.app import ComposeResult
from textual.containers import Vertical
from textual.events import Key
from textual.screen import Screen
from textual.widgets import Footer, Header, Input, RichLog

from ..commands import create_commands
from ..widgets.command_palette import CommandPalette

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
        yield Header()
        with Vertical(id="chat-container"):
            yield RichLog(id="chat-log", markup=True)
            yield Input(
                placeholder="输入消息按回车发送，/ 打开命令面板...",
                id="chat-input",
            )
        yield Footer()

    def on_mount(self) -> None:
        """Called when screen is mounted."""
        logger.info("chat_screen mounted")
        log = self.query_one("#chat-log", RichLog)
        log.write("[bold green]欢迎使用 OpenJax TUI![/bold green]")
        # Show platform-specific quit key
        quit_key = "Ctrl+C" if sys.platform == "darwin" else "Ctrl+Q"
        log.write(f"输入消息按回车发送，输入 / 打开命令面板，{quit_key} 退出。\n")

    def on_input_changed(self, event: Input.Changed) -> None:
        """Handle input changes to detect '/' for command palette."""
        value = event.value
        if value.startswith("/"):
            logger.info("input_changed slash query=%s", value)
            palette = self.show_command_palette()
            palette.refresh_commands(value[1:])
            return
        self.dismiss_command_palette()

    def on_input_submitted(self, event: Input.Submitted) -> None:
        """Handle input submission."""
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
        """Handle up/down keys for command candidate navigation."""
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
        try:
            existing = self.query_one("#command-palette", CommandPalette)
            return existing
        except Exception:
            pass

        commands = create_commands(self.app)
        palette = CommandPalette(commands=commands, id="command-palette")
        container = self.query_one("#chat-container", Vertical)
        input_widget = self.query_one("#chat-input", Input)
        container.mount(palette, before=input_widget)
        logger.info("command_palette shown")
        return palette

    def dismiss_command_palette(self) -> None:
        """Dismiss command palette if visible."""
        try:
            palette = self.query_one("#command-palette", CommandPalette)
        except Exception:
            return
        palette.remove()
        logger.info("command_palette dismissed")

    def on_command_palette_dismissed(self, event: CommandPalette.Dismissed) -> None:
        """Handle command palette dismissal."""
        self.dismiss_command_palette()
        self.query_one("#chat-input", Input).focus()

    def add_user_message(self, text: str) -> None:
        """Add a user message to the chat log.

        Args:
            text: The message text
        """
        log = self.query_one("#chat-log", RichLog)
        log.write(f"[bold blue]你:[/bold blue] {text}")

    def add_assistant_message(self, text: str) -> None:
        """Add an assistant message to the chat log.

        Args:
            text: The message text
        """
        log = self.query_one("#chat-log", RichLog)
        log.write(f"[bold green]助手:[/bold green] {text}")

    def add_system_message(self, text: str) -> None:
        """Add a system message to the chat log.

        Args:
            text: The message text (can include Rich markup)
        """
        log = self.query_one("#chat-log", RichLog)
        log.write(text)

    def clear_messages(self) -> None:
        """Clear all messages from the chat log."""
        log = self.query_one("#chat-log", RichLog)
        log.clear()
