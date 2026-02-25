"""Thinking status widget shown above the chat input."""

from __future__ import annotations

from textual.widgets import Static


class ThinkingStatus(Static):
    """Inline thinking indicator with a five-dot breathing wave."""

    DOT_COUNT = 5
    TICK_SECONDS = 0.06
    PHASE_STEP = 0.28
    HOLD_TICKS_AT_END = 4

    def __init__(self, **kwargs) -> None:
        super().__init__(**kwargs)
        self._phase_position = 0.0
        self._hold_ticks_remaining = 0
        self._timer = None

    def on_mount(self) -> None:
        """Start animation timer when widget is mounted."""
        self.update(self.render_text())
        self._timer = self.set_interval(self.TICK_SECONDS, self._tick)

    def on_unmount(self) -> None:
        """Stop animation timer when widget is removed."""
        if self._timer is not None:
            self._timer.stop()
            self._timer = None

    def render_text(self) -> str:
        """Render one animation frame."""
        lead = self._phase_position % self.DOT_COUNT
        dots: list[str] = []

        for index in range(self.DOT_COUNT):
            # Use directional distance to create a single-direction flowing tail.
            tail_distance = (lead - index) % self.DOT_COUNT
            if tail_distance < 0.34:
                dots.append("[bold #37c9ff]•[/bold #37c9ff]")
            elif tail_distance < 0.80:
                dots.append("[#37c9ff]•[/#37c9ff]")
            elif tail_distance < 1.35:
                dots.append("[#2ca4cc]•[/#2ca4cc]")
            elif tail_distance < 1.90:
                dots.append("[#217d99]•[/#217d99]")
            else:
                dots.append("[dim]•[/dim]")
        return f"[bold #37c9ff]Thinking[/bold #37c9ff] {''.join(dots)}"

    def _tick(self) -> None:
        """Advance animation frame."""
        if self._hold_ticks_remaining > 0:
            self._hold_ticks_remaining -= 1
            if self._hold_ticks_remaining == 0:
                self._phase_position = 0.0
        elif self._phase_position >= self.DOT_COUNT - 1:
            self._phase_position = float(self.DOT_COUNT - 1)
            self._hold_ticks_remaining = self.HOLD_TICKS_AT_END
        else:
            self._phase_position = min(
                self._phase_position + self.PHASE_STEP,
                float(self.DOT_COUNT - 1),
            )
        self.update(self.render_text())
