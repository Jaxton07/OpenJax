from __future__ import annotations

import re
from typing import Any, Callable


def emit_ui_line(state: Any, text: str, *, refresh_history_view_fn: Callable[[Any], None]) -> None:
    if state is not None and state.input_backend == "prompt_toolkit":
        state.history_blocks.append(text)
        refresh_history_view_fn(state)
        return
    if state is not None and getattr(state, "last_basic_ui_block_emitted", False):
        print()
    print(text)
    if state is not None:
        state.last_basic_ui_block_emitted = True


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
        if turn not in state.stream_text_by_turn:
            state.stream_text_by_turn[turn] = ""
        state.stream_text_by_turn[turn] = state.stream_text_by_turn.get(turn, "") + delta
        stream_text = state.stream_text_by_turn.get(turn, "")
        block = f"{assistant_prefix} {align_multiline_fn(stream_text)}"
        idx = _upsert_turn_block(state, turn, block)
        state.stream_block_index = idx
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

    state.assistant_message_by_turn[turn] = content

    if state.input_backend == "prompt_toolkit":
        # Final assistant message is authoritative for the turn. Always upsert
        # the turn block so UI does not depend on transient streaming state.
        aligned = content.replace("\n", "\n  ")
        block = f"{assistant_prefix} {aligned}"
        _upsert_turn_block(state, turn, block)
        state.stream_text_by_turn[turn] = content
        finalize_stream_line_fn(state)
        setter = getattr(state, "history_setter", None)
        if callable(setter):
            setter()
        return

    if state.stream_turn_id == turn:
        streamed = state.stream_text_by_turn.get(turn, "")
        if streamed == content:
            finalize_stream_line_fn(state)
            return
        finalize_stream_line_fn(state)

    print_prefixed_block_fn(state, assistant_prefix, content)


def align_multiline(text: str, continuation_prefix: str) -> str:
    if not text:
        return ""
    return text.replace("\n", f"\n{continuation_prefix}")


def extract_updated_target(output: str) -> str | None:
    match = re.search(r"\bUPDATE\s+([^\s:]+)", output)
    if not match:
        return None
    return match.group(1).strip()


def tool_result_label(tool_name: str, output: str) -> str:
    name = tool_name.strip().lower()
    if name == "read_file":
        return "Read 1 file"
    if name in {"apply_patch", "edit_file_range", "write_file"}:
        target = extract_updated_target(output)
        if target:
            return f"Update({target})"
        return "Update file"
    if name == "list_dir":
        return "Read directory"
    if name == "grep_files":
        return "Search files"
    if name == "shell":
        return "Run shell command"
    if not name:
        return "Tool call"
    return name.replace("_", " ").title()


def _upsert_turn_block(state: Any, turn: str, block: str) -> int:
    idx = state.turn_block_index.get(turn)
    if idx is not None and 0 <= idx < len(state.history_blocks):
        state.history_blocks[idx] = block
        return idx

    state.history_blocks.append(block)
    idx = len(state.history_blocks) - 1
    state.turn_block_index[turn] = idx
    return idx
