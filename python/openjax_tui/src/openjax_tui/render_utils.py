from __future__ import annotations

import re


def align_multiline(text: str, continuation_prefix: str) -> str:
    if not text:
        return ""
    return text.replace("\n", f"\n{continuation_prefix}")


def extract_updated_target(output: str) -> str | None:
    match = re.search(r"\bUPDATE\s+([^\s:]+)", output)
    if not match:
        return None
    return match.group(1).strip()


def tool_result_label(tool_name: str, output: str) -> str:
    name = tool_name.strip().lower()
    if name == "read_file":
        return "Read 1 file"
    if name in {"apply_patch", "edit_file_range", "write_file"}:
        target = extract_updated_target(output)
        if target:
            return f"Update({target})"
        return "Update file"
    if name == "list_dir":
        return "Read directory"
    if name == "grep_files":
        return "Search files"
    if name == "shell":
        return "Run shell command"
    if not name:
        return "Tool call"
    return name.replace("_", " ").title()
