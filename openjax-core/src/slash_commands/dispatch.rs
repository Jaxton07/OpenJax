use super::kinds::SlashResult;
use super::registry::SlashCommandRegistry;

pub fn dispatch_slash_command(input: &str) -> SlashResult {
    let normalized = input.trim().strip_prefix('/').unwrap_or(input);
    match SlashCommandRegistry::find(normalized) {
        Some(m) => m.kind.execute(),
        None => SlashResult::Err(format!("unknown command: /{}", normalized)),
    }
}
