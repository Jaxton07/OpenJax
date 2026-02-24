"""调试工具函数模块。

提供事件格式化、文本截断和 ANSI 处理等调试相关功能。
"""

from __future__ import annotations

import re
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from openjax_sdk.models import EventEnvelope


def format_event_debug_line(evt: EventEnvelope) -> str:
    """格式化事件为调试日志行。

    Args:
        evt: 事件信封

    Returns:
        格式化后的调试日志行
    """
    base = f"event received type={evt.event_type} turn_id={evt.turn_id or '-'}"

    if evt.event_type == "assistant_delta":
        delta = str(evt.payload.get("content_delta", ""))
        preview, truncated = truncate_debug_preview(delta, limit=80)
        return (
            f"{base} payload_keys={sorted(evt.payload.keys())} "
            f"delta_len={len(delta)} delta_preview={preview!r} delta_truncated={truncated}"
        )

    if evt.event_type == "assistant_message":
        content = str(evt.payload.get("content", ""))
        preview, truncated = truncate_debug_preview(content, limit=120)
        return (
            f"{base} payload_keys={sorted(evt.payload.keys())} "
            f"content_len={len(content)} content_preview={preview!r} content_truncated={truncated}"
        )

    return f"{base} payload_keys={sorted(evt.payload.keys())}"


def truncate_debug_preview(text: str, limit: int) -> tuple[str, bool]:
    """截断文本用于调试预览。

    Args:
        text: 原始文本
        limit: 最大长度限制

    Returns:
        (截断后的文本, 是否被截断)
    """
    normalized = text.replace("\n", "\\n").replace("\r", "\\r")
    if len(normalized) <= limit:
        return normalized, False
    return normalized[:limit] + "...", True


def normalize_history_for_prompt_toolkit(
    text: str,
    *,
    ansi_green: str = "\x1b[32m",
    ansi_red: str = "\x1b[31m",
    ansi_reset: str = "\x1b[0m",
    assistant_prefix: str = "⏺",
) -> str:
    """标准化历史文本用于 prompt_toolkit 显示。

    TextArea 渲染纯文本；ANSI 转义序列会作为原始序列泄露，需要清理。

    Args:
        text: 原始文本
        ansi_green: 绿色 ANSI 代码
        ansi_red: 红色 ANSI 代码
        ansi_reset: 重置 ANSI 代码
        assistant_prefix: 助手前缀字符

    Returns:
        清理后的文本
    """
    # 将带颜色的前缀替换为无色前缀
    normalized = text.replace(f"{ansi_green}{assistant_prefix}{ansi_reset}", assistant_prefix)
    normalized = normalized.replace(f"{ansi_red}{assistant_prefix}{ansi_reset}", assistant_prefix)
    # 移除所有 ANSI 转义序列
    return re.sub(r"\x1b\[[0-9;]*m", "", normalized)
