"""Commands module for OpenJax TUI."""

from __future__ import annotations

from typing import TYPE_CHECKING

from openjax_tui.widgets.command_palette import Command

if TYPE_CHECKING:
    from openjax_tui.app import OpenJaxApp


def create_commands(app: "OpenJaxApp") -> list[Command]:
    """Create the list of available commands.

    Args:
        app: The main application instance

    Returns:
        List of Command objects
    """
    return [
        Command(
            name="help",
            description="显示帮助信息",
            handler=lambda: app.action_help(),
        ),
        Command(
            name="clear",
            description="清空对话历史",
            handler=lambda: app.action_clear(),
        ),
        Command(
            name="exit",
            description="退出程序",
            handler=lambda: app.action_exit(),
        ),
        Command(
            name="pending",
            description="查看待处理审批",
            handler=lambda: app.action_pending(),
        ),
        Command(
            name="approve",
            description="批准当前审批请求",
            handler=lambda: app.action_approve(),
        ),
        Command(
            name="deny",
            description="拒绝当前审批请求",
            handler=lambda: app.action_deny(),
        ),
    ]
