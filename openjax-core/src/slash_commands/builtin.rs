/// builtin handler：返回 (展示消息, 是否替换输入框内容)
pub fn builtin_help_handler() -> (String, bool) {
    let cmds = super::registry::SlashCommandRegistry::all_commands();
    let text = cmds
        .iter()
        .map(|c| format!("/{:<8} {}", c.name, c.description))
        .collect::<Vec<_>>()
        .join("\n");
    (text, false)
}

pub fn builtin_clear_handler() -> (String, bool) {
    ("clearing context...".to_string(), false)
}

pub fn builtin_explain_template() -> (String, bool) {
    ("Explain the relevant code path and key tradeoffs.".to_string(), true)
}

pub fn builtin_review_template() -> (String, bool) {
    ("Review the current changes, prioritize findings, and keep the summary brief.".to_string(), true)
}
