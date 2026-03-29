use std::collections::HashMap;

use super::shell::ShellType;
use super::spec::ToolsConfig;

#[derive(Debug, Clone)]
pub struct ToolCall {
    pub name: String,
    pub args: HashMap<String, String>,
}

pub fn parse_tool_call(input: &str) -> Option<ToolCall> {
    let input = input.trim();

    if !input.starts_with("tool:") {
        return None;
    }

    let rest = &input[5..].trim();
    if rest.is_empty() {
        return None;
    }

    let (name, mut i) = parse_name(rest)?;
    let mut args = HashMap::new();

    while i < rest.len() {
        skip_spaces(rest, &mut i);
        if i >= rest.len() {
            break;
        }

        let key_start = i;
        while i < rest.len() {
            let ch = rest[i..].chars().next().expect("char exists");
            if ch == '=' || ch.is_whitespace() {
                break;
            }
            i += ch.len_utf8();
        }

        if key_start == i {
            return None;
        }
        let key = &rest[key_start..i];

        skip_spaces(rest, &mut i);
        if i >= rest.len() || !rest[i..].starts_with('=') {
            while i < rest.len()
                && !rest[i..]
                    .chars()
                    .next()
                    .expect("char exists")
                    .is_whitespace()
            {
                i += rest[i..].chars().next().expect("char exists").len_utf8();
            }
            continue;
        }
        i += 1; // skip '='
        skip_spaces(rest, &mut i);
        if i >= rest.len() {
            args.insert(key.to_string(), String::new());
            break;
        }

        let first = rest[i..].chars().next().expect("char exists");
        let value = if first == '\'' || first == '"' {
            parse_quoted_value(rest, &mut i, first)?
        } else {
            parse_unquoted_value(rest, &mut i)
        };
        args.insert(key.to_string(), value);
    }

    Some(ToolCall { name, args })
}

fn parse_name(rest: &str) -> Option<(String, usize)> {
    let mut i = 0;
    skip_spaces(rest, &mut i);
    let start = i;
    while i < rest.len() {
        let ch = rest[i..].chars().next()?;
        if ch.is_whitespace() {
            break;
        }
        i += ch.len_utf8();
    }
    if start == i {
        return None;
    }
    Some((rest[start..i].to_string(), i))
}

fn skip_spaces(s: &str, i: &mut usize) {
    while *i < s.len() {
        let ch = s[*i..].chars().next().expect("char exists");
        if !ch.is_whitespace() {
            break;
        }
        *i += ch.len_utf8();
    }
}

fn parse_unquoted_value(s: &str, i: &mut usize) -> String {
    let start = *i;
    while *i < s.len() {
        let ch = s[*i..].chars().next().expect("char exists");
        if ch.is_whitespace() {
            break;
        }
        *i += ch.len_utf8();
    }
    s[start..*i].to_string()
}

fn parse_quoted_value(s: &str, i: &mut usize, quote: char) -> Option<String> {
    *i += quote.len_utf8();
    let mut out = String::new();

    while *i < s.len() {
        let ch = s[*i..].chars().next()?;
        if ch == '\\' {
            let next_index = *i + ch.len_utf8();
            if next_index >= s.len() {
                out.push('\\');
                *i = next_index;
                continue;
            }
            let next = s[next_index..].chars().next()?;
            if next == quote || next == '\\' {
                out.push(next);
                *i = next_index + next.len_utf8();
            } else {
                out.push('\\');
                *i = next_index;
            }
            continue;
        }

        *i += ch.len_utf8();

        if ch == quote {
            return Some(out);
        }

        out.push(ch);
    }

    None
}

#[derive(Debug, Clone, Copy)]
pub struct ToolRuntimeConfig {
    pub sandbox_mode: SandboxMode,
    pub shell_type: ShellType,
    pub tools_config: ToolsConfig,
    pub prevent_shell_skill_trigger: bool,
}

impl Default for ToolRuntimeConfig {
    fn default() -> Self {
        Self {
            sandbox_mode: SandboxMode::WorkspaceWrite,
            shell_type: ShellType::default(),
            tools_config: ToolsConfig::default(),
            prevent_shell_skill_trigger: true,
        }
    }
}

impl ToolRuntimeConfig {
    pub fn with_config(config: ToolsConfig) -> Self {
        Self {
            sandbox_mode: SandboxMode::WorkspaceWrite,
            shell_type: ShellType::default(),
            tools_config: config,
            prevent_shell_skill_trigger: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SandboxMode {
    WorkspaceWrite,
    DangerFullAccess,
}

impl SandboxMode {
    pub fn from_env() -> Self {
        match std::env::var("OPENJAX_SANDBOX_MODE").as_deref() {
            Ok("workspace_write") => Self::WorkspaceWrite,
            Ok("danger_full_access") => Self::DangerFullAccess,
            _ => Self::WorkspaceWrite,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::WorkspaceWrite => "workspace_write",
            Self::DangerFullAccess => "danger_full_access",
        }
    }
}

pub const MAX_AGENT_DEPTH: i32 = 10;

#[cfg(test)]
mod tests {
    use super::parse_tool_call;

    #[test]
    fn parse_tool_call_preserves_quoted_shell_command() {
        let input = "tool:shell cmd='echo hi >/tmp/openjax-e2e.txt' require_escalated=true";
        let parsed = parse_tool_call(input).expect("expected parsed tool call");
        assert_eq!(parsed.name, "shell");
        assert_eq!(
            parsed.args.get("cmd").map(String::as_str),
            Some("echo hi >/tmp/openjax-e2e.txt")
        );
        assert_eq!(
            parsed.args.get("require_escalated").map(String::as_str),
            Some("true")
        );
    }

}
