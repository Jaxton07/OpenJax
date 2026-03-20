pub mod builtin;
pub mod dispatch;
pub mod kinds;
pub mod registry;

pub use dispatch::dispatch_slash_command;
pub use kinds::{SlashCommandKind, SlashResult};
pub use registry::{register_skill_command, SlashCommand, SlashMatch, SlashCommandRegistry};
