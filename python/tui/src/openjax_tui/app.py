"""Main application for OpenJax TUI."""

from __future__ import annotations

import sys

from textual.app import App
from textual.reactive import reactive

from .screens.chat import ChatScreen
from .state import AppState, TurnPhase


class OpenJaxApp(App):
    """OpenJax TUI main application."""

    TITLE = "OpenJax"
    SUB_TITLE = "AI Agent Framework"
    CSS_PATH = "styles.tcss"

    # Reactive state (synced with AppState)
    session_id: reactive[str | None] = reactive(None)
    turn_phase: reactive[TurnPhase] = reactive(TurnPhase.IDLE)
    current_input: reactive[str] = reactive("")

    def __init__(self, **kwargs) -> None:
        """Initialize the application."""
        super().__init__(**kwargs)
        self.state = AppState()

    def on_mount(self) -> None:
        """Push the chat screen when app mounts."""
        self.push_screen(ChatScreen())

    def action_help_quit(self) -> None:
        """Override to quit immediately without confirmation."""
        self.exit()

    def action_help(self) -> None:
        """Show help information."""
        help_text = """
[bold cyan]OpenJax TUI 帮助[/bold cyan]

[bold]快捷键:[/bold]
  Ctrl+C (macOS) / Ctrl+Q (Linux/Windows) - 退出程序
  / - 打开命令面板
  Escape - 关闭命令面板

[bold]命令:[/bold]
  /help    - 显示此帮助信息
  /clear   - 清空对话历史
  /exit    - 退出程序
  /pending - 查看待处理审批

[bold]使用说明:[/bold]
  直接输入消息按回车发送
  输入 / 触发命令面板，支持模糊搜索
        """
        # Get the current screen and add help message
        if isinstance(self.screen, ChatScreen):
            self.screen.add_system_message(help_text)

    def action_clear(self) -> None:
        """Clear conversation history."""
        self.state.clear_messages()
        if isinstance(self.screen, ChatScreen):
            self.screen.clear_messages()
            self.screen.add_system_message("[dim]对话历史已清空[/dim]")

    def action_exit(self) -> None:
        """Exit the application."""
        self.exit()

    def action_pending(self) -> None:
        """Show pending approvals."""
        count = self.state.get_pending_approval_count()
        if count == 0:
            msg = "[dim]没有待处理的审批请求[/dim]"
        else:
            msg = f"[yellow]有 {count} 个待处理的审批请求[/yellow]"

        if isinstance(self.screen, ChatScreen):
            self.screen.add_system_message(msg)

    def action_command_palette(self) -> None:
        """Open the command palette."""
        if isinstance(self.screen, ChatScreen):
            self.screen.show_command_palette()

    def submit_message(self, text: str) -> None:
        """Submit a user message.

        Args:
            text: The message text to submit
        """
        # Add to state
        self.state.add_message("user", text)

        # Show in UI
        if isinstance(self.screen, ChatScreen):
            self.screen.add_user_message(text)

        # TODO: In Phase 3, integrate with SDK to send to backend
        # For now, just echo a mock response
        self._mock_response(text)

    def _mock_response(self, user_text: str) -> None:
        """Generate a mock response (for testing without SDK).

        Args:
            user_text: The user's input text
        """
        # Simulate processing
        self.state.set_turn_phase(TurnPhase.THINKING)
        self.turn_phase = TurnPhase.THINKING

        # Mock response
        response = f"收到消息: {user_text}\n\n(这是模拟响应，Phase 3 将集成真实 SDK)"

        # Add to state
        self.state.add_message("assistant", response)

        # Show in UI
        if isinstance(self.screen, ChatScreen):
            self.screen.add_assistant_message(response)

        # Reset phase
        self.state.set_turn_phase(TurnPhase.IDLE)
        self.turn_phase = TurnPhase.IDLE
