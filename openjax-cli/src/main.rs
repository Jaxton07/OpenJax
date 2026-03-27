use clap::{Parser, Subcommand};

mod update;

#[derive(Parser)]
#[command(
    name = "openjax",
    version,
    about = "OpenJax CLI — manage your OpenJax installation"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Update OpenJax to the latest version
    Update(update::UpdateArgs),
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Update(args) => update::run(args).await,
    }
}
