mod core;
mod diff;
mod draw;
pub(crate) mod style_diff;

pub use core::{CrosstermTerminal, Frame, Terminal, init_crossterm_terminal};

#[cfg(test)]
mod tests;
