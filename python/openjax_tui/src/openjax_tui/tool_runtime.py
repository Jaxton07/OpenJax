from __future__ import annotations

from typing import Any, Callable


def record_tool_started(
    state: Any,
    turn: str,
    tool_name: str,
    *,
    monotonic_fn: Callable[[], float],
) -> None:
    key = (turn, tool_name)
    starts = state.active_tool_starts.setdefault(key, [])
    starts.append(monotonic_fn())


def record_tool_completed(
    state: Any,
    turn: str,
    tool_name: str,
    ok: bool,
    *,
    monotonic_fn: Callable[[], float],
    tool_turn_stats_cls: type,
) -> None:
    stats = state.tool_turn_stats.setdefault(turn, tool_turn_stats_cls())
    stats.calls += 1
    if ok:
        stats.ok_count += 1
    else:
        stats.fail_count += 1
    if tool_name:
        stats.tools.add(tool_name)

    key = (turn, tool_name)
    starts = state.active_tool_starts.get(key, [])
    if starts:
        elapsed_ms = max(int((monotonic_fn() - starts.pop()) * 1000), 0)
        stats.known_duration_ms += elapsed_ms
    if not starts:
        state.active_tool_starts.pop(key, None)


def status_bullet(
    *,
    state: Any,
    ok: bool,
    assistant_prefix: str,
    ansi_green: str,
    ansi_red: str,
    ansi_reset: str,
    supports_ansi_color_fn: Callable[[], bool],
) -> str:
    if state is not None and state.input_backend == "prompt_toolkit":
        color = ansi_green if ok else ansi_red
        return f"{color}{assistant_prefix}{ansi_reset}"
    if not supports_ansi_color_fn():
        return "🟢" if ok else "🔴"
    color = ansi_green if ok else ansi_red
    return f"{color}{assistant_prefix}{ansi_reset}"


def print_tool_call_result_line(
    state: Any,
    tool_name: str,
    ok: bool,
    output: str,
    *,
    status_bullet_fn: Callable[[bool], str],
    tool_result_label_fn: Callable[[str, str], str],
    finalize_stream_line_fn: Callable[[Any], None],
    emit_ui_spacer_fn: Callable[[Any], None],
    emit_ui_line_fn: Callable[[Any, str], None],
) -> None:
    bullet = status_bullet_fn(ok)
    label = tool_result_label_fn(tool_name, output)
    if not ok:
        label = f"{label} (failed)"
    finalize_stream_line_fn(state)
    emit_ui_spacer_fn(state)
    emit_ui_line_fn(state, f"{bullet} {label}")
    emit_ui_spacer_fn(state)


def print_tool_summary_for_turn(
    state: Any,
    turn: str,
    *,
    status_bullet_fn: Callable[[bool], str],
    finalize_stream_line_fn: Callable[[Any], None],
    emit_ui_line_fn: Callable[[Any, str], None],
) -> None:
    stats = state.tool_turn_stats.get(turn)
    if stats is None or stats.calls == 0:
        return

    tools = ", ".join(sorted(stats.tools)) if stats.tools else "-"
    duration = f"{stats.known_duration_ms}ms" if stats.known_duration_ms else "n/a"
    ok = stats.fail_count == 0
    bullet = status_bullet_fn(ok)
    finalize_stream_line_fn(state)
    emit_ui_line_fn(
        state,
        f"{bullet} tools: calls={stats.calls} ok={stats.ok_count} "
        f"fail={stats.fail_count} duration={duration} names=[{tools}]"
    )


def emit_ui_spacer(state: Any) -> None:
    if state is not None and state.input_backend == "prompt_toolkit":
        return
    print()
