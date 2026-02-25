# pyright: reportAny=false, reportUnknownArgumentType=false, reportUnknownLambdaType=false, reportPrivateUsage=false, reportPrivateLocalImportUsage=false, reportUnusedImport=false, reportUnusedCallResult=false

import io
import unittest
from contextlib import redirect_stdout
from unittest.mock import patch

from openjax_sdk.models import EventEnvelope
from openjax_tui import assistant_render
from openjax_tui import event_dispatch
from openjax_tui import tool_runtime as tr
from openjax_tui.state import AppState, ToolTurnStats


def _evt(turn_id: str, event_type: str, payload: dict[str, object]) -> EventEnvelope:
    return EventEnvelope(
        protocol_version="v1",
        kind="event",
        session_id="s1",
        turn_id=turn_id,
        event_type=event_type,
        payload=payload,
    )


class ToolSummaryTest(unittest.TestCase):
    def _print_event(self, state: AppState, evt: EventEnvelope) -> None:
        event_dispatch.print_event(
            evt,
            state=state,
            print_tool_turn_summary=False,
            render_assistant_delta_fn=lambda *_: None,
            render_assistant_message_fn=lambda *_: None,
            finalize_stream_line_if_turn_fn=lambda *_: None,
            record_tool_started_fn=lambda turn, tool_name: tr.record_tool_started(
                state, turn, tool_name, monotonic_fn=__import__("time").monotonic
            ),
            record_tool_completed_fn=lambda turn, tool_name, ok: tr.record_tool_completed(
                state,
                turn,
                tool_name,
                ok,
                monotonic_fn=__import__("time").monotonic,
                tool_turn_stats_cls=ToolTurnStats,
            ),
            print_tool_call_result_line_fn=lambda s, tool_name, ok, output, elapsed_ms=0, target_hint=None: tr.print_tool_call_result_line(
                s,
                tool_name,
                ok,
                output,
                status_bullet_fn=lambda _ok: "⏺",
                tool_result_label_fn=assistant_render.tool_result_label,
                finalize_stream_line_fn=assistant_render.finalize_stream_line,
                emit_ui_spacer_fn=lambda _s: None,
                emit_ui_line_fn=lambda _s, text: print(text),
                elapsed_ms=elapsed_ms,
                target_hint=target_hint,
            ),
            use_inline_approval_panel_fn=lambda _s: False,
            print_tool_summary_for_turn_fn=lambda *_args, **_kwargs: None,
        )

    def test_tool_events_print_single_line_per_completion(self) -> None:
        state = AppState()
        out = io.StringIO()

        with redirect_stdout(out), patch("time.monotonic", side_effect=[1.0, 1.3, 2.0, 2.25]):
            self._print_event(state, _evt("1", "tool_call_started", {"tool_name": "shell"}))
            self._print_event(
                state,
                _evt(
                    "1",
                    "tool_call_completed",
                    {"tool_name": "shell", "ok": True, "output": "command finished successfully"},
                ),
            )
            self._print_event(state, _evt("1", "tool_call_started", {"tool_name": "search"}))
            self._print_event(
                state,
                _evt(
                    "1",
                    "tool_call_completed",
                    {"tool_name": "search", "ok": False, "output": "timeout waiting for response"},
                ),
            )

        text = out.getvalue()
        self.assertIn("⏺ Run shell command · 300ms", text)
        self.assertIn("⏺ Search · 250ms · timeout waiting for response", text)
        self.assertNotIn("[started]", text)
        self.assertNotIn("[running]", text)
        self.assertNotIn("[completed]", text)

    def test_read_file_completion_line_includes_target(self) -> None:
        state = AppState()
        out = io.StringIO()

        with redirect_stdout(out), patch("time.monotonic", side_effect=[1.0, 1.005]):
            self._print_event(state, _evt("1", "tool_call_started", {"tool_name": "read_file"}))
            self._print_event(
                state,
                _evt(
                    "1",
                    "tool_call_completed",
                    {
                        "tool_name": "read_file",
                        "ok": True,
                        "output": "L1: hello",
                        "args": {"file_path": "test.txt"},
                    },
                ),
            )

        self.assertRegex(out.getvalue(), r"⏺ Read 1 file \(test\.txt\) · \d+ms")

    def test_duration_switches_to_seconds_and_minutes(self) -> None:
        state = AppState()
        out = io.StringIO()

        with redirect_stdout(out), patch("time.monotonic", side_effect=[1.0, 20.304, 30.0, 95.4]):
            self._print_event(state, _evt("1", "tool_call_started", {"tool_name": "shell"}))
            self._print_event(
                state,
                _evt(
                    "1",
                    "tool_call_completed",
                    {"tool_name": "shell", "ok": True, "output": "done"},
                ),
            )
            self._print_event(state, _evt("2", "tool_call_started", {"tool_name": "shell"}))
            self._print_event(
                state,
                _evt(
                    "2",
                    "tool_call_completed",
                    {"tool_name": "shell", "ok": True, "output": "done"},
                ),
            )

        text = out.getvalue()
        self.assertIn("Run shell command · 19.3s", text)
        self.assertIn("Run shell command · 1m05.4s", text)

    def test_status_bullet_colored_dot_in_prompt_toolkit_backend(self) -> None:
        state = AppState()
        state.input_backend = "prompt_toolkit"

        with patch("openjax_tui.startup_ui._supports_ansi_color", return_value=True):
            bullet = tr.status_bullet(
                state=state,
                ok=True,
                assistant_prefix="⏺",
                ansi_green="\x1b[32m",
                ansi_red="\x1b[31m",
                ansi_reset="\x1b[0m",
                supports_ansi_color_fn=lambda: True,
            )
        self.assertEqual(bullet, "\x1b[32m⏺\x1b[0m")

    def test_tool_runtime_status_bullet_without_ansi(self) -> None:
        state = AppState()
        bullet = tr.status_bullet(
            state=state,
            ok=True,
            assistant_prefix="⏺",
            ansi_green="\x1b[32m",
            ansi_red="\x1b[31m",
            ansi_reset="\x1b[0m",
            supports_ansi_color_fn=lambda: False,
        )
        self.assertEqual(bullet, "⏺")

    def test_tool_line_appends_to_history_in_prompt_toolkit_backend(self) -> None:
        state = AppState()
        state.input_backend = "prompt_toolkit"

        def _emit_ui_line(s: AppState, text: str) -> None:
            if s.input_backend == "prompt_toolkit":
                s.history_blocks.append(text)

        with patch("time.monotonic", side_effect=[1.0, 1.2]):
            event_dispatch.print_event(
                _evt("1", "tool_call_started", {"tool_name": "shell"}),
                state=state,
                print_tool_turn_summary=False,
                render_assistant_delta_fn=lambda *_: None,
                render_assistant_message_fn=lambda *_: None,
                finalize_stream_line_if_turn_fn=lambda *_: None,
                record_tool_started_fn=lambda turn, tool_name: tr.record_tool_started(
                    state, turn, tool_name, monotonic_fn=__import__("time").monotonic
                ),
                record_tool_completed_fn=lambda turn, tool_name, ok: tr.record_tool_completed(
                    state,
                    turn,
                    tool_name,
                    ok,
                    monotonic_fn=__import__("time").monotonic,
                    tool_turn_stats_cls=ToolTurnStats,
                ),
                print_tool_call_result_line_fn=lambda s, tool_name, ok, output, elapsed_ms=0, target_hint=None: tr.print_tool_call_result_line(
                    s,
                    tool_name,
                    ok,
                    output,
                    status_bullet_fn=lambda _ok: "⏺",
                    tool_result_label_fn=assistant_render.tool_result_label,
                    finalize_stream_line_fn=assistant_render.finalize_stream_line,
                    emit_ui_spacer_fn=lambda _s: None,
                    emit_ui_line_fn=_emit_ui_line,
                    elapsed_ms=elapsed_ms,
                    target_hint=target_hint,
                ),
                use_inline_approval_panel_fn=lambda _s: False,
                print_tool_summary_for_turn_fn=lambda *_args, **_kwargs: None,
            )
            event_dispatch.print_event(
                _evt("1", "tool_call_completed", {"tool_name": "shell", "ok": True}),
                state=state,
                print_tool_turn_summary=False,
                render_assistant_delta_fn=lambda *_: None,
                render_assistant_message_fn=lambda *_: None,
                finalize_stream_line_if_turn_fn=lambda *_: None,
                record_tool_started_fn=lambda turn, tool_name: tr.record_tool_started(
                    state, turn, tool_name, monotonic_fn=__import__("time").monotonic
                ),
                record_tool_completed_fn=lambda turn, tool_name, ok: tr.record_tool_completed(
                    state,
                    turn,
                    tool_name,
                    ok,
                    monotonic_fn=__import__("time").monotonic,
                    tool_turn_stats_cls=ToolTurnStats,
                ),
                print_tool_call_result_line_fn=lambda s, tool_name, ok, output, elapsed_ms=0, target_hint=None: tr.print_tool_call_result_line(
                    s,
                    tool_name,
                    ok,
                    output,
                    status_bullet_fn=lambda _ok: "⏺",
                    tool_result_label_fn=assistant_render.tool_result_label,
                    finalize_stream_line_fn=assistant_render.finalize_stream_line,
                    emit_ui_spacer_fn=lambda _s: None,
                    emit_ui_line_fn=_emit_ui_line,
                    elapsed_ms=elapsed_ms,
                    target_hint=target_hint,
                ),
                use_inline_approval_panel_fn=lambda _s: False,
                print_tool_summary_for_turn_fn=lambda *_args, **_kwargs: None,
            )
        self.assertEqual(len(state.history_blocks), 1)
        self.assertIn("Run shell command · 199ms", state.history_blocks[0])

    def test_event_dispatch_ignores_turn_started(self) -> None:
        called: list[str] = []
        event_dispatch.print_event(
            _evt("1", "turn_started", {}),
            state=None,
            print_tool_turn_summary=False,
            render_assistant_delta_fn=lambda *_: called.append("delta"),
            render_assistant_message_fn=lambda *_: called.append("message"),
            finalize_stream_line_if_turn_fn=lambda *_: called.append("finalize"),
            record_tool_started_fn=lambda *_: called.append("tool_start"),
            record_tool_completed_fn=lambda *_: called.append("tool_done") or 0,
            print_tool_call_result_line_fn=lambda *_, **__: called.append("tool_line"),
            use_inline_approval_panel_fn=lambda *_: False,
            print_tool_summary_for_turn_fn=lambda *_, **__: called.append("summary"),
        )
        self.assertEqual(called, [])


if __name__ == "__main__":
    _ = unittest.main()
