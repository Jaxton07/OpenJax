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

pub(crate) fn is_approval_blocking_error(err_text: &str) -> bool {
    let lower = err_text.to_ascii_lowercase();
    lower.contains("approval rejected") || lower.contains("approval timed out")
}

pub(crate) fn approval_rejected_stop_message() -> String {
    "操作已取消：用户拒绝了工具调用，本回合已停止。".to_string()
}

pub(crate) fn approval_timed_out_stop_message() -> String {
    "审批超时：本次工具调用未在限定时间内确认，本回合已停止。请继续给出下一步指令。".to_string()
}
