mod cli;

use clap::{Parser, Subcommand};
use cli::*;

#[derive(Parser)]
#[command(
    name = "rCAT CLI",
    version,
    about = "A CLI for interacting with rCATs and medieval vaults via the Sage wallet RPC"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Verifies Sage RPC connection and fetches the connected wallet's first address
    Ping {},
}

#[tokio::main]
async fn main() {
    let args = Cli::parse();

    let res = match args.command {
        Commands::Ping {} => cli_ping().await,
    };

    if let Err(err) = res {
        eprintln!("Error: {err}");
    }
}
