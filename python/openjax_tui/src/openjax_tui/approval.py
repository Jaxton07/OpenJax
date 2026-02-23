from __future__ import annotations

import contextlib
from typing import Any

from openjax_sdk.exceptions import OpenJaxResponseError

from .state import AppState


def print_pending(state: AppState) -> None:
    if not state.pending_approvals:
        print("[approval] no pending approvals")
        return
    print("[approval] pending:")
    for request_id in list(state.approval_order):
        record = state.pending_approvals.get(request_id)
        if record and record.status == "pending":
            focus_marker = "*" if request_id == state.approval_focus_id else " "
            print(
                f" {focus_marker} {request_id} (turn:{record.turn_id}) target={record.target or '-'}"
            )


def use_inline_approval_panel(state: AppState) -> bool:
    return state.input_backend == "prompt_toolkit" and state.approval_ui_enabled


def pop_pending(state: AppState, approval_request_id: str) -> None:
    state.pending_approvals.pop(approval_request_id, None)
    with contextlib.suppress(ValueError):
        state.approval_order.remove(approval_request_id)
    if state.approval_focus_id == approval_request_id:
        state.approval_focus_id = focused_approval_id(state)


def focused_approval_id(state: AppState) -> str | None:
    if state.approval_focus_id and state.approval_focus_id in state.pending_approvals:
        return state.approval_focus_id
    while state.approval_order:
        approval_request_id = state.approval_order[-1]
        record = state.pending_approvals.get(approval_request_id)
        if record and record.status == "pending":
            state.approval_focus_id = approval_request_id
            return approval_request_id
        state.approval_order.pop()
    state.approval_focus_id = None
    return None


def approval_mode_active(state: AppState) -> bool:
    return state.turn_phase == "approval" and focused_approval_id(state) is not None


def input_prompt_prefix(state: AppState, user_prompt_prefix: str) -> str:
    if use_inline_approval_panel(state):
        return user_prompt_prefix
    if approval_mode_active(state):
        return "approval>"
    return user_prompt_prefix


def approval_pending_ids(state: AppState) -> list[str]:
    ids: list[str] = []
    for approval_request_id in state.approval_order:
        record = state.pending_approvals.get(approval_request_id)
        if record and record.status == "pending":
            ids.append(approval_request_id)
    return ids


def move_approval_focus(state: AppState, step: int) -> None:
    pending_ids = approval_pending_ids(state)
    if not pending_ids:
        state.approval_focus_id = None
        return
    focus_id = focused_approval_id(state)
    if focus_id is None or focus_id not in pending_ids:
        state.approval_focus_id = pending_ids[-1]
        return
    idx = pending_ids.index(focus_id)
    next_idx = max(0, min(len(pending_ids) - 1, idx + step))
    state.approval_focus_id = pending_ids[next_idx]


def toggle_approval_selection(state: AppState) -> None:
    state.approval_selected_action = (
        "deny" if state.approval_selected_action == "allow" else "allow"
    )


def approval_toolbar_text(state: AppState, divider_line: str) -> str:
    if not state.approval_ui_enabled or not approval_mode_active(state):
        return ""
    focus_id = focused_approval_id(state)
    if not focus_id:
        return ""
    record = state.pending_approvals.get(focus_id)
    if record is None:
        return ""
    total = len(approval_pending_ids(state))
    selected_allow = state.approval_selected_action == "allow"
    target = (record.target or "-").strip() or "-"
    reason = " ".join(str(record.reason or "-").split())
    allow_label = "❯ 1. Yes" if selected_allow else "  1. Yes"
    deny_label = "❯ 2. No" if not selected_allow else "  2. No"
    lines = [
        divider_line,
        f" Approval Request ({total} pending)",
        f" id: {focus_id}",
        f" target: {target}",
        f" reason: {reason}",
        " Confirm this action?",
        f" {allow_label}",
        f" {deny_label}",
        " Tab/Up/Down switch · Enter confirm · Esc reject · /approve <id> y|n",
    ]
    return "\n".join(lines)


def approval_toolbar_fragments(state: AppState, divider_line: str) -> Any:
    text = approval_toolbar_text(state, divider_line)
    if not text:
        return ""
    return [("bg:default fg:default noreverse", text)]


def is_expired_approval_error(err: OpenJaxResponseError) -> bool:
    if err.code == "APPROVAL_NOT_FOUND":
        return True
    message = err.message.lower()
    return "timed out" in message or "timeout" in message


async def resolve_latest_approval(
    client: Any,
    state: AppState,
    approved: bool,
    *,
    focused_approval_id_fn: Any,
    resolve_approval_by_id_fn: Any,
) -> None:
    focus_id = focused_approval_id_fn(state)
    if focus_id:
        await resolve_approval_by_id_fn(client, state, focus_id, approved)
        return

    while state.approval_order:
        approval_request_id = state.approval_order[-1]
        if approval_request_id in state.pending_approvals:
            await resolve_approval_by_id_fn(client, state, approval_request_id, approved)
            return
        state.approval_order.pop()
    print("[approval] no pending approvals")


async def resolve_approval_by_id(
    client: Any,
    state: AppState,
    approval_request_id: str,
    approved: bool,
    *,
    use_inline_approval_panel_fn: Any,
    pop_pending_fn: Any,
    is_expired_approval_error_fn: Any,
    log_approval_event_fn: Any,
) -> None:
    record = state.pending_approvals.get(approval_request_id)
    if not record or record.status != "pending":
        print(f"[approval] request not found: {approval_request_id}")
        return
    try:
        ok = await client.resolve_approval(
            turn_id=record.turn_id,
            request_id=approval_request_id,
            approved=approved,
            reason="approved_by_tui" if approved else "rejected_by_tui",
        )
        log_approval_event_fn(
            action="resolved_submit",
            request_id=approval_request_id,
            turn_id=record.turn_id,
            target=record.target,
            approved=approved,
            resolved=ok,
            detail="client_submit",
        )
        if not use_inline_approval_panel_fn(state):
            print(f"[approval] resolved: {approval_request_id} ok={ok}")
        pop_pending_fn(state, approval_request_id)
    except OpenJaxResponseError as err:
        if is_expired_approval_error_fn(err):
            log_approval_event_fn(
                action="resolve_expired",
                request_id=approval_request_id,
                turn_id=record.turn_id,
                target=record.target,
                approved=approved,
                resolved=False,
                detail=err.code,
            )
            print(f"[approval] auto-denied (expired): {approval_request_id}")
            pop_pending_fn(state, approval_request_id)
            return
        log_approval_event_fn(
            action="resolve_failed",
            request_id=approval_request_id,
            turn_id=record.turn_id,
            target=record.target,
            approved=approved,
            resolved=False,
            detail=err.code,
        )
        print(f"[approval] resolve failed: {err.code} {err.message}")
