use super::slash_commands::{SlashCommandEntry, score_match};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CommandPopupState {
    pub open: bool,
    pub query: String,
    pub selected: usize,
    pub filtered: Vec<SlashCommandEntry>,
}

impl CommandPopupState {
    pub fn refresh(&mut self, query: &str, all: &[SlashCommandEntry]) {
        self.query = query.to_string();
        let mut scored: Vec<(usize, SlashCommandEntry)> = all
            .iter()
            .filter_map(|it| score_match(query, it).map(|score| (score, it.clone())))
            .collect();
        scored.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.name.cmp(b.1.name)));
        self.filtered = scored.into_iter().map(|(_, item)| item).collect();
        self.selected = 0;
        self.open = true;
    }

    pub fn close(&mut self) {
        self.open = false;
        self.query.clear();
        self.selected = 0;
        self.filtered.clear();
    }

    pub fn move_selection(&mut self, direction: i32) {
        if self.filtered.is_empty() {
            return;
        }
        let max = self.filtered.len() as i32 - 1;
        self.selected = (self.selected as i32 + direction).clamp(0, max) as usize;
    }

    pub fn selected_command(&self) -> Option<SlashCommandEntry> {
        self.filtered.get(self.selected).cloned()
    }
}
