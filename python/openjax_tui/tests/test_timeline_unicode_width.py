import unittest
import unicodedata

from openjax_tui import tool_runtime


def _naive_len_truncate(text: str, max_len: int) -> str:
    if len(text) <= max_len:
        return text
    return text[: max_len - 3] + "..."


def _noop_finalize(_state: object) -> None:
    return None


def _noop_spacer(_state: object) -> None:
    return None


def _render_completed_line(output: str) -> str:
    lines: list[str] = []

    def _collect_line(_state: object, text: str) -> None:
        lines.append(text)

    tool_runtime.print_tool_call_result_line(
        state=None,
        tool_name="shell",
        ok=True,
        output=output,
        status_bullet_fn=lambda ok: "OK",
        tool_result_label_fn=lambda _name, _out: "Tool",
        finalize_stream_line_fn=_noop_finalize,
        emit_ui_spacer_fn=_noop_spacer,
        emit_ui_line_fn=_collect_line,
        elapsed_ms=7,
    )
    return lines[-1]


def _extract_snippet(line: str) -> str:
    marker = "[completed] 7ms "
    _, _, snippet = line.partition(marker)
    return snippet


def _display_width(text: str) -> int:
    total = 0
    for ch in text:
        if ch == "\u200d" or unicodedata.combining(ch):
            continue
        codepoint = ord(ch)
        if 0xFE00 <= codepoint <= 0xFE0F:
            continue
        if unicodedata.category(ch) in {"Cf", "Cc", "Cs"}:
            continue
        if unicodedata.east_asian_width(ch) in {"W", "F"}:
            total += 2
            continue
        if (
            0x1F300 <= codepoint <= 0x1FAFF
            or 0x1F000 <= codepoint <= 0x1F02F
            or 0x2600 <= codepoint <= 0x27BF
        ):
            total += 2
            continue
        total += 1
    return total


class TimelineUnicodeWidthTest(unittest.TestCase):
    def test_combining_marks_do_not_trigger_false_truncation(self) -> None:
        output = ("Cafe\u0301 " * 11).strip()
        completed_line = _render_completed_line(output)
        snippet = _extract_snippet(completed_line)

        normalized = " ".join(output.split())
        naive = _naive_len_truncate(normalized, max_len=60)

        self.assertEqual(snippet, normalized)
        self.assertNotEqual(naive, normalized)
        self.assertTrue(naive.endswith("..."))

    def test_cjk_and_emoji_snippet_uses_display_width_limit(self) -> None:
        output = "执行结果🙂 你好世界 " * 8
        completed_line = _render_completed_line(output)
        snippet = _extract_snippet(completed_line)

        self.assertTrue(snippet.endswith("..."))
        self.assertLessEqual(_display_width(snippet), 60)

    def test_truncation_keeps_zwj_cluster_readable(self) -> None:
        output = "状态 👩\u200d💻 正常 " * 10
        completed_line = _render_completed_line(output)
        snippet = _extract_snippet(completed_line)

        self.assertTrue(snippet.endswith("..."))
        self.assertNotIn("\u200d...", snippet)

    def test_multiline_output_collapses_to_readable_single_row(self) -> None:
        output = "第一行\n第二行 emoji🙂\nCafe\u0301"
        completed_line = _render_completed_line(output)
        snippet = _extract_snippet(completed_line)

        self.assertEqual(snippet, "第一行 第二行 emoji🙂 Cafe\u0301")


if __name__ == "__main__":
    _ = unittest.main()
