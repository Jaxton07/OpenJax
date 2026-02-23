from __future__ import annotations

from typing import Any, Callable


def emit_ui_line(state: Any, text: str, *, refresh_history_view_fn: Callable[[Any], None]) -> None:
    if state is not None and state.input_backend == "prompt_toolkit":
        state.history_blocks.append(text)
        refresh_history_view_fn(state)
        return
    print(text)


def print_prefixed_block(
    state: Any,
    prefix: str,
    content: str,
    *,
    align_multiline_fn: Callable[[str], str],
    emit_ui_line_fn: Callable[[Any, str], None],
) -> None:
    aligned = align_multiline_fn(content)
    emit_ui_line_fn(state, f"{prefix} {aligned}")


def finalize_stream_line(state: Any) -> None:
    if state is None:
        return
    if state.stream_turn_id is not None:
        if state.input_backend != "prompt_toolkit":
            print()
        state.stream_turn_id = None
        state.stream_block_index = None


def finalize_stream_line_if_turn(
    state: Any,
    turn: str,
    *,
    finalize_stream_line_fn: Callable[[Any], None],
) -> None:
    if state is None:
        return
    if state.stream_turn_id == turn:
        finalize_stream_line_fn(state)


def render_assistant_delta(
    state: Any,
    turn: str,
    delta: str,
    *,
    assistant_prefix: str,
    align_multiline_fn: Callable[[str], str],
    finalize_stream_line_fn: Callable[[Any], None],
    refresh_history_view_fn: Callable[[Any], None],
) -> None:
    if state is None or not delta:
        return
    if state.input_backend == "prompt_toolkit":
        if state.stream_turn_id != turn:
            finalize_stream_line_fn(state)
            state.stream_turn_id = turn
            state.stream_text_by_turn[turn] = ""
            state.stream_block_index = len(state.history_blocks)
            state.history_blocks.append(f"{assistant_prefix} ")
        state.stream_text_by_turn[turn] = state.stream_text_by_turn.get(turn, "") + delta
        stream_text = state.stream_text_by_turn.get(turn, "")
        block = f"{assistant_prefix} {align_multiline_fn(stream_text)}"
        idx = state.stream_block_index
        if idx is not None and 0 <= idx < len(state.history_blocks):
            state.history_blocks[idx] = block
        refresh_history_view_fn(state)
        return
    if state.stream_turn_id != turn:
        finalize_stream_line_fn(state)
        state.stream_turn_id = turn
        state.stream_text_by_turn[turn] = ""
        print(f"{assistant_prefix} ", end="", flush=True)
    state.stream_text_by_turn[turn] = state.stream_text_by_turn.get(turn, "") + delta
    print(align_multiline_fn(delta), end="", flush=True)


def render_assistant_message(
    state: Any,
    turn: str,
    content: str,
    *,
    assistant_prefix: str,
    print_prefixed_block_fn: Callable[[Any, str, str], None],
    finalize_stream_line_fn: Callable[[Any], None],
) -> None:
    if state is None:
        print_prefixed_block_fn(state, assistant_prefix, content)
        return

    if state.stream_turn_id == turn:
        streamed = state.stream_text_by_turn.get(turn, "")
        if streamed == content:
            finalize_stream_line_fn(state)
            return
        finalize_stream_line_fn(state)

    print_prefixed_block_fn(state, assistant_prefix, content)
