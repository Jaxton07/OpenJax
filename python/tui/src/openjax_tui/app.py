"""Main application for OpenJax TUI."""

from __future__ import annotations

import asyncio
import logging
from collections.abc import Callable, Coroutine

from textual.app import App
from textual.reactive import reactive

from .event_mapper import UiOperation, map_event
from .logging_setup import get_logger
from .screens.chat import ChatScreen
from .sdk_runtime import SdkRuntime
from .state import AppState, TurnPhase

logger = logging.getLogger("openjax_tui")


class OpenJaxApp(App):
    """OpenJax TUI main application."""

    TITLE = "OpenJax"
    SUB_TITLE = "AI Agent Framework"
    CSS_PATH = "styles.tcss"

    # Reactive state (synced with AppState)
    session_id: reactive[str | None] = reactive(None)
    turn_phase: reactive[TurnPhase] = reactive(TurnPhase.IDLE)
    current_input: reactive[str] = reactive("")

    def __init__(
        self,
        runtime_factory: Callable[..., SdkRuntime] | None = None,
        **kwargs,
    ) -> None:
        """Initialize the application."""
        super().__init__(**kwargs)
        self.state = AppState()
        self._runtime_factory = runtime_factory or SdkRuntime
        self._runtime: SdkRuntime | None = None
        self._background_tasks: set[asyncio.Task[None]] = set()
        self._stream_render_scheduled = False
        get_logger()
        logger.info("app initialized")

    async def on_mount(self) -> None:
        """Push chat screen and initialize SDK runtime."""
        logger.info("mounting chat screen")
        self.push_screen(ChatScreen())
        # Defer first render until child widgets are mounted.
        self.call_after_refresh(self._render_state)
        await self._ensure_runtime_started()

    async def on_unmount(self) -> None:
        """Shutdown runtime when app is unmounted."""
        await self._stop_runtime(graceful=True)

    def action_help_quit(self) -> None:
        """Override to quit immediately without confirmation."""
        self.exit()

    def action_help(self) -> None:
        """Show help information."""
        logger.info("action_help triggered")
        help_text = """
[bold cyan]OpenJax TUI 帮助[/bold cyan]

[bold]快捷键:[/bold]
  Ctrl+C (macOS) / Ctrl+Q (Linux/Windows) - 退出程序
  / - 打开命令面板
  Escape - 关闭命令面板

[bold]命令:[/bold]
  /help    - 显示此帮助信息
  /clear   - 清空对话历史
  /exit    - 退出程序
  /approve - 批准当前审批请求
  /deny    - 拒绝当前审批请求
  /pending - 查看待处理审批

[bold]使用说明:[/bold]
  直接输入消息按回车发送
  输入 / 触发命令面板，支持模糊搜索
        """
        self._add_system_message(help_text)

    def action_clear(self) -> None:
        """Clear conversation history."""
        logger.info("action_clear triggered")
        self.state.clear_messages()
        self._add_system_message("[dim]对话历史已清空[/dim]")

    def action_exit(self) -> None:
        """Exit the application."""
        logger.info("action_exit triggered")
        self.exit()

    def action_pending(self) -> None:
        """Show pending approvals."""
        count = self.state.get_pending_approval_count()
        logger.info("action_pending triggered count=%s", count)
        if count == 0:
            self._add_system_message("[dim]没有待处理的审批请求[/dim]")
            return

        lines = [f"[yellow]待处理审批 {count} 个:[/yellow]"]
        for approval_id in self.state.approval_order:
            approval = self.state.pending_approvals.get(approval_id)
            if approval is None:
                continue
            marker = "*" if approval_id == self.state.approval_focus_id else " "
            lines.append(
                f"{marker} {approval.id}  action={approval.action} turn={approval.turn_id or '-'}"
            )
        self._add_system_message("\n".join(lines))

    def action_approve(self) -> None:
        """Approve the focused pending approval request."""
        self._resolve_focused_approval(True)

    def action_deny(self) -> None:
        """Deny the focused pending approval request."""
        self._resolve_focused_approval(False)

    def handle_approval_popup_selection(self, option_name: str) -> None:
        """Handle a confirmed selection from approval popup."""
        self._handle_popup_selection(option_name)

    def action_command_palette(self) -> None:
        """Open the command palette."""
        logger.info("action_command_palette triggered")
        screen = self._get_chat_screen()
        if screen is not None:
            screen.show_command_palette()

    def submit_message(self, text: str) -> None:
        """Submit a user message."""
        logger.info("submit_message text_len=%s", len(text))
        self.state.add_message("user", text)
        self._render_state()

        self.state.set_turn_phase(TurnPhase.THINKING)
        self.turn_phase = TurnPhase.THINKING
        self._spawn_task(self._submit_turn_async(text))

    async def _submit_turn_async(self, text: str) -> None:
        try:
            await self._ensure_runtime_started()
            if self._runtime is None:
                raise RuntimeError("SDK runtime unavailable")
            turn_id = await self._runtime.submit_turn(text)
            logger.info("submit_turn success turn_id=%s", turn_id)
        except Exception as err:
            await self._handle_runtime_error(err)

    async def _resolve_approval_async(
        self,
        *,
        turn_id: str,
        request_id: str,
        approved: bool,
    ) -> None:
        try:
            await self._ensure_runtime_started()
            if self._runtime is None:
                raise RuntimeError("SDK runtime unavailable")
            await self._runtime.resolve_approval(
                turn_id=turn_id,
                request_id=request_id,
                approved=approved,
            )
            logger.info(
                "resolve_approval submitted request_id=%s turn_id=%s approved=%s",
                request_id,
                turn_id,
                approved,
            )
        except Exception as err:
            await self._handle_runtime_error(err)

    async def _ensure_runtime_started(self) -> None:
        if self._runtime is not None:
            return
        self._runtime = self._runtime_factory(
            on_event=self._handle_runtime_event,
            on_error=self._handle_runtime_error,
        )
        try:
            session_id = await self._runtime.start()
        except Exception as err:
            logger.exception("runtime start failed")
            self._runtime = None
            await self._handle_runtime_error(err)
            return

        self.state.session_id = session_id
        self.session_id = session_id
        logger.info("runtime started session_id=%s", session_id)

    async def _stop_runtime(self, graceful: bool) -> None:
        runtime = self._runtime
        self._runtime = None
        if runtime is None:
            return
        await runtime.stop(graceful=graceful)

    async def _handle_runtime_event(self, evt) -> None:
        logger.info("event received type=%s turn_id=%s", evt.event_type, evt.turn_id)
        ops = map_event(evt, self.state)
        self._apply_ui_operations(ops)

    async def _handle_runtime_error(self, err: Exception) -> None:
        logger.exception("runtime error: %s", err)
        self.state.set_error(str(err))
        self.turn_phase = TurnPhase.ERROR
        self._add_system_message(f"[red]错误: {err}[/red]")
        self.state.set_turn_phase(TurnPhase.IDLE)
        self.turn_phase = TurnPhase.IDLE

    def _apply_ui_operations(self, ops: list[UiOperation]) -> None:
        needs_render = False
        stream_updated = False
        approval_changed = False

        for op in ops:
            if op.kind == "turn_completed" and op.turn_id:
                if op.text:
                    render_kind = self.state.turn_render_kind_by_turn.pop(
                        op.turn_id, "markdown"
                    )
                    self.state.add_message(
                        "assistant",
                        op.text,
                        turn_id=op.turn_id,
                        render_kind=render_kind,
                    )
                needs_render = True
            elif op.kind == "stream_updated":
                stream_updated = True
            elif op.kind in {
                "phase_changed",
                "approval_added",
                "approval_removed",
                "tool_call_completed",
            }:
                needs_render = True
                if op.kind in {"approval_added", "approval_removed"}:
                    approval_changed = True

        self.turn_phase = self.state.turn_phase

        if stream_updated:
            self._schedule_stream_render()
        elif needs_render:
            self._render_state()

        if approval_changed:
            self._sync_approval_popup()

    def _schedule_stream_render(self) -> None:
        if self._stream_render_scheduled:
            return
        self._stream_render_scheduled = True
        try:
            self.set_timer(0.04, self._flush_stream_render)
        except Exception:
            self._flush_stream_render()

    def _flush_stream_render(self) -> None:
        self._stream_render_scheduled = False
        self._render_state()

    def _add_system_message(self, text: str) -> None:
        self.state.add_message("system", text)
        self._render_state()

    def _sync_approval_popup(self) -> None:
        """Synchronize approval popup visibility with pending approvals."""
        if self.state.get_pending_approval_count() == 0:
            self._hide_approval_popup()
            return
        self._show_approval_popup_for_focus()

    def _show_approval_popup_for_focus(self) -> None:
        """Show popup for the currently focused approval request."""
        screen = self._get_chat_screen()
        if screen is None:
            return

        approval = self.state.latest_pending_approval()
        if self.state.approval_focus_id:
            approval = self.state.pending_approvals.get(self.state.approval_focus_id, approval)
        if approval is None:
            self._hide_approval_popup()
            return
        screen.show_approval_popup(approval)

    def _hide_approval_popup(self) -> None:
        """Hide approval popup and restore input."""
        screen = self._get_chat_screen()
        if screen is None:
            return
        screen.dismiss_approval_popup()

    def _handle_popup_selection(self, option_name: str) -> None:
        """Dispatch popup selection action."""
        if option_name == "approve":
            self.action_approve()
            return
        if option_name == "deny":
            self.action_deny()
            return
        if option_name == "cancel":
            self._hide_approval_popup()
            self._add_system_message("[dim]审批已暂存，可稍后处理[/dim]")
            return
        logger.warning("unknown approval popup option=%s", option_name)

    def _resolve_focused_approval(self, approved: bool) -> None:
        approval = self.state.latest_pending_approval()
        if self.state.approval_focus_id:
            approval = self.state.pending_approvals.get(self.state.approval_focus_id, approval)

        if approval is None:
            self._add_system_message("[dim]没有待处理的审批请求[/dim]")
            return

        if not approval.turn_id:
            self._add_system_message(f"[red]审批缺少 turn_id: {approval.id}[/red]")
            return

        action_text = "批准" if approved else "拒绝"
        self._add_system_message(f"[dim]已提交{action_text}请求: {approval.id}[/dim]")
        self._spawn_task(
            self._resolve_approval_async(
                turn_id=approval.turn_id,
                request_id=approval.id,
                approved=approved,
            )
        )

    def _render_state(self) -> None:
        screen = self._get_chat_screen()
        if screen is not None:
            screen.render_state(self.state)

    def _get_chat_screen(self) -> ChatScreen | None:
        if isinstance(self.screen, ChatScreen):
            return self.screen
        return None

    def _spawn_task(self, coroutine: Coroutine[object, object, None]) -> None:
        try:
            loop = asyncio.get_running_loop()
        except RuntimeError:
            asyncio.run(coroutine)
            return

        task = loop.create_task(coroutine)
        self._background_tasks.add(task)

        def _done(completed: asyncio.Task[None]) -> None:
            self._background_tasks.discard(completed)

        task.add_done_callback(_done)
