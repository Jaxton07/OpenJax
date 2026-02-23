import io
import unittest
from contextlib import redirect_stdout
from unittest.mock import patch

from openjax_sdk.models import EventEnvelope
from openjax_tui.app import AppState
from openjax_tui import event_dispatch
from openjax_tui import assistant_render as render_utils
from openjax_tui import tool_runtime


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
        from openjax_tui import assistant_render, tool_runtime as tr

        def _finalize_stream_line_if_turn(turn: str) -> None:
            assistant_render.finalize_stream_line_if_turn(
                state, turn, finalize_stream_line_fn=assistant_render.finalize_stream_line
            )

        event_dispatch.print_event(
            evt,
            state=state,
            print_tool_turn_summary=False,
            render_assistant_delta_fn=lambda turn, delta: assistant_render.render_assistant_delta(
                state, turn, delta,
                assistant_prefix="⏺",
                align_multiline_fn=lambda t: t.replace("\n", "\n  "),
                finalize_stream_line_fn=assistant_render.finalize_stream_line,
                refresh_history_view_fn=lambda s: None,
            ),
            render_assistant_message_fn=lambda turn, content: assistant_render.render_assistant_message(
                state, turn, content,
                assistant_prefix="⏺",
                print_prefixed_block_fn=lambda s, p, c: print(f"{p} {c.replace(chr(10), chr(10)+'  ')}"),
                finalize_stream_line_fn=assistant_render.finalize_stream_line,
            ),
            finalize_stream_line_if_turn_fn=_finalize_stream_line_if_turn,
            record_tool_started_fn=lambda turn, tool_name: tr.record_tool_started(
                state, turn, tool_name, monotonic_fn=__import__('time').monotonic
            ),
            record_tool_completed_fn=lambda turn, tool_name, ok: tr.record_tool_completed(
                state, turn, tool_name, ok,
                monotonic_fn=__import__('time').monotonic,
                tool_turn_stats_cls=__import__('openjax_tui.state', fromlist=['ToolTurnStats']).ToolTurnStats
            ),
            print_tool_call_result_line_fn=lambda s, tool_name, ok, output: tr.print_tool_call_result_line(
                s, tool_name, ok, output,
                status_bullet_fn=lambda ok: "🟢" if ok else "🔴",
                tool_result_label_fn=assistant_render.tool_result_label,
                finalize_stream_line_fn=assistant_render.finalize_stream_line,
                emit_ui_spacer_fn=lambda s: None,
                emit_ui_line_fn=lambda s, text: print(text),
            ),
            use_inline_approval_panel_fn=lambda s: False,
            print_tool_summary_for_turn_fn=lambda s, turn: tr.print_tool_summary_for_turn(
                s, turn,
                status_bullet_fn=lambda ok: "🟢" if ok else "🔴",
                finalize_stream_line_fn=assistant_render.finalize_stream_line,
                emit_ui_line_fn=lambda s, text: print(text),
            ),
        )

    def _status_bullet(self, state: AppState, ok: bool) -> str:
        from openjax_tui.startup_ui import _supports_ansi_color
        return tool_runtime.status_bullet(
            state=state,
            ok=ok,
            assistant_prefix="⏺",
            ansi_green="\x1b[32m",
            ansi_red="\x1b[31m",
            ansi_reset="\x1b[0m",
            supports_ansi_color_fn=_supports_ansi_color,
        )

    def test_tool_events_print_immediate_lines(self) -> None:
        state = AppState()
        out = io.StringIO()

        import time
        with redirect_stdout(out), patch(
            "time.monotonic", side_effect=[1.0, 1.3, 2.0, 2.25]
        ), patch("openjax_tui.startup_ui._supports_ansi_color", return_value=False):
            self._print_event(state, _evt("1", "tool_call_started", {"tool_name": "shell"}))
            self._print_event(state, _evt("1", "tool_call_completed", {"tool_name": "shell", "ok": True}))
            self._print_event(state, _evt("1", "tool_call_started", {"tool_name": "search"}))
            self._print_event(state, _evt("1", "tool_call_completed", {"tool_name": "search", "ok": False}))
            self._print_event(state, _evt("1", "turn_completed", {}))

        text = out.getvalue()
        self.assertIn("🟢 Run shell command", text)
        self.assertIn("🔴 Search (failed)", text)
        self.assertNotIn("tools: calls=", text)

    def test_status_bullet_colored_dot_in_prompt_toolkit_backend(self) -> None:
        state = AppState()
        state.input_backend = "prompt_toolkit"

        with patch("openjax_tui.startup_ui._supports_ansi_color", return_value=True):
            bullet = self._status_bullet(state, ok=True)

        self.assertEqual(bullet, "\x1b[32m⏺\x1b[0m")

    def test_tool_line_appends_to_history_in_prompt_toolkit_backend(self) -> None:
        state = AppState()
        state.input_backend = "prompt_toolkit"

        def _print_event_with_history(state: AppState, evt: EventEnvelope) -> None:
            from openjax_tui import assistant_render, tool_runtime as tr

            def _emit_ui_line(s: AppState, text: str) -> None:
                if s.input_backend == "prompt_toolkit":
                    s.history_blocks.append(text)

            def _print_tool_call_result_line(s: AppState, tool_name: str, ok: bool, output: str) -> None:
                tr.print_tool_call_result_line(
                    s, tool_name, ok, output,
                    status_bullet_fn=lambda ok: "🟢" if ok else "🔴",
                    tool_result_label_fn=assistant_render.tool_result_label,
                    finalize_stream_line_fn=assistant_render.finalize_stream_line,
                    emit_ui_spacer_fn=lambda s: None,
                    emit_ui_line_fn=_emit_ui_line,
                )

            event_dispatch.print_event(
                evt,
                state=state,
                print_tool_turn_summary=False,
                render_assistant_delta_fn=lambda turn, delta: None,
                render_assistant_message_fn=lambda turn, content: None,
                finalize_stream_line_if_turn_fn=lambda turn: None,
                record_tool_started_fn=lambda turn, tool_name: tr.record_tool_started(
                    state, turn, tool_name, monotonic_fn=__import__('time').monotonic
                ),
                record_tool_completed_fn=lambda turn, tool_name, ok: tr.record_tool_completed(
                    state, turn, tool_name, ok,
                    monotonic_fn=__import__('time').monotonic,
                    tool_turn_stats_cls=__import__('openjax_tui.state', fromlist=['ToolTurnStats']).ToolTurnStats
                ),
                print_tool_call_result_line_fn=_print_tool_call_result_line,
                use_inline_approval_panel_fn=lambda s: False,
                print_tool_summary_for_turn_fn=lambda s, turn: None,
            )

        import time
        with patch("time.monotonic", side_effect=[1.0, 1.2]), patch(
            "openjax_tui.startup_ui._supports_ansi_color", return_value=True
        ):
            _print_event_with_history(state, _evt("1", "tool_call_started", {"tool_name": "shell"}))
            _print_event_with_history(state, _evt("1", "tool_call_completed", {"tool_name": "shell", "ok": True}))
            _print_event_with_history(state, _evt("1", "turn_completed", {}))

        self.assertEqual(len(state.history_blocks), 1)
        self.assertIn("Run shell command", state.history_blocks[0])

    def test_render_utils_module_helpers(self) -> None:
        self.assertEqual(render_utils.align_multiline("a\nb", "  "), "a\n  b")
        self.assertEqual(
            render_utils.tool_result_label("apply_patch", "UPDATE test.txt"),
            "Update(test.txt)",
        )

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
            record_tool_completed_fn=lambda *_: called.append("tool_done"),
            print_tool_call_result_line_fn=lambda *_, **__: called.append("tool_line"),
            use_inline_approval_panel_fn=lambda *_: False,
            print_tool_summary_for_turn_fn=lambda *_, **__: called.append("summary"),
        )
        self.assertEqual(called, [])

    def test_tool_runtime_status_bullet_without_ansi(self) -> None:
        state = AppState()
        self.assertEqual(
            tool_runtime.status_bullet(
                state=state,
                ok=True,
                assistant_prefix="⏺",
                ansi_green="\x1b[32m",
                ansi_red="\x1b[31m",
                ansi_reset="\x1b[0m",
                supports_ansi_color_fn=lambda: False,
            ),
            "🟢",
        )


if __name__ == "__main__":
    unittest.main()
