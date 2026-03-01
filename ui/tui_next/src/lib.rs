pub mod app;
pub mod approval;
pub mod history_cell;
pub mod input;
pub mod insert_history;
pub mod state;
pub mod terminal;
pub mod tui;
pub mod wrapping;

pub async fn run() -> anyhow::Result<()> {
    crate::runtime::run().await
}

mod runtime;
mod runtime_loop;
