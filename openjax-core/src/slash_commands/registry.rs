use std::sync::Arc;
use std::sync::LazyLock;
use std::sync::RwLock;

use super::builtin::{builtin_explain_template, builtin_help_handler, builtin_review_template};
use super::kinds::SlashCommandKind;

static DYNAMIC_SKILL_COMMANDS: LazyLock<RwLock<Vec<SlashCommand>>> =
    LazyLock::new(|| RwLock::new(Vec::new()));

/// 注册的斜杠命令
#[derive(Clone)]
pub struct SlashCommand {
    pub name: &'static str,
    /// 别名列表，find() 和 match_prefix() 均会检查别名
    pub aliases: &'static [&'static str],
    pub description: &'static str,
    /// 命令的使用提示，不含 <args>（除非命令本身需要参数）
    pub usage_hint: &'static str,
    pub kind: SlashCommandKind,
}

/// 斜杠命令匹配结果
#[derive(Clone)]
pub struct SlashMatch {
    pub command_name: &'static str,
    pub description: &'static str,
    pub usage_hint: &'static str,
    pub replacement: String,
    pub kind: SlashCommandKind,
}

impl SlashMatch {
    /// 执行 builtin 命令，返回 Some((展示消息, 是否替换输入框))；非 builtin 返回 None
    pub fn execute_builtin(&self) -> Option<(String, bool)> {
        match &self.kind {
            SlashCommandKind::Builtin {
                handler,
                replaces_input,
            } => Some((handler().0, *replaces_input)),
            _ => None,
        }
    }
}

/// 斜杠命令注册表
pub struct SlashCommandRegistry;

impl SlashCommandRegistry {
    fn builtin_commands() -> Vec<SlashCommand> {
        vec![
            SlashCommand {
                name: "help",
                aliases: &["?"],
                description: "Show available commands",
                usage_hint: "/help",
                kind: SlashCommandKind::Builtin {
                    handler: Arc::new(builtin_help_handler),
                    replaces_input: false,
                },
            },
            SlashCommand {
                name: "clear",
                aliases: &["cls"],
                description: "Clear current session context",
                usage_hint: "/clear",
                // clear 需要 gateway 执行 clear_runtime，不是本地 builtin
                kind: SlashCommandKind::SessionAction { action: "clear" },
            },
            SlashCommand {
                name: "explain",
                aliases: &[],
                description: "Insert explain prompt template into input",
                usage_hint: "/explain",
                kind: SlashCommandKind::Builtin {
                    handler: Arc::new(builtin_explain_template),
                    replaces_input: true,
                },
            },
            SlashCommand {
                name: "review",
                aliases: &[],
                description: "Insert code review prompt template into input",
                usage_hint: "/review",
                kind: SlashCommandKind::Builtin {
                    handler: Arc::new(builtin_review_template),
                    replaces_input: true,
                },
            },
            SlashCommand {
                name: "compact",
                aliases: &[],
                description: "Compact conversation history",
                usage_hint: "/compact",
                kind: SlashCommandKind::SessionAction { action: "compact" },
            },
        ]
    }

    /// 获取所有命令（内置 + 动态 skill）
    pub fn all_commands() -> Vec<SlashCommand> {
        let mut commands = Self::builtin_commands();
        commands.extend(DYNAMIC_SKILL_COMMANDS.read().unwrap().clone());
        commands
    }

    /// 精确查找（名称或别名均可匹配）
    pub fn find(name: &str) -> Option<SlashMatch> {
        let normalized = name.trim().strip_prefix('/').unwrap_or(name);
        Self::all_commands()
            .into_iter()
            .find(|c| c.name == normalized || c.aliases.contains(&normalized))
            .map(|c| SlashMatch {
                command_name: c.name,
                description: c.description,
                usage_hint: c.usage_hint,
                replacement: format!("/{} ", c.name),
                kind: c.kind,
            })
    }

    /// 前缀匹配（包含名称和别名前缀；所有 Kind 均可匹配）
    pub fn match_prefix(query: &str, limit: usize) -> Vec<SlashMatch> {
        let normalized = query.trim().strip_prefix('/').unwrap_or(query);
        let mut matches: Vec<SlashMatch> = Self::all_commands()
            .into_iter()
            .filter(|c| {
                c.name.starts_with(normalized)
                    || c.aliases.iter().any(|a| a.starts_with(normalized))
            })
            .map(|c| SlashMatch {
                command_name: c.name,
                description: c.description,
                usage_hint: c.usage_hint,
                replacement: format!("/{} ", c.name),
                kind: c.kind,
            })
            .collect();

        // 精确匹配优先，其次字母序
        matches.sort_by(|a, b| {
            let a_exact = a.command_name == normalized;
            let b_exact = b.command_name == normalized;
            match (a_exact, b_exact) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.command_name.cmp(b.command_name),
            }
        });

        matches.truncate(limit);
        matches
    }

    /// 新的 skill 发现前调用，清空动态命令列表（防止重复调用时积累）
    pub fn clear_dynamic_commands() {
        DYNAMIC_SKILL_COMMANDS.write().unwrap().clear();
    }
}

/// 注册一个 skill 命令（由 loader 在发现 skill 时调用）
pub fn register_skill_command(skill_name: &'static str, description: &'static str) {
    let mut skills = DYNAMIC_SKILL_COMMANDS.write().unwrap();
    if !skills.iter().any(|c| c.name == skill_name) {
        skills.push(SlashCommand {
            name: skill_name,
            aliases: &[],
            description,
            usage_hint: Box::leak(format!("/{}", skill_name).into_boxed_str()),
            kind: SlashCommandKind::Skill { skill_name },
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_exact_clear() {
        let m = SlashCommandRegistry::find("clear").unwrap();
        assert_eq!(m.command_name, "clear");
        // clear 是 SessionAction，不是 Builtin
        assert!(matches!(
            m.kind,
            SlashCommandKind::SessionAction { action: "clear" }
        ));
        assert!(m.execute_builtin().is_none());
    }

    #[test]
    fn test_find_alias_cls_resolves_to_clear() {
        let m = SlashCommandRegistry::find("cls").unwrap();
        // 别名 cls 解析到主命令 clear
        assert_eq!(m.command_name, "clear");
    }

    #[test]
    fn test_find_alias_question_resolves_to_help() {
        let m = SlashCommandRegistry::find("?").unwrap();
        assert_eq!(m.command_name, "help");
    }

    #[test]
    fn test_no_duplicate_cls_or_question_in_all_commands() {
        let all = SlashCommandRegistry::all_commands();
        // cls 和 ? 不应作为独立命令出现
        assert!(!all.iter().any(|c| c.name == "cls"));
        assert!(!all.iter().any(|c| c.name == "?"));
        // help 和 clear 存在
        assert!(all.iter().any(|c| c.name == "help"));
        assert!(all.iter().any(|c| c.name == "clear"));
    }

    #[test]
    fn test_match_prefix_cl_matches_clear() {
        let matches = SlashCommandRegistry::match_prefix("/cl", 10);
        assert!(matches.iter().any(|m| m.command_name == "clear"));
        // cls 不应单独出现
        assert!(!matches.iter().any(|m| m.command_name == "cls"));
    }

    #[test]
    fn test_match_prefix_question_matches_help_via_alias() {
        let matches = SlashCommandRegistry::match_prefix("/?", 10);
        assert!(matches.iter().any(|m| m.command_name == "help"));
    }

    #[test]
    fn test_explain_replaces_input_true() {
        let m = SlashCommandRegistry::find("explain").unwrap();
        let (_, replaces) = m.execute_builtin().unwrap();
        assert!(replaces);
    }

    #[test]
    fn test_help_replaces_input_false() {
        let m = SlashCommandRegistry::find("help").unwrap();
        let (_, replaces) = m.execute_builtin().unwrap();
        assert!(!replaces);
    }

    #[test]
    fn test_compact_is_session_action() {
        let m = SlashCommandRegistry::find("compact").unwrap();
        assert_eq!(m.kind.session_action_name(), Some("compact"));
    }

    #[test]
    fn test_usage_hint_no_args_suffix() {
        let all = SlashCommandRegistry::all_commands();
        for cmd in &all {
            assert!(
                !cmd.usage_hint.contains("<args>"),
                "Command '{}' usage_hint should not contain '<args>': {}",
                cmd.name,
                cmd.usage_hint
            );
        }
    }
}
