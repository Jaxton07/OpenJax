use std::sync::Arc;
use std::sync::RwLock;
use std::sync::LazyLock;

use super::builtin::{builtin_clear_handler, builtin_explain_template, builtin_help_handler, builtin_review_template};
use super::kinds::SlashCommandKind;

static DYNAMIC_SKILL_COMMANDS: LazyLock<RwLock<Vec<SlashCommand>>> =
    LazyLock::new(|| RwLock::new(Vec::new()));

/// 注册的斜杠命令
#[derive(Clone)]
pub struct SlashCommand {
    pub name: &'static str,
    pub description: &'static str,
    pub kind: SlashCommandKind,
}

impl SlashCommand {
    pub fn new(name: &'static str, description: &'static str, kind: SlashCommandKind) -> Self {
        Self { name, description, kind }
    }

    pub fn new_skill(skill_name: &'static str, description: &'static str) -> Self {
        Self {
            name: skill_name,
            description,
            kind: SlashCommandKind::Skill { skill_name },
        }
    }
}

/// 斜杠命令匹配结果
#[derive(Clone)]
pub struct SlashMatch {
    pub command_name: &'static str,
    pub description: &'static str,
    pub usage_hint: String,
    pub replacement: String,
    pub kind: SlashCommandKind,
}

impl SlashMatch {
    /// 执行 builtin 命令，返回 Some((展示消息, 是否替换输入框内容))，如果不是 builtin 则返回 None
    pub fn execute_builtin(&self) -> Option<(String, bool)> {
        match &self.kind {
            SlashCommandKind::Builtin { handler } => {
                let (msg, replaces) = handler();
                Some((msg, replaces))
            }
            _ => None,
        }
    }
}

/// 斜杠命令注册表
pub struct SlashCommandRegistry;

impl SlashCommandRegistry {
    /// 获取所有内置命令
    fn builtin_commands() -> Vec<SlashCommand> {
        vec![
            SlashCommand {
                name: "help",
                description: "Show available commands",
                kind: SlashCommandKind::Builtin {
                    handler: Arc::new(builtin_help_handler),
                },
            },
            SlashCommand {
                name: "?",
                description: "Alias for help",
                kind: SlashCommandKind::Builtin {
                    handler: Arc::new(builtin_help_handler),
                },
            },
            SlashCommand {
                name: "clear",
                description: "Clear current context",
                kind: SlashCommandKind::Builtin {
                    handler: Arc::new(builtin_clear_handler),
                },
            },
            SlashCommand {
                name: "cls",
                description: "Alias for clear",
                kind: SlashCommandKind::Builtin {
                    handler: Arc::new(builtin_clear_handler),
                },
            },
            SlashCommand {
                name: "explain",
                description: "Explain current code context",
                kind: SlashCommandKind::Builtin {
                    handler: Arc::new(builtin_explain_template),
                },
            },
            SlashCommand {
                name: "review",
                description: "Review current changes",
                kind: SlashCommandKind::Builtin {
                    handler: Arc::new(builtin_review_template),
                },
            },
            SlashCommand {
                name: "compact",
                description: "Compact conversation history",
                kind: SlashCommandKind::SessionAction {
                    action: "compact",
                },
            },
        ]
    }

    /// 获取所有命令（内置 + 动态）
    pub fn all_commands() -> Vec<SlashCommand> {
        let mut commands = Self::builtin_commands();
        commands.extend(DYNAMIC_SKILL_COMMANDS.read().unwrap().clone());
        commands
    }

    /// 查找精确匹配的命令
    pub fn find(name: &str) -> Option<SlashMatch> {
        let normalized = name.trim().strip_prefix('/').unwrap_or(name);
        Self::all_commands()
            .into_iter()
            .find(|c| c.name == normalized)
            .map(|c| {
                let usage_hint = format!("/{} <args>", c.name);
                let replacement = format!("/{} ", c.name);
                SlashMatch {
                    command_name: c.name,
                    description: c.description,
                    usage_hint,
                    replacement,
                    kind: c.kind,
                }
            })
    }

    /// 前缀匹配命令
    pub fn match_prefix(query: &str, limit: usize) -> Vec<SlashMatch> {
        let normalized = query.trim().strip_prefix('/').unwrap_or(query);
        let mut matches: Vec<SlashMatch> = Self::all_commands()
            .into_iter()
            .filter(|c| {
                // Exclude SessionActions - they are not handled via prefix matching
                if matches!(c.kind, SlashCommandKind::SessionAction { .. }) {
                    return false;
                }
                // Match by name prefix only (consistent with old TUI behavior)
                c.name.starts_with(normalized)
            })
            .map(|c| {
                let usage_hint = format!("/{} <args>", c.name);
                let replacement = format!("/{} ", c.name);
                SlashMatch {
                    command_name: c.name,
                    description: c.description,
                    usage_hint,
                    replacement,
                    kind: c.kind,
                }
            })
            .collect();

        // 排序：精确匹配优先，然后前缀匹配
        matches.sort_by(|a, b| {
            let a_exact = a.command_name == normalized;
            let b_exact = b.command_name == normalized;
            if a_exact && !b_exact {
                std::cmp::Ordering::Less
            } else if !a_exact && b_exact {
                std::cmp::Ordering::Greater
            } else {
                a.command_name.cmp(b.command_name)
            }
        });

        matches.truncate(limit);
        matches
    }
}

/// 注册一个 skill 命令（由 loader 在发现 skill 时调用）
pub fn register_skill_command(skill_name: &'static str, description: &'static str) {
    let mut skills = DYNAMIC_SKILL_COMMANDS.write().unwrap();
    // 去重
    if !skills.iter().any(|c| c.name == skill_name) {
        skills.push(SlashCommand::new_skill(skill_name, description));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_exact_clear() {
        let m = SlashCommandRegistry::find("clear").unwrap();
        assert_eq!(m.command_name, "clear");
        let (msg, replaces) = m.execute_builtin().unwrap();
        assert_eq!(msg, "clearing context...");
        assert!(!replaces);
    }

    #[test]
    fn test_alias_cls_resolves_to_clear() {
        let m = SlashCommandRegistry::find("cls").unwrap();
        assert_eq!(m.command_name, "cls");
        // cls is an alias for clear, so it executes clear's handler
        let (msg, _) = m.execute_builtin().unwrap();
        assert_eq!(msg, "clearing context...");
    }

    #[test]
    fn test_alias_question_resolves_to_help() {
        let m = SlashCommandRegistry::find("?").unwrap();
        assert_eq!(m.command_name, "?");
        let (msg, _) = m.execute_builtin().unwrap();
        assert!(msg.contains("/help"));
    }

    #[test]
    fn test_prefix_cl_matches_clear() {
        let matches = SlashCommandRegistry::match_prefix("/cl", 10);
        assert!(!matches.is_empty());
        assert!(matches.iter().any(|m| m.command_name == "clear"));
    }

    #[test]
    fn test_explain_returns_template_with_replaces_input_true() {
        let m = SlashCommandRegistry::find("explain").unwrap();
        let (_, replaces) = m.execute_builtin().unwrap();
        assert!(replaces);
    }

    #[test]
    fn test_review_returns_template_with_replaces_input_true() {
        let m = SlashCommandRegistry::find("review").unwrap();
        let (_, replaces) = m.execute_builtin().unwrap();
        assert!(replaces);
    }

    #[test]
    fn test_compact_is_session_action() {
        let m = SlashCommandRegistry::find("compact").unwrap();
        assert!(matches!(m.kind, SlashCommandKind::SessionAction { .. }));
        assert_eq!(m.kind.session_action_name(), Some("compact"));
    }

    #[test]
    fn test_all_commands_includes_compact() {
        let all = SlashCommandRegistry::all_commands();
        assert!(all.iter().any(|c| c.name == "compact"));
    }
}
