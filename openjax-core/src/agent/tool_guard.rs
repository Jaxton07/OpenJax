#[derive(Debug, Clone, Copy)]
enum ApplyPatchReadGuardReason {
    AfterSuccess,
    AfterContextMismatchFailure,
}

impl ApplyPatchReadGuardReason {
    fn log_reason(self) -> &'static str {
        match self {
            Self::AfterSuccess => "after_success",
            Self::AfterContextMismatchFailure => "after_context_mismatch_failure",
        }
    }

    fn user_message(self) -> &'static str {
        match self {
            Self::AfterSuccess => {
                "apply_patch 已成功执行。再次 apply_patch 前请先调用 read_file 获取最新内容；若是单文件连续行修正，请优先使用 edit_file_range。"
            }
            Self::AfterContextMismatchFailure => {
                "上一次 apply_patch 报 hunk context not found。请先 read_file 刷新上下文；若是单文件连续行修正，请改用 edit_file_range。"
            }
        }
    }
}

#[derive(Debug, Default, Clone)]
pub(crate) struct ApplyPatchReadGuard {
    reason: Option<ApplyPatchReadGuardReason>,
}

impl ApplyPatchReadGuard {
    pub(crate) fn block_user_message_for_tool(&self, tool_name: &str) -> Option<&'static str> {
        if tool_name != "apply_patch" {
            return None;
        }
        self.reason.map(ApplyPatchReadGuardReason::user_message)
    }

    pub(crate) fn block_log_reason_for_tool(&self, tool_name: &str) -> Option<&'static str> {
        if tool_name != "apply_patch" {
            return None;
        }
        self.reason.map(ApplyPatchReadGuardReason::log_reason)
    }

    pub(crate) fn on_tool_success(&mut self, tool_name: &str) {
        match tool_name {
            "read_file" => self.reason = None,
            "apply_patch" => self.reason = Some(ApplyPatchReadGuardReason::AfterSuccess),
            _ => {}
        }
    }

    pub(crate) fn on_tool_failure(&mut self, tool_name: &str, err_text: &str) {
        if tool_name == "read_file" {
            self.reason = None;
            return;
        }

        if tool_name == "apply_patch"
            && err_text
                .to_ascii_lowercase()
                .contains("hunk context not found")
        {
            self.reason = Some(ApplyPatchReadGuardReason::AfterContextMismatchFailure);
        }
    }
}
