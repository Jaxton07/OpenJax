"""State management for OpenJax TUI."""

from __future__ import annotations

from dataclasses import dataclass, field
from enum import Enum, auto
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from datetime import datetime


class TurnPhase(Enum):
    """Turn processing phases."""

    IDLE = auto()
    THINKING = auto()
    STREAMING = auto()
    ERROR = auto()


@dataclass
class Message:
    """Chat message."""

    role: str  # "user" or "assistant"
    content: str
    timestamp: str | None = None
    metadata: dict = field(default_factory=dict)


@dataclass
class ApprovalRequest:
    """Pending approval request."""

    id: str
    action: str
    reason: str | None = None
    timestamp: str | None = None


@dataclass
class AppState:
    """Application state manager.

    This class holds all mutable state for the TUI application.
    It is designed to work with Textual's reactive system.
    """

    # Session state
    session_id: str | None = None
    turn_phase: TurnPhase = field(default=TurnPhase.IDLE)

    # Messages
    messages: list[Message] = field(default_factory=list)

    # Approvals
    pending_approvals: dict[str, ApprovalRequest] = field(default_factory=dict)

    # UI state
    current_input: str = ""
    command_palette_open: bool = False

    def add_message(self, role: str, content: str, **metadata) -> Message:
        """Add a new message to the history.

        Args:
            role: Message role ("user" or "assistant")
            content: Message content
            **metadata: Additional metadata

        Returns:
            The created Message object
        """
        from datetime import datetime

        msg = Message(
            role=role,
            content=content,
            timestamp=datetime.now().isoformat(),
            metadata=metadata,
        )
        self.messages.append(msg)
        return msg

    def clear_messages(self) -> None:
        """Clear all messages."""
        self.messages.clear()

    def add_approval(self, approval_id: str, action: str, reason: str | None = None) -> ApprovalRequest:
        """Add a pending approval request.

        Args:
            approval_id: Unique approval identifier
            action: Action to approve
            reason: Optional reason for the action

        Returns:
            The created ApprovalRequest object
        """
        from datetime import datetime

        approval = ApprovalRequest(
            id=approval_id,
            action=action,
            reason=reason,
            timestamp=datetime.now().isoformat(),
        )
        self.pending_approvals[approval_id] = approval
        return approval

    def resolve_approval(self, approval_id: str) -> ApprovalRequest | None:
        """Resolve (remove) a pending approval.

        Args:
            approval_id: Approval identifier to resolve

        Returns:
            The resolved ApprovalRequest or None if not found
        """
        return self.pending_approvals.pop(approval_id, None)

    def get_pending_approval_count(self) -> int:
        """Get count of pending approvals."""
        return len(self.pending_approvals)

    def set_turn_phase(self, phase: TurnPhase) -> None:
        """Set the current turn phase."""
        self.turn_phase = phase

    def is_processing(self) -> bool:
        """Check if currently processing a turn."""
        return self.turn_phase in (TurnPhase.THINKING, TurnPhase.STREAMING)
