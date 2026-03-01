use crate::bottom_pane::command_popup::CommandPopupState;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComposerState {
    pub buffer: String,
    pub cursor: usize,
    pub history: Vec<String>,
    pub history_index: Option<usize>,
    pub draft: String,
    pub slash_popup: CommandPopupState,
    pub input_enabled: bool,
}

impl Default for ComposerState {
    fn default() -> Self {
        Self {
            buffer: String::new(),
            cursor: 0,
            history: Vec::new(),
            history_index: None,
            draft: String::new(),
            slash_popup: CommandPopupState::default(),
            input_enabled: true,
        }
    }
}

impl ComposerState {
    pub fn insert_text(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        let mut chars: Vec<char> = self.buffer.chars().collect();
        let cursor = self.cursor.min(chars.len());
        let pasted: Vec<char> = text.chars().collect();
        chars.splice(cursor..cursor, pasted.iter().copied());
        self.buffer = chars.into_iter().collect();
        self.cursor = cursor + pasted.len();
        self.history_index = None;
        self.draft.clear();
    }

    pub fn insert_char(&mut self, ch: char) {
        let mut chars: Vec<char> = self.buffer.chars().collect();
        let cursor = self.cursor.min(chars.len());
        chars.insert(cursor, ch);
        self.buffer = chars.into_iter().collect();
        self.cursor = cursor + 1;
        self.history_index = None;
        self.draft.clear();
    }

    pub fn backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let mut chars: Vec<char> = self.buffer.chars().collect();
        let cursor = self.cursor.min(chars.len());
        chars.remove(cursor - 1);
        self.buffer = chars.into_iter().collect();
        self.cursor = cursor - 1;
        self.history_index = None;
        self.draft.clear();
    }

    pub fn move_left(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    pub fn move_right(&mut self) {
        let len = self.buffer.chars().count();
        self.cursor = (self.cursor + 1).min(len);
    }

    pub fn recall_prev(&mut self) {
        if self.history.is_empty() {
            return;
        }
        let idx = match self.history_index {
            Some(current) => current.saturating_sub(1),
            None => {
                self.draft = self.buffer.clone();
                self.history.len() - 1
            }
        };
        self.history_index = Some(idx);
        self.buffer = self.history[idx].clone();
        self.cursor = self.buffer.chars().count();
    }

    pub fn recall_next(&mut self) {
        let Some(current) = self.history_index else {
            return;
        };
        if current + 1 < self.history.len() {
            let idx = current + 1;
            self.history_index = Some(idx);
            self.buffer = self.history[idx].clone();
            self.cursor = self.buffer.chars().count();
            return;
        }
        self.history_index = None;
        self.buffer = self.draft.clone();
        self.cursor = self.buffer.chars().count();
        self.draft.clear();
    }

    pub fn consume_submitted(&mut self) -> Option<String> {
        let submitted = self.buffer.trim().to_string();
        if submitted.is_empty() {
            return None;
        }
        if self.history.last() != Some(&submitted) {
            self.history.push(submitted.clone());
        }
        self.buffer.clear();
        self.cursor = 0;
        self.history_index = None;
        self.draft.clear();
        Some(submitted)
    }

    pub fn slash_query(&self) -> Option<String> {
        if !self.buffer.starts_with('/') {
            return None;
        }
        Some(self.buffer.trim_start_matches('/').to_string())
    }
}
