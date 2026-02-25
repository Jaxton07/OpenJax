"""Chat screen for OpenJax TUI."""

from __future__ import annotations

from textual.app import ComposeResult
from textual.containers import Vertical
from textual.screen import Screen
from textual.widgets import Footer, Header, Input, RichLog


class ChatScreen(Screen):
    """Main chat interface screen."""

    BINDINGS = [
        ("ctrl+q", "quit", "退出"),
        ("ctrl+c", "quit", "退出"),
    ]

    def compose(self) -> ComposeResult:
        """Compose the chat screen layout."""
        yield Header()
        with Vertical(id="chat-container"):
            yield RichLog(id="chat-log", markup=True)
            yield Input(
                placeholder="输入消息按回车发送...",
                id="chat-input",
            )
        yield Footer()

    def on_mount(self) -> None:
        """Called when screen is mounted."""
        log = self.query_one("#chat-log", RichLog)
        log.write("[bold green]欢迎使用 OpenJax TUI![/bold green]")
        log.write("输入消息按回车发送，Ctrl+Q 退出。\n")

    def on_input_submitted(self, event: Input.Submitted) -> None:
        """Handle input submission."""
        if event.value.strip():
            log = self.query_one("#chat-log", RichLog)
            log.write(f"[bold blue]你:[/bold blue] {event.value}")
            # Clear input after submission
            self.query_one("#chat-input", Input).value = ""
