from __future__ import annotations

from typing import Any, Callable


def print_event(
    evt: Any,
    *,
    state: Any,
    print_tool_turn_summary: bool,
    render_assistant_delta_fn: Callable[[str, str], None],
    render_assistant_message_fn: Callable[[str, str], None],
    finalize_stream_line_if_turn_fn: Callable[[str], None],
    record_tool_started_fn: Callable[[str, str], None],
    record_tool_completed_fn: Callable[[str, str, bool], None],
    print_tool_call_result_line_fn: Callable[[Any, str, bool, str], None],
    use_inline_approval_panel_fn: Callable[[Any], bool],
    print_tool_summary_for_turn_fn: Callable[[Any, str], None],
) -> None:
    turn = evt.turn_id or "-"
    t = evt.event_type
    if t == "assistant_delta":
        render_assistant_delta_fn(turn, str(evt.payload.get("content_delta", "")))
        return
    if t == "assistant_message":
        content = str(evt.payload.get("content", ""))
        render_assistant_message_fn(turn, content)
        return
    if t == "tool_call_started":
        finalize_stream_line_if_turn_fn(turn)
        record_tool_started_fn(turn, str(evt.payload.get("tool_name", "")))
        return
    if t == "tool_call_completed":
        finalize_stream_line_if_turn_fn(turn)
        tool_name = str(evt.payload.get("tool_name", ""))
        ok = bool(evt.payload.get("ok"))
        output = str(evt.payload.get("output", ""))
        record_tool_completed_fn(turn, tool_name, ok)
        print_tool_call_result_line_fn(state, tool_name, ok, output)
        return
    if t == "approval_requested":
        finalize_stream_line_if_turn_fn(turn)
        if state is None or not use_inline_approval_panel_fn(state):
            print(
                "[approval] id={rid} target={target} reason={reason}".format(
                    rid=evt.payload.get("request_id"),
                    target=evt.payload.get("target"),
                    reason=evt.payload.get("reason"),
                )
            )
        return
    if t == "approval_resolved":
        finalize_stream_line_if_turn_fn(turn)
        if state is None or not use_inline_approval_panel_fn(state):
            print(
                f"[approval] id={evt.payload.get('request_id')} approved={evt.payload.get('approved')}"
            )
        return
    if t == "turn_completed":
        finalize_stream_line_if_turn_fn(turn)
        if print_tool_turn_summary:
            print_tool_summary_for_turn_fn(state, turn)
        if state is not None:
            state.tool_turn_stats.pop(turn, None)
        return
    if t == "turn_started":
        return
    print(f"[{t}]")
