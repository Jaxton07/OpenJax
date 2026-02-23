from __future__ import annotations

from typing import Any


def slash_command_candidates(
    text_before_cursor: str,
    slash_commands: tuple[str, ...],
) -> list[str]:
    if not text_before_cursor.startswith("/"):
        return []
    token = text_before_cursor.split(maxsplit=1)[0]
    if not token:
        return list(slash_commands)
    return [cmd for cmd in slash_commands if cmd.startswith(token)]


def slash_hint_text(input_text: str, slash_commands: tuple[str, ...]) -> str:
    candidates = slash_command_candidates(input_text, slash_commands)
    if not candidates:
        return ""
    joined = "  ".join(candidates)
    return f" commands: {joined}"


def slash_hint_fragments(input_text: str, slash_commands: tuple[str, ...]) -> Any:
    text = slash_hint_text(input_text, slash_commands)
    if not text:
        return ""
    return [("bg:default fg:default noreverse", text)]


def build_slash_command_completer(
    slash_commands: tuple[str, ...],
    prompt_toolkit_completer: Any,
    prompt_toolkit_completion: Any,
) -> Any:
    if prompt_toolkit_completer is None or prompt_toolkit_completion is None:
        return None

    completion_cls = prompt_toolkit_completion

    class _SlashCompleter(prompt_toolkit_completer):
        def get_completions(self, document: Any, complete_event: Any) -> Any:
            _ = complete_event
            text_before_cursor = str(getattr(document, "text_before_cursor", ""))
            token = text_before_cursor.split(maxsplit=1)[0]
            for command in slash_command_candidates(text_before_cursor, slash_commands):
                yield completion_cls(
                    command,
                    start_position=-len(token),
                )

    return _SlashCompleter()
