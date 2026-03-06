use crate::state::{SlashCommandKind, SlashLocalAction, SlashMatch};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SlashCommand {
    pub name: &'static str,
    pub aliases: &'static [&'static str],
    pub description: &'static str,
    pub usage_hint: &'static str,
    pub replacement: &'static str,
    pub kind: SlashCommandKind,
}

const SLASH_COMMANDS: &[SlashCommand] = &[
    SlashCommand {
        name: "clear",
        aliases: &["cls"],
        description: "Clear history and reset the TUI session view",
        usage_hint: "/clear",
        replacement: "/clear",
        kind: SlashCommandKind::LocalAction(SlashLocalAction::Clear),
    },
    SlashCommand {
        name: "help",
        aliases: &["?"],
        description: "Show available slash commands in the status area",
        usage_hint: "/help",
        replacement: "/help",
        kind: SlashCommandKind::LocalAction(SlashLocalAction::Help),
    },
    SlashCommand {
        name: "explain",
        aliases: &[],
        description: "Insert an explain prompt template into the input box",
        usage_hint: "/explain",
        replacement: "Explain the relevant code path and key tradeoffs.",
        kind: SlashCommandKind::PromptTemplate,
    },
    SlashCommand {
        name: "review",
        aliases: &[],
        description: "Insert a code review prompt template into the input box",
        usage_hint: "/review",
        replacement: "Review the current changes, prioritize findings, and keep the summary brief.",
        kind: SlashCommandKind::PromptTemplate,
    },
];

pub fn all_commands() -> &'static [SlashCommand] {
    SLASH_COMMANDS
}

pub fn find_exact(query: &str) -> Option<SlashMatch> {
    let normalized = normalize_query(query)?;
    SLASH_COMMANDS
        .iter()
        .find(|command| command.name == normalized || command.aliases.contains(&normalized))
        .map(to_match)
}

pub fn match_commands(query: &str, limit: usize) -> Vec<SlashMatch> {
    let normalized = normalize_query(query).unwrap_or_default();
    let mut commands: Vec<_> = SLASH_COMMANDS.iter().collect();
    commands.sort_by_key(|command| sort_key(command, normalized));
    commands
        .into_iter()
        .filter(|command| matches_query(command, normalized))
        .take(limit)
        .map(to_match)
        .collect()
}

fn normalize_query(query: &str) -> Option<&str> {
    let trimmed = query.trim();
    trimmed.strip_prefix('/')
}

fn matches_query(command: &SlashCommand, query: &str) -> bool {
    if query.is_empty() {
        return true;
    }
    command.name.starts_with(query) || command.aliases.iter().any(|alias| alias.starts_with(query))
}

fn sort_key(command: &SlashCommand, query: &str) -> (u8, u8, usize) {
    let exact_name = (!query.is_empty() && command.name == query) as u8;
    let prefix_name = command.name.starts_with(query) as u8;
    let prefix_alias = command.aliases.iter().any(|alias| alias.starts_with(query)) as u8;
    let registration_index = SLASH_COMMANDS
        .iter()
        .position(|candidate| candidate.name == command.name)
        .unwrap_or(usize::MAX);
    (
        1u8.saturating_sub(exact_name),
        if prefix_name == 1 {
            0
        } else if prefix_alias == 1 {
            1
        } else {
            2
        },
        registration_index,
    )
}

fn to_match(command: &SlashCommand) -> SlashMatch {
    SlashMatch {
        command_name: command.name,
        description: command.description,
        usage_hint: command.usage_hint,
        replacement: command.replacement.to_string(),
        kind: command.kind,
    }
}

#[cfg(test)]
mod tests {
    use super::{find_exact, match_commands};

    #[test]
    fn matches_prefix_and_aliases() {
        let names: Vec<_> = match_commands("/cl", 5)
            .into_iter()
            .map(|item| item.command_name)
            .collect();
        assert_eq!(names, vec!["clear"]);

        let names: Vec<_> = match_commands("/?", 5)
            .into_iter()
            .map(|item| item.command_name)
            .collect();
        assert_eq!(names, vec!["help"]);
    }

    #[test]
    fn exact_match_resolves_alias() {
        let matched = find_exact("/cls").expect("alias should resolve");
        assert_eq!(matched.command_name, "clear");
    }

    #[test]
    fn empty_query_returns_sorted_commands() {
        let names: Vec<_> = match_commands("/", 5)
            .into_iter()
            .map(|item| item.command_name)
            .collect();
        assert_eq!(names, vec!["clear", "help", "explain", "review"]);
    }

    #[test]
    fn non_prefix_input_does_not_match() {
        let names: Vec<_> = match_commands("/rvw", 5)
            .into_iter()
            .map(|item| item.command_name)
            .collect();
        assert!(names.is_empty());
    }
}
