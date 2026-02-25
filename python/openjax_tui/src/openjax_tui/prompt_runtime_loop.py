from __future__ import annotations

import asyncio
import contextlib
import os
import shutil
import time
import traceback
from dataclasses import dataclass
from typing import Any, Awaitable, Callable

from openjax_sdk import OpenJaxAsyncClient

from .approval import approval_toolbar_text as _approval_toolbar_text
from .debug_utils import normalize_history_for_prompt_toolkit as _normalize_history_for_prompt_toolkit
from .prompt_ui import history_text as _history_text
from .prompt_ui import invalidate_prompt_application as _invalidate_prompt_application
from .prompt_ui import refresh_history_view as _refresh_history_view
from .slash_commands import build_slash_command_completer as _build_slash_command_completer
from .slash_commands import slash_hint_text as _slash_hint_text
from .state import AppState, ViewMode
from .status_animation import get_status_indicator_text as _status_indicator_text
from .status_animation import stop_animation as _stop_status_animation
from .status_animation import sync_animation_controller as _sync_status_animation_controller
from .viewport_adapter import (
    HistoryViewportAdapter,
    PilotHistoryViewportAdapter,
    TextAreaHistoryViewportAdapter,
    resolve_history_viewport_impl as _resolve_history_viewport_impl,
    retain_live_viewport_blocks as _retain_live_viewport_blocks,
)


@dataclass
class PromptToolkitComponents:
    prompt_session_cls: Any | None
    patch_stdout: Any | None
    application_cls: Any | None
    text_area_cls: Any | None
    document_cls: Any | None
    layout_cls: Any | None
    hsplit_cls: Any | None
    vsplit_cls: Any | None
    window_cls: Any | None
    formatted_text_control_cls: Any | None
    condition_cls: Any | None
    conditional_container_cls: Any | None
    dimension_cls: Any | None
    completer_cls: Any | None
    completion_cls: Any | None
    run_in_terminal_fn: Callable[..., Any] | None


def prompt_toolkit_runtime_available(components: PromptToolkitComponents) -> bool:
    required = (
        components.prompt_session_cls,
        components.patch_stdout,
        components.application_cls,
        components.text_area_cls,
        components.document_cls,
        components.layout_cls,
        components.hsplit_cls,
        components.vsplit_cls,
        components.window_cls,
        components.formatted_text_control_cls,
        components.condition_cls,
        components.conditional_container_cls,
    )
    return all(item is not None for item in required)


def _status_line_text(state: AppState) -> str:
    flash_message = str(getattr(state, "approval_flash_message", "")).strip()
    if flash_message:
        flash_until = float(getattr(state, "approval_flash_until", 0.0))
        if time.monotonic() < flash_until:
            return flash_message
        state.approval_flash_message = ""
        state.approval_flash_until = 0.0
    return _status_indicator_text(state)


def compact_history_window(
    state: AppState,
    *,
    max_history_window_lines: int,
    tui_debug_fn: Callable[[str], None],
) -> list[str]:
    if state.view_mode == ViewMode.LIVE_VIEWPORT:
        return []
    current_lines = sum(block.count("\n") + 2 for block in state.history_blocks)
    if current_lines <= max_history_window_lines:
        return []

    dropped_blocks: list[str] = []
    dropped_lines = 0
    while current_lines > max_history_window_lines and len(state.history_blocks) > 1:
        if state.stream_block_index == 0:
            break
        removed = state.history_blocks.pop(0)
        dropped_blocks.append(removed)
        removed_lines = removed.count("\n") + 2
        dropped_lines += removed_lines
        current_lines -= removed_lines

        if state.stream_block_index is not None:
            state.stream_block_index = max(state.stream_block_index - 1, 0)

        updated_turn_index: dict[str, int] = {}
        for turn_id, idx in state.turn_block_index.items():
            next_idx = idx - 1
            if next_idx >= 0:
                updated_turn_index[turn_id] = next_idx
        state.turn_block_index = updated_turn_index

    if dropped_blocks:
        state.history_manual_scroll = max(int(state.history_manual_scroll) - dropped_lines, 0)
        tui_debug_fn(
            "history compacted dropped_blocks={blocks} dropped_lines={lines} remaining_blocks={remaining} remaining_lines={current}".format(
                blocks=len(dropped_blocks),
                lines=dropped_lines,
                remaining=len(state.history_blocks),
                current=current_lines,
            )
        )
    return dropped_blocks


async def fallback_prompt_toolkit_to_basic(
    client: OpenJaxAsyncClient,
    state: AppState,
    *,
    reason: str,
    run_input_loop_basic_fn: Callable[[OpenJaxAsyncClient, AppState], Awaitable[None]],
    request_prompt_redraw_fn: Callable[[AppState], None],
    finalize_stream_line_fn: Callable[[AppState], None],
) -> None:
    finalize_stream_line_fn(state)
    state.live_viewport_owner_turn_id = None
    state.live_viewport_turn_ownership.clear()
    state.history_setter = None
    state.prompt_invalidator = None
    state.input_backend = "basic"
    state.input_backend_reason = reason
    _stop_status_animation(
        state,
        request_prompt_redraw_fn=lambda: request_prompt_redraw_fn(state),
    )
    await run_input_loop_basic_fn(client, state)


async def run_prompt_toolkit_loop(
    client: OpenJaxAsyncClient,
    state: AppState,
    *,
    components: PromptToolkitComponents,
    key_bindings: Any,
    prompt_style: Any,
    slash_commands: tuple[str, ...],
    user_prompt_prefix: str,
    divider_line_fn: Callable[[], str],
    handle_user_line_fn: Callable[[OpenJaxAsyncClient, AppState, str], Awaitable[bool]],
    fallback_to_basic_fn: Callable[[OpenJaxAsyncClient, AppState, str], Awaitable[None]],
    request_prompt_redraw_fn: Callable[[AppState], None],
    drain_background_task_fn: Callable[[Any], Awaitable[None]],
    tui_log_info_fn: Callable[[str], None],
    tui_debug_fn: Callable[[str], None],
) -> None:
    if state.input_ready is None:
        raise RuntimeError("input gate is not initialized")
    if not prompt_toolkit_runtime_available(components):
        await fallback_to_basic_fn(client, state, "prompt_toolkit_unavailable")
        return

    state.approval_ui_enabled = True
    state.last_scrollback_flush_emitted = False
    line_queue: asyncio.Queue[str] = asyncio.Queue()
    loop = asyncio.get_running_loop()
    max_history_window_lines = max(
        120, int(os.environ.get("OPENJAX_TUI_HISTORY_WINDOW_LINES", "500"))
    )
    input_bottom_offset = max(1, int(os.environ.get("OPENJAX_TUI_INPUT_BOTTOM_OFFSET", "10")))

    def _schedule_scrollback_flush(blocks: list[str]) -> None:
        if not blocks:
            return
        scrollback_text = "\n\n".join(blocks).rstrip()
        if not scrollback_text:
            return
        if components.run_in_terminal_fn is None:
            return

        def _flush_output() -> None:
            if state.last_scrollback_flush_emitted:
                print()
            print(scrollback_text, flush=True)
            state.last_scrollback_flush_emitted = True

        future = components.run_in_terminal_fn(_flush_output)

        def _ignore_future_error(task: Any) -> None:
            with contextlib.suppress(Exception):
                task.result()

        done_callback = getattr(future, "add_done_callback", None)
        if callable(done_callback):
            done_callback(_ignore_future_error)

    history_adapter: HistoryViewportAdapter
    use_pilot_viewport = (
        state.view_mode == ViewMode.LIVE_VIEWPORT
        and _resolve_history_viewport_impl() == "pilot"
    )

    if use_pilot_viewport:
        pilot_history_text = "\n"

        def _set_pilot_history_text(value: str) -> None:
            nonlocal pilot_history_text
            pilot_history_text = value

        history_control = components.formatted_text_control_cls(lambda: pilot_history_text)
        history_window = components.window_cls(
            content=history_control,
            wrap_lines=True,
            always_hide_cursor=True,
            height=(
                components.dimension_cls(weight=1)
                if components.dimension_cls is not None
                else None
            ),
        )
        history_adapter = PilotHistoryViewportAdapter(
            state,
            history_window,
            set_text_fn=_set_pilot_history_text,
        )
    else:
        history_height = (
            components.dimension_cls(weight=1)
            if components.dimension_cls is not None
            else None
        )
        try:
            history_view = components.text_area_cls(
                text="\n",
                multiline=True,
                wrap_lines=True,
                read_only=True,
                focusable=False,
                scrollbar=True,
                height=history_height,
            )
        except TypeError:
            history_view = components.text_area_cls(
                text="\n",
                multiline=True,
                wrap_lines=True,
                read_only=True,
                focusable=False,
                height=history_height,
            )
        history_adapter = TextAreaHistoryViewportAdapter(
            state,
            history_view,
            document_cls=components.document_cls,
        )

    def _history_plain_text() -> str:
        return _normalize_history_for_prompt_toolkit(_history_text(state))

    def _render_history_view() -> None:
        dropped_blocks = _retain_live_viewport_blocks(state)
        dropped_blocks.extend(
            compact_history_window(
                state,
                max_history_window_lines=max_history_window_lines,
                tui_debug_fn=tui_debug_fn,
            )
        )
        history_adapter.refresh(_history_plain_text())
        if dropped_blocks:
            _schedule_scrollback_flush(dropped_blocks)

    def _accept_input(buffer: Any) -> bool:
        text = str(getattr(buffer, "text", ""))
        buffer.text = ""
        if state.input_ready is not None and not state.input_ready.is_set():
            return True
        loop.call_soon_threadsafe(line_queue.put_nowait, text)
        return True

    slash_completer = _build_slash_command_completer(
        slash_commands,
        components.completer_cls,
        components.completion_cls,
    )
    input_view = components.text_area_cls(
        prompt=f"{user_prompt_prefix} ",
        multiline=True,
        wrap_lines=True,
        accept_handler=_accept_input,
        completer=slash_completer,
        complete_while_typing=True,
    )
    slash_hint_panel = components.window_cls(
        content=components.formatted_text_control_cls(
            lambda: _slash_hint_text(str(getattr(input_view.buffer, "text", "")), slash_commands)
        ),
        dont_extend_height=True,
        height=1,
    )
    status_panel = components.window_cls(
        content=components.formatted_text_control_cls(
            lambda: _status_line_text(state)
        ),
        dont_extend_height=True,
        height=1,
    )

    def _border_line(left: str, right: str) -> str:
        width = max(shutil.get_terminal_size(fallback=(100, 24)).columns - 2, 8)
        return f"{left}{'─' * width}{right}"

    input_top_border = components.window_cls(
        content=components.formatted_text_control_cls(lambda: _border_line("╭", "╮")),
        dont_extend_height=True,
        height=1,
    )
    input_bottom_border = components.window_cls(
        content=components.formatted_text_control_cls(lambda: _border_line("╰", "╯")),
        dont_extend_height=True,
        height=1,
    )
    input_middle_row = components.vsplit_cls(
        [
            components.window_cls(
                content=components.formatted_text_control_cls("│"),
                width=1,
                dont_extend_height=True,
                height=1,
            ),
            input_view,
            components.window_cls(
                content=components.formatted_text_control_cls("│"),
                width=1,
                dont_extend_height=True,
                height=1,
            ),
        ],
        height=1,
    )

    approval_lines = max(1, input_bottom_offset - 1)
    approval_panel = components.window_cls(
        content=components.formatted_text_control_cls(
            lambda: _approval_toolbar_text(state, divider_line_fn())
        ),
        wrap_lines=True,
        always_hide_cursor=True,
        height=approval_lines,
        dont_extend_height=True,
    )

    root_container = components.hsplit_cls(
        [
            history_adapter.container,
            components.window_cls(height=1, char=" "),
            status_panel,
            input_top_border,
            input_middle_row,
            input_bottom_border,
            slash_hint_panel,
            approval_panel,
        ]
    )
    app = components.application_cls(
        layout=components.layout_cls(root_container, focused_element=input_view),
        key_bindings=key_bindings,
        style=prompt_style,
        full_screen=False,
    )

    state.prompt_invalidator = lambda: _invalidate_prompt_application(app)

    def _refresh_history_with_tail() -> None:
        _render_history_view()
        with contextlib.suppress(Exception):
            history_adapter.sync_manual_scroll_from_render()
        _invalidate_prompt_application(app)

    state.history_setter = _refresh_history_with_tail
    _sync_status_animation_controller(
        state,
        request_prompt_redraw_fn=lambda: request_prompt_redraw_fn(state),
    )

    if key_bindings is not None:
        @key_bindings.add("pageup", eager=True)
        def _history_pageup(event: object) -> None:
            with contextlib.suppress(Exception):
                history_adapter.set_manual_scroll(history_adapter.current_scroll() - 20)
            app_obj = getattr(event, "app", None)
            if app_obj is not None:
                app_obj.invalidate()

        @key_bindings.add("pagedown", eager=True)
        def _history_pagedown(event: object) -> None:
            with contextlib.suppress(Exception):
                max_scroll = history_adapter.max_scroll()
                next_scroll = history_adapter.current_scroll() + 20
                if max_scroll is not None:
                    next_scroll = min(next_scroll, max_scroll)
                    history_adapter.set_manual_scroll(next_scroll)
                    if next_scroll >= max_scroll:
                        history_adapter.follow_tail()
                else:
                    history_adapter.set_manual_scroll(next_scroll)
                if max_scroll == 0:
                    history_adapter.follow_tail()
            app_obj = getattr(event, "app", None)
            if app_obj is not None:
                app_obj.invalidate()

    _refresh_history_view(state)
    app_task: asyncio.Task[None] = asyncio.create_task(app.run_async())
    try:
        while state.running:
            if app_task.done():
                with contextlib.suppress(asyncio.CancelledError):
                    exc = app_task.exception()
                    if exc is not None:
                        tui_log_info_fn(
                            f"prompt_toolkit loop failed type={type(exc).__name__} message={exc}"
                        )
                        tui_debug_fn(
                            "prompt_toolkit loop traceback\n"
                            + "".join(
                                traceback.format_exception(
                                    type(exc),
                                    exc,
                                    exc.__traceback__,
                                )
                            )
                        )
                        raise exc
                tui_log_info_fn(
                    "prompt_toolkit loop exited unexpectedly; fallback to basic backend"
                )
                await fallback_to_basic_fn(client, state, "prompt_toolkit_exited_early")
                return
            try:
                line = await asyncio.wait_for(line_queue.get(), timeout=0.2)
            except asyncio.TimeoutError:
                continue
            except EOFError:
                state.running = False
                return
            except KeyboardInterrupt:
                state.running = False
                raise
            except asyncio.CancelledError:
                state.running = False
                return

            if not await handle_user_line_fn(client, state, line):
                return
    finally:
        _stop_status_animation(
            state,
            request_prompt_redraw_fn=lambda: request_prompt_redraw_fn(state),
        )
        state.history_setter = None
        state.prompt_invalidator = None
        if getattr(app, "is_running", False):
            with contextlib.suppress(Exception):
                app.exit(result=None)
        await drain_background_task_fn(app_task)
