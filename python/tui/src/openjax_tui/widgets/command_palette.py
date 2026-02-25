"""Command palette widget for OpenJax TUI."""

from __future__ import annotations

from dataclasses import dataclass
import logging
from typing import Callable

from textual.message import Message
from textual.widgets import Static

logger = logging.getLogger("openjax_tui")


@dataclass
class Command:
    """Command definition."""

    name: str
    description: str
    handler: Callable[[], None]
    shortcut: str | None = None


class CommandPalette(Static):
    """Inline command list shown above chat input."""

    def __init__(self, commands: list[Command] | None = None, **kwargs) -> None:
        """Initialize command palette.

        Args:
            commands: List of available commands
            **kwargs: Additional arguments passed to Static
        """
        super().__init__(**kwargs)
        self.query = ""
        self.commands = list(commands or [])
        self.filtered_commands = self.commands.copy()
        self.selected_index = 0

    def on_mount(self) -> None:
        """Called when widget is mounted."""
        self.refresh_commands("")

    def filter_commands(self, query: str) -> None:
        """Filter and rank commands based on query.

        Args:
            query: Search query string
        """
        normalized_query = query.lower().strip().lstrip("/")
        if not normalized_query:
            self.filtered_commands = self.commands.copy()
        else:
            scored: list[tuple[int, Command]] = []
            for cmd in self.commands:
                score = self._score_command(normalized_query, cmd)
                if score is not None:
                    scored.append((score, cmd))
            scored.sort(key=lambda pair: (pair[0], pair[1].name))
            self.filtered_commands = [cmd for _, cmd in scored]

    def refresh_commands(self, query: str) -> None:
        """Refresh visible commands for a query."""
        self.query = query
        self.filter_commands(query)
        self.selected_index = 0
        self.update_command_list()
        logger.info("command_palette refresh query=%s matches=%s", query, len(self.filtered_commands))

    @staticmethod
    def _score_command(query: str, cmd: Command) -> int | None:
        """Return a score for a command; lower score means better match."""
        name = cmd.name.lower()
        description = cmd.description.lower()
        if name == query:
            return 0
        if name.startswith(query):
            return 10 + (len(name) - len(query))
        if query in name:
            return 100 + name.index(query)
        if query in description:
            return 300 + description.index(query)

        position = -1
        gap_penalty = 0
        for char in query:
            next_pos = name.find(char, position + 1)
            if next_pos < 0:
                return None
            if position >= 0:
                gap_penalty += next_pos - position - 1
            position = next_pos
        return 500 + gap_penalty

    def update_command_list(self) -> None:
        """Render current command candidates."""
        if not self.filtered_commands:
            self.update("[dim]无匹配命令[/dim]")
            return

        lines: list[str] = []
        for index, cmd in enumerate(self.filtered_commands):
            prefix = "› " if index == self.selected_index else "  "
            line = f"{prefix}[bold]/{cmd.name}[/bold]  [dim]{cmd.description}[/dim]"
            lines.append(line)
        self.update("\n".join(lines))

    def execute_command(self, index: int) -> None:
        """Execute command at given index.

        Args:
            index: Index in filtered_commands list
        """
        if 0 <= index < len(self.filtered_commands):
            cmd = self.filtered_commands[index]
            logger.info("command_palette execute name=%s index=%s", cmd.name, index)
            self.dismiss()
            cmd.handler()

    def execute_best_match(self) -> bool:
        """Execute the top ranked command.

        Returns:
            True if a command was executed, False otherwise.
        """
        if not self.filtered_commands:
            return False
        self.execute_command(self.selected_index)
        return True

    def dismiss(self) -> None:
        """Dismiss the command palette."""
        self.post_message(self.Dismissed())

    def on_key(self, event) -> None:
        """Handle key events."""
        if event.key == "escape":
            event.stop()
            self.dismiss()
        elif event.key == "down":
            event.stop()
            self.move_selection(1)
        elif event.key == "up":
            event.stop()
            self.move_selection(-1)

    def move_selection(self, direction: int) -> None:
        """Move selection in command candidates.

        Args:
            direction: 1 for down, -1 for up
        """
        if not self.filtered_commands:
            return

        new_index = self.selected_index + direction
        new_index = max(0, min(new_index, len(self.filtered_commands) - 1))
        self.selected_index = new_index
        self.update_command_list()

    class Dismissed(Message):
        """Message sent when palette is dismissed."""

        pass
