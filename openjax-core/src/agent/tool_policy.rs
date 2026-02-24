use std::collections::HashMap;

pub(crate) fn duplicate_tool_call_warning(
    tool_name: &str,
    args: &HashMap<String, String>,
) -> String {
    format!(
        "[warning] tool {} with args {:?} was already called recently, skipping",
        tool_name, args
    )
}

pub(crate) fn should_abort_on_consecutive_duplicate_skips(count: usize, max_skips: usize) -> bool {
    count >= max_skips
}

pub(crate) fn duplicate_skip_abort_message(max_skips: usize) -> String {
    format!(
        "检测到连续 {} 次重复工具调用，已提前结束本回合以避免循环。请继续下一轮或换一种指令。",
        max_skips
    )
}

pub(crate) fn is_approval_rejected_error(err_text: &str) -> bool {
    err_text.to_ascii_lowercase().contains("approval rejected")
}

pub(crate) fn approval_rejected_stop_message() -> String {
    "操作已取消：用户拒绝了工具调用，本回合已停止。".to_string()
}
