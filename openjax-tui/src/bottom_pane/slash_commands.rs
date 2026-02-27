#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlashCommandEntry {
    pub name: &'static str,
    pub description: &'static str,
    pub enabled: bool,
}

pub fn default_commands() -> Vec<SlashCommandEntry> {
    vec![
        SlashCommandEntry {
            name: "help",
            description: "Show help",
            enabled: true,
        },
        SlashCommandEntry {
            name: "clear",
            description: "Clear history",
            enabled: true,
        },
        SlashCommandEntry {
            name: "exit",
            description: "Exit app",
            enabled: true,
        },
        SlashCommandEntry {
            name: "pending",
            description: "List pending approvals",
            enabled: true,
        },
        SlashCommandEntry {
            name: "approve",
            description: "Approve focused request",
            enabled: true,
        },
        SlashCommandEntry {
            name: "deny",
            description: "Deny focused request",
            enabled: true,
        },
        SlashCommandEntry {
            name: "model",
            description: "Model picker (coming soon)",
            enabled: false,
        },
        SlashCommandEntry {
            name: "permissions",
            description: "Permissions (coming soon)",
            enabled: false,
        },
        SlashCommandEntry {
            name: "review",
            description: "Review mode (coming soon)",
            enabled: false,
        },
        SlashCommandEntry {
            name: "skills",
            description: "Skills panel (coming soon)",
            enabled: false,
        },
    ]
}

pub fn score_match(query: &str, item: &SlashCommandEntry) -> Option<usize> {
    let q = query.trim().to_ascii_lowercase();
    if q.is_empty() {
        return Some(1000);
    }
    let name = item.name.to_ascii_lowercase();
    let desc = item.description.to_ascii_lowercase();

    if name == q {
        return Some(0);
    }
    if name.starts_with(&q) {
        return Some(100 + (name.len() - q.len()));
    }
    if let Some(idx) = name.find(&q) {
        return Some(300 + idx);
    }
    if let Some(idx) = desc.find(&q) {
        return Some(500 + idx);
    }

    let mut pos = 0usize;
    let mut gap_penalty = 0usize;
    for ch in q.chars() {
        let Some(found) = name[pos..].find(ch) else {
            return None;
        };
        gap_penalty += found;
        pos += found + 1;
    }
    Some(700 + gap_penalty)
}
