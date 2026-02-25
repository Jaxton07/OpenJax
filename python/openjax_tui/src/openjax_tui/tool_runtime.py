from __future__ import annotations

import re
import unicodedata
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
) -> int:
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
    elapsed_ms = 0
    if starts:
        elapsed_ms = max(int((monotonic_fn() - starts.pop()) * 1000), 0)
        stats.known_duration_ms += elapsed_ms
    if not starts:
        state.active_tool_starts.pop(key, None)
    return elapsed_ms


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
        return assistant_prefix
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
    elapsed_ms: int = 0,
    target_hint: str | None = None,
) -> None:
    label = tool_result_label_fn(tool_name, output)
    target = target_hint or _tool_target_from_output(tool_name, output)
    error_text = _summarize_tool_output(output) if not ok else ""
    completed_line = _format_tool_completion_line(
        label=label,
        target=target,
        elapsed_ms=elapsed_ms,
        ok=ok,
        error_text=error_text,
    )
    finalize_stream_line_fn(state)
    emit_ui_line_fn(state, f"{status_bullet_fn(ok)} {completed_line}")
    _ = emit_ui_spacer_fn


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
    _ = state


def _format_tool_completion_line(
    *,
    label: str,
    target: str | None,
    elapsed_ms: int = 0,
    ok: bool,
    error_text: str = "",
) -> str:
    row = label if not target else f"{label} ({target})"
    row = f"{row} · {_format_duration(max(elapsed_ms, 0))}"
    if not ok and error_text:
        row = f"{row} · {error_text}"
    return row


def _tool_target_from_output(tool_name: str, output: str) -> str | None:
    name = tool_name.strip().lower()
    if name != "read_file":
        return None
    normalized = " ".join(output.split())
    if not normalized:
        return None
    patterns = (
        r"\b(?:READ|Read)\s+([^\s:]+)",
        r"\bpath(?:=|:)\s*([^\s,]+)",
        r"\bfile(?:=|:)\s*([^\s,]+)",
    )
    for pattern in patterns:
        match = re.search(pattern, normalized)
        if match:
            return match.group(1).strip("()[]{}\"'")
    return None


def _format_duration(elapsed_ms: int) -> str:
    ms = max(elapsed_ms, 0)
    if ms < 1000:
        return f"{ms}ms"
    total_seconds = ms / 1000.0
    if total_seconds < 60:
        return f"{total_seconds:.1f}s"
    minutes = int(total_seconds // 60)
    seconds = total_seconds - (minutes * 60)
    return f"{minutes}m{seconds:04.1f}s"


def _summarize_tool_output(output: str, *, max_len: int = 60) -> str:
    normalized = " ".join(output.split())
    if not normalized:
        return ""
    if _display_width(normalized) <= max_len:
        return normalized
    return _truncate_display_width(normalized, max_width=max_len, suffix="...")


def _truncate_display_width(text: str, *, max_width: int, suffix: str = "") -> str:
    if max_width <= 0:
        return ""

    text_width = _display_width(text)
    if text_width <= max_width:
        return text

    suffix_width = _display_width(suffix)
    if suffix_width >= max_width:
        return _slice_by_display_width(suffix, max_width=max_width)

    head_width = max_width - suffix_width
    head = _slice_by_display_width(text, max_width=head_width).rstrip()
    return f"{head}{suffix}"


def _slice_by_display_width(text: str, *, max_width: int) -> str:
    if max_width <= 0:
        return ""

    width = 0
    clusters = _graphemeish_clusters(text)
    collected: list[str] = []
    for cluster in clusters:
        cluster_width = _display_width(cluster)
        if width + cluster_width > max_width:
            break
        collected.append(cluster)
        width += cluster_width
    return "".join(collected)


def _graphemeish_clusters(text: str) -> list[str]:
    clusters: list[str] = []
    regional_indicator_count = 0
    for ch in text:
        if not clusters:
            clusters.append(ch)
            regional_indicator_count = 1 if _is_regional_indicator(ch) else 0
            continue

        last = clusters[-1]
        attach = False
        if _is_zero_width_char(ch):
            attach = True
        elif _ends_with_zwj(last):
            attach = True
        elif _is_regional_indicator(ch) and regional_indicator_count % 2 == 1:
            attach = True

        if attach:
            clusters[-1] = f"{last}{ch}"
        else:
            clusters.append(ch)

        if _is_regional_indicator(ch):
            regional_indicator_count += 1
        else:
            regional_indicator_count = 0
    return clusters


def _display_width(text: str) -> int:
    total = 0
    for ch in text:
        total += _char_display_width(ch)
    return total


def _char_display_width(ch: str) -> int:
    if _is_zero_width_char(ch):
        return 0
    if _is_wide_emoji(ch):
        return 2
    return 2 if unicodedata.east_asian_width(ch) in {"W", "F"} else 1


def _is_zero_width_char(ch: str) -> bool:
    codepoint = ord(ch)
    if ch == "\u200d":
        return True
    if unicodedata.combining(ch):
        return True
    if 0xFE00 <= codepoint <= 0xFE0F:
        return True
    if 0xE0100 <= codepoint <= 0xE01EF:
        return True
    category = unicodedata.category(ch)
    return category in {"Cf", "Cc", "Cs"}


def _ends_with_zwj(cluster: str) -> bool:
    return cluster.endswith("\u200d")


def _is_regional_indicator(ch: str) -> bool:
    codepoint = ord(ch)
    return 0x1F1E6 <= codepoint <= 0x1F1FF


def _is_wide_emoji(ch: str) -> bool:
    codepoint = ord(ch)
    return (
        0x1F300 <= codepoint <= 0x1FAFF
        or 0x1F000 <= codepoint <= 0x1F02F
        or 0x2600 <= codepoint <= 0x27BF
    )
