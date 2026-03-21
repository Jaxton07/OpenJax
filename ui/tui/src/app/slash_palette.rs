use crate::slash_commands::{all_commands, find_exact, match_commands};
use crate::state::{SlashCommandKind, SlashPaletteState};

use super::App;

#[derive(Debug, Eq, PartialEq)]
pub enum SlashAcceptResult {
    None,
    CompletedInput,
    ExecutedLocalAction,
}

impl App {
    pub fn refresh_slash_palette(&mut self) {
        if self.state.pending_approval.is_some() {
            self.dismiss_slash_palette();
            return;
        }

        let Some(query) = self.active_slash_query() else {
            self.dismiss_slash_palette();
            return;
        };

        let matches = match_commands(&query, 5);
        let selected_index = self
            .state
            .slash_palette
            .selected_index
            .min(matches.len().saturating_sub(1));
        self.state.slash_palette = SlashPaletteState {
            visible: true,
            query,
            matches,
            selected_index,
        };
    }

    pub fn dismiss_slash_palette(&mut self) {
        self.state.slash_palette = SlashPaletteState::default();
    }

    pub fn is_slash_palette_active(&self) -> bool {
        self.state.slash_palette.visible
    }

    pub fn move_slash_selection(&mut self, delta: i8) {
        if !self.state.slash_palette.visible || self.state.slash_palette.matches.is_empty() {
            return;
        }
        let len = self.state.slash_palette.matches.len() as i8;
        let current = self.state.slash_palette.selected_index as i8;
        let next = (current + delta).rem_euclid(len);
        self.state.slash_palette.selected_index = next as usize;
    }

    pub fn complete_slash_selection(&mut self) -> SlashAcceptResult {
        let selected = self
            .state
            .slash_palette
            .matches
            .get(self.state.slash_palette.selected_index)
            .cloned();
        let Some(selected) = selected else {
            return SlashAcceptResult::None;
        };

        self.state.input = format!("/{}", selected.command_name);
        self.state.input_cursor = self.state.input.len();

        if find_exact(&self.state.input).is_some() {
            self.dismiss_slash_palette();
        } else {
            self.refresh_slash_palette();
        }

        SlashAcceptResult::CompletedInput
    }

    pub fn submit_slash_command_if_exact(&mut self) -> bool {
        let input = self.state.input.trim();
        let Some(matched) = find_exact(input) else {
            return false;
        };

        match &matched.kind {
            SlashCommandKind::Builtin { .. } => {
                let Some((msg, replaces)) = matched.execute_builtin() else {
                    return false;
                };
                match matched.command_name {
                    "help" => {
                        // Help displays in status area without modifying input
                        self.set_live_status(msg);
                        self.state.input.clear();
                        self.state.input_cursor = 0;
                        self.dismiss_slash_palette();
                    }
                    _ => {
                        // Prompt templates (explain, review) replace input when flagged
                        if replaces {
                            self.state.input = msg;
                            self.state.input_cursor = self.state.input.len();
                        } else {
                            // Other builtins: show message without replacing input
                            self.set_live_status(msg);
                            self.state.input.clear();
                            self.state.input_cursor = 0;
                            self.dismiss_slash_palette();
                        }
                        self.refresh_slash_palette();
                    }
                }
                true
            }
            SlashCommandKind::SessionAction { .. } | SlashCommandKind::Skill { .. } => {
                // Not handled locally; will be handled by the UI/agent separately
                false
            }
        }
    }

    pub fn slash_help_lines(&self) -> Vec<String> {
        all_commands()
            .iter()
            .map(|command| format!("/{:<8} {}", command.name, command.description))
            .collect()
    }

    fn active_slash_query(&self) -> Option<String> {
        let cursor = self
            .clamp_cursor_to_char_boundary(self.state.input_cursor)
            .min(self.state.input.len());
        let before_cursor = &self.state.input[..cursor];
        let trimmed = before_cursor.trim_start();
        let leading_ws = before_cursor.len().saturating_sub(trimmed.len());

        if !trimmed.starts_with('/') {
            return None;
        }
        if trimmed.split_whitespace().count() > 1 {
            return None;
        }
        if cursor < leading_ws {
            return None;
        }
        Some(trimmed.to_string())
    }
}

#[cfg(test)]
mod tests {
    use crate::app::{App, SlashAcceptResult};

    #[test]
    fn palette_opens_for_slash_prefix_and_closes_past_first_token() {
        let mut app = App::default();
        app.append_input("/cl");
        assert!(app.is_slash_palette_active());
        assert_eq!(app.state.slash_palette.matches[0].command_name, "clear");

        app.append_input(" more");
        assert!(!app.is_slash_palette_active());
    }

    #[test]
    fn completing_selection_only_fills_command_name() {
        let mut app = App::default();
        app.append_input("/rev");
        let result = app.complete_slash_selection();
        assert_eq!(result, SlashAcceptResult::CompletedInput);
        assert_eq!(app.state.input, "/review");
        assert!(!app.is_slash_palette_active());
    }

    #[test]
    fn exact_local_action_is_handled_without_submit() {
        let mut app = App::default();
        app.append_input("/help");
        let action = app.submit_slash_command_if_exact();
        assert!(action);
        assert!(
            app.state
                .live_messages
                .iter()
                .any(|message| message.content.contains("/clear"))
        );
    }

    #[test]
    fn exact_prompt_template_expands_on_submit() {
        let mut app = App::default();
        app.append_input("/review");
        let action = app.submit_slash_command_if_exact();
        assert!(action);
        assert_eq!(
            app.state.input,
            "Review the current changes, prioritize findings, and keep the summary brief."
        );
    }
}
