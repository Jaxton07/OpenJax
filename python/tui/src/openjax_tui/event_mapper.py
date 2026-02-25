"""Map SDK events into state transitions and UI operations."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any

from .state import AppState, TurnPhase, extract_tool_target, summarize_tool_output


@dataclass
class UiOperation:
    """UI operation produced by event mapping."""

    kind: str
    turn_id: str | None = None
    text: str | None = None
    request_id: str | None = None


def map_event(evt: Any, state: AppState) -> list[UiOperation]:
    """Map a daemon event into state updates and UI operations."""
    event_type = evt.event_type
    turn_id = evt.turn_id
    ops: list[UiOperation] = []

    if event_type == "turn_started" and turn_id:
        state.start_turn(turn_id)
        ops.append(UiOperation(kind="phase_changed"))
        return ops

    if event_type == "assistant_delta" and turn_id:
        delta = str(evt.payload.get("content_delta", ""))
        if delta:
            aggregated = state.append_delta(turn_id, delta)
            ops.append(UiOperation(kind="stream_updated", turn_id=turn_id, text=aggregated))
            ops.append(UiOperation(kind="phase_changed"))
        return ops

    if event_type == "assistant_message" and turn_id:
        content = str(evt.payload.get("content", ""))
        state.stream_text_by_turn[turn_id] = content
        state.active_turn_id = turn_id
        state.turn_render_kind_by_turn[turn_id] = "markdown"
        state.set_turn_phase(TurnPhase.STREAMING)
        ops.append(UiOperation(kind="stream_updated", turn_id=turn_id, text=content))
        ops.append(UiOperation(kind="phase_changed"))
        return ops

    if event_type == "approval_requested" and turn_id:
        request_id = str(evt.payload.get("request_id", ""))
        if request_id:
            action = str(evt.payload.get("target", evt.payload.get("action", "")))
            reason = evt.payload.get("reason")
            state.add_approval(
                approval_id=request_id,
                turn_id=turn_id,
                action=action or "unknown",
                reason=str(reason) if reason is not None else None,
            )
            ops.append(UiOperation(kind="approval_added", request_id=request_id))
        return ops

    if event_type == "approval_resolved":
        request_id = str(evt.payload.get("request_id", ""))
        if request_id:
            state.resolve_approval(request_id)
            ops.append(UiOperation(kind="approval_removed", request_id=request_id))
        return ops

    if event_type == "tool_call_started" and turn_id:
        tool_name = str(evt.payload.get("tool_name", "")).strip()
        target = evt.payload.get("target")
        if tool_name and isinstance(target, str) and target.strip():
            state.add_tool_target_hint(turn_id, tool_name, target.strip())
        return ops

    if event_type == "tool_call_completed":
        tool_name = str(evt.payload.get("tool_name", ""))
        ok = bool(evt.payload.get("ok", False))
        output = str(evt.payload.get("output", ""))
        turn_target = state.pop_tool_target_hint(turn_id or "", tool_name) if turn_id else None
        state.add_tool_call_result(
            tool_name=tool_name,
            ok=ok,
            output=output,
            output_preview=summarize_tool_output(output),
            target=turn_target or extract_tool_target(tool_name, output),
            elapsed_ms=0,
        )
        ops.append(UiOperation(kind="tool_call_completed"))
        return ops

    if event_type == "turn_completed" and turn_id:
        final_text = state.finalize_turn(turn_id)
        ops.append(UiOperation(kind="turn_completed", turn_id=turn_id, text=final_text))
        ops.append(UiOperation(kind="phase_changed"))
        return ops

    return ops
