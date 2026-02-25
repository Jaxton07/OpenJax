"""State management for OpenJax TUI."""

from __future__ import annotations

from dataclasses import dataclass, field
from enum import Enum, auto
import re
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
    turn_id: str | None
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
    approval_order: list[str] = field(default_factory=list)
    approval_focus_id: str | None = None

    # UI state
    current_input: str = ""
    command_palette_open: bool = False

    # Streaming state
    active_turn_id: str | None = None
    stream_text_by_turn: dict[str, str] = field(default_factory=dict)
    turn_render_kind_by_turn: dict[str, str] = field(default_factory=dict)

    # Error state
    last_error: str | None = None

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

        metadata_with_default = dict(metadata)
        metadata_with_default.setdefault("render_kind", "plain")
        msg = Message(
            role=role,
            content=content,
            timestamp=datetime.now().isoformat(),
            metadata=metadata_with_default,
        )
        self.messages.append(msg)
        return msg

    def add_tool_call_result(
        self,
        tool_name: str,
        ok: bool,
        output: str,
        *,
        output_preview: str,
        target: str | None = None,
        elapsed_ms: int = 0,
    ) -> Message:
        """Add a summarized tool-call result line."""
        label = _tool_result_label(tool_name, output)
        return self.add_message(
            "tool",
            label,
            render_kind="plain",
            ok=ok,
            tool_name=tool_name,
            output_preview=output_preview,
            target=target,
            elapsed_ms=elapsed_ms,
        )

    def clear_messages(self) -> None:
        """Clear all messages."""
        self.messages.clear()

    def add_approval(
        self,
        approval_id: str,
        action: str,
        reason: str | None = None,
        turn_id: str | None = None,
    ) -> ApprovalRequest:
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
            turn_id=turn_id,
            action=action,
            reason=reason,
            timestamp=datetime.now().isoformat(),
        )
        self.pending_approvals[approval_id] = approval
        if approval_id not in self.approval_order:
            self.approval_order.append(approval_id)
        self.approval_focus_id = approval_id
        return approval

    def resolve_approval(self, approval_id: str) -> ApprovalRequest | None:
        """Resolve (remove) a pending approval.

        Args:
            approval_id: Approval identifier to resolve

        Returns:
            The resolved ApprovalRequest or None if not found
        """
        resolved = self.pending_approvals.pop(approval_id, None)
        if approval_id in self.approval_order:
            self.approval_order.remove(approval_id)
        if self.approval_focus_id == approval_id:
            self.approval_focus_id = self.approval_order[-1] if self.approval_order else None
        return resolved

    def get_pending_approval_count(self) -> int:
        """Get count of pending approvals."""
        return len(self.pending_approvals)

    def set_turn_phase(self, phase: TurnPhase) -> None:
        """Set the current turn phase."""
        self.turn_phase = phase

    def is_processing(self) -> bool:
        """Check if currently processing a turn."""
        return self.turn_phase in (TurnPhase.THINKING, TurnPhase.STREAMING)

    def start_turn(self, turn_id: str) -> None:
        """Mark a turn as active and initialize stream buffer."""
        self.active_turn_id = turn_id
        self.stream_text_by_turn.setdefault(turn_id, "")
        self.turn_render_kind_by_turn.setdefault(turn_id, "plain")
        self.set_turn_phase(TurnPhase.THINKING)

    def append_delta(self, turn_id: str, delta: str) -> str:
        """Append assistant delta for a turn and return aggregated text."""
        current = self.stream_text_by_turn.get(turn_id, "")
        updated = current + delta
        self.stream_text_by_turn[turn_id] = updated
        self.active_turn_id = turn_id
        self.set_turn_phase(TurnPhase.STREAMING)
        return updated

    def finalize_turn(self, turn_id: str, final_content: str | None = None) -> str:
        """Finalize turn stream content and return final text."""
        if final_content is None:
            final_text = self.stream_text_by_turn.get(turn_id, "")
        else:
            final_text = final_content
            self.stream_text_by_turn[turn_id] = final_text

        if self.active_turn_id == turn_id:
            self.active_turn_id = None
        self.stream_text_by_turn.pop(turn_id, None)
        self.set_turn_phase(TurnPhase.IDLE)
        return final_text

    def set_error(self, message: str) -> None:
        """Set last error and move phase into error."""
        self.last_error = message
        self.set_turn_phase(TurnPhase.ERROR)

    def latest_pending_approval(self) -> ApprovalRequest | None:
        """Get the latest pending approval request."""
        if not self.approval_order:
            return None
        approval_id = self.approval_order[-1]
        return self.pending_approvals.get(approval_id)


def _tool_result_label(tool_name: str, output: str) -> str:
    name = tool_name.strip().lower()
    if name == "read_file":
        return "Read 1 file"
    if name in {"apply_patch", "edit_file_range", "write_file"}:
        _ = output
        return "Update 1 file"
    if name == "list_dir":
        return "Read directory"
    if name == "grep_files":
        return "Search files"
    if name == "shell":
        return "Run shell command"
    if not name:
        return "Tool call"
    return name.replace("_", " ").title()


def _extract_updated_target(output: str) -> str | None:
    match = re.search(r"\bUPDATE\s+([^\s:]+)", output)
    if not match:
        return None
    return match.group(1).strip()


def summarize_tool_output(output: str, *, max_len: int = 120) -> str:
    """Summarize tool output into a single line for card previews."""
    normalized = " ".join(output.split())
    if not normalized:
        return ""
    if len(normalized) <= max_len:
        return normalized
    return f"{normalized[: max_len - 3]}..."


def extract_tool_target(tool_name: str, output: str) -> str | None:
    """Best-effort target extraction for tool card metadata."""
    name = tool_name.strip().lower()
    normalized = " ".join(output.split())
    if not normalized:
        return None
    if name == "read_file":
        patterns = (
            r"\b(?:READ|Read)\s+([^\s:]+)",
            r"\bpath(?:=|:)\s*([^\s,]+)",
            r"\bfile(?:=|:)\s*([^\s,]+)",
        )
        for pattern in patterns:
            match = re.search(pattern, normalized)
            if match:
                return match.group(1).strip("()[]{}\"'")
    if name in {"apply_patch", "edit_file_range", "write_file"}:
        return _extract_updated_target(output)
    return None
