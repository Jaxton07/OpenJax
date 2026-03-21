use openjax_core::slash_commands::{
    SlashCommand as CoreSlashCommand, SlashCommandRegistry, SlashMatch,
};

pub fn all_commands() -> Vec<CoreSlashCommand> {
    SlashCommandRegistry::all_commands()
}

pub fn find_exact(query: &str) -> Option<SlashMatch> {
    SlashCommandRegistry::find(query)
}

pub fn match_commands(query: &str, limit: usize) -> Vec<SlashMatch> {
    SlashCommandRegistry::match_prefix(query, limit)
}
