"""Approval popup widget shown above the chat input."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Callable

from textual.message import Message
from textual.widgets import Static


@dataclass
class ApprovalOption:
    """Approval option row displayed in popup."""

    name: str
    description: str
    handler: Callable[[], None]


class ApprovalPopup(Static):
    """Inline approval popup with approve/deny/cancel options."""

    MAX_SUMMARY_LEN = 140

    def __init__(self, **kwargs) -> None:
        super().__init__(**kwargs)
        self.summary = ""
        self.options = [
            ApprovalOption(name="approve", description="批准请求", handler=lambda: None),
            ApprovalOption(name="deny", description="拒绝请求", handler=lambda: None),
            ApprovalOption(name="cancel", description="稍后处理", handler=lambda: None),
        ]
        self.selected_index = 0

    def set_summary(self, summary: str) -> None:
        """Update summary line and refresh popup content."""
        self.summary = self._truncate(summary.strip())
        self.refresh_popup()

    def refresh_popup(self) -> None:
        """Render summary and option rows."""
        lines = [f"[bold yellow]{self.summary}[/bold yellow]"]
        for index, option in enumerate(self.options):
            prefix = "› " if index == self.selected_index else "  "
            lines.append(f"{prefix}[bold]{option.name}[/bold] [dim]{option.description}[/dim]")
        self.update("\n".join(lines))

    def move_selection(self, direction: int) -> None:
        """Move highlighted option without wraparound."""
        next_index = max(0, min(self.selected_index + direction, len(self.options) - 1))
        if next_index == self.selected_index:
            return
        self.selected_index = next_index
        self.refresh_popup()

    def confirm_selection(self) -> None:
        """Emit selected option event."""
        option = self.options[self.selected_index]
        self.post_message(self.SelectionConfirmed(option_name=option.name))

    def dismiss(self) -> None:
        """Dismiss popup."""
        self.post_message(self.Dismissed())

    def on_mount(self) -> None:
        """Render initial content after mount."""
        self.refresh_popup()

    def on_key(self, event) -> None:
        """Handle popup-local key bindings."""
        if event.key == "up":
            event.stop()
            self.move_selection(-1)
        elif event.key == "down":
            event.stop()
            self.move_selection(1)
        elif event.key == "enter":
            event.stop()
            self.confirm_selection()
        elif event.key == "escape":
            event.stop()
            self.dismiss()

    @classmethod
    def format_summary(
        cls,
        *,
        approval_id: str,
        action: str,
        turn_id: str | None,
        reason: str | None,
    ) -> str:
        """Build one-line approval summary."""
        normalized_reason = reason.strip() if reason else "-"
        summary = (
            f"[{approval_id}] action={action or '-'} turn={turn_id or '-'} reason={normalized_reason}"
        )
        return cls._truncate(summary)

    @classmethod
    def _truncate(cls, value: str) -> str:
        if len(value) <= cls.MAX_SUMMARY_LEN:
            return value
        return f"{value[: cls.MAX_SUMMARY_LEN - 3]}..."

    class Dismissed(Message):
        """Message emitted when popup is dismissed."""

    @dataclass
    class SelectionConfirmed(Message):
        """Message emitted when a selection is confirmed."""

        option_name: str
