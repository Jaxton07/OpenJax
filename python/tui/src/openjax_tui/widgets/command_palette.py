"""Command palette widget for OpenJax TUI."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Callable

from textual.containers import Horizontal, Vertical
from textual.message import Message
from textual.reactive import reactive
from textual.widgets import Input, Label, ListItem, ListView, Static


@dataclass
class Command:
    """Command definition."""

    name: str
    description: str
    handler: Callable[[], None]
    shortcut: str | None = None


class CommandPalette(Static):
    """Command palette widget for fuzzy command search.

    Usage:
        - Type "/" to open
        - Type to filter commands
        - Enter to execute first match
        - Escape to close
    """

    # Reactive state
    query: reactive[str] = reactive("")
    commands: reactive[list[Command]] = reactive(lambda: [])
    filtered_commands: reactive[list[Command]] = reactive(lambda: [])

    def __init__(self, commands: list[Command] | None = None, **kwargs) -> None:
        """Initialize command palette.

        Args:
            commands: List of available commands
            **kwargs: Additional arguments passed to Static
        """
        super().__init__(**kwargs)
        self.commands = commands or []
        self.filtered_commands = self.commands.copy()

    def compose(self):
        """Compose the command palette layout."""
        with Vertical(id="command-palette-container"):
            yield Input(
                placeholder="输入命令...",
                id="command-input",
            )
            yield ListView(id="command-list")

    def on_mount(self) -> None:
        """Called when widget is mounted."""
        self.update_command_list()
        # Focus the input
        self.query_one("#command-input", Input).focus()

    def watch_query(self, query: str) -> None:
        """Watch for query changes and filter commands."""
        self.filter_commands(query)

    def filter_commands(self, query: str) -> None:
        """Filter commands based on query.

        Args:
            query: Search query string
        """
        query = query.lower().strip()
        if not query:
            self.filtered_commands = self.commands.copy()
        else:
            # Fuzzy match: command name or description contains query
            self.filtered_commands = [
                cmd
                for cmd in self.commands
                if query in cmd.name.lower() or query in cmd.description.lower()
            ]
        self.update_command_list()

    def update_command_list(self) -> None:
        """Update the command list display."""
        # Check if widget is mounted before trying to update UI
        if not self.is_mounted:
            return

        try:
            list_view = self.query_one("#command-list", ListView)
        except Exception:
            # Widget not fully mounted yet
            return

        list_view.clear()

        for i, cmd in enumerate(self.filtered_commands):
            # Create list item with command info
            item = ListItem(
                Horizontal(
                    Label(f"/{cmd.name}", classes="command-name"),
                    Label(cmd.description, classes="command-description"),
                    classes="command-item",
                ),
                id=f"cmd-{i}",
            )
            list_view.append(item)

        # Highlight first item if available
        if self.filtered_commands:
            list_view.index = 0

    def on_input_changed(self, event: Input.Changed) -> None:
        """Handle input changes."""
        self.query = event.value

    def on_input_submitted(self, event: Input.Submitted) -> None:
        """Handle input submission (Enter key).

        Executes the first filtered command.
        """
        if self.filtered_commands:
            # Execute the first command
            self.execute_command(0)

    def on_list_view_selected(self, event: ListView.Selected) -> None:
        """Handle list item selection."""
        # Get the index from the item id
        item_id = event.item.id
        if item_id and item_id.startswith("cmd-"):
            index = int(item_id.split("-")[1])
            self.execute_command(index)

    def execute_command(self, index: int) -> None:
        """Execute command at given index.

        Args:
            index: Index in filtered_commands list
        """
        if 0 <= index < len(self.filtered_commands):
            cmd = self.filtered_commands[index]
            # Close palette first
            self.dismiss()
            # Execute command handler
            cmd.handler()

    def dismiss(self) -> None:
        """Dismiss the command palette."""
        # Notify parent to remove this widget
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
        """Move selection in command list.

        Args:
            direction: 1 for down, -1 for up
        """
        list_view = self.query_one("#command-list", ListView)
        if not self.filtered_commands:
            return

        current_index = list_view.index or 0
        new_index = current_index + direction
        new_index = max(0, min(new_index, len(self.filtered_commands) - 1))
        list_view.index = new_index

    class Dismissed(Message):
        """Message sent when palette is dismissed."""

        pass
