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
    /// Verifies the first address was derived correctly
    VerifyAddress {},
}

#[tokio::main]
async fn main() {
    let args = Cli::parse();

    let res = match args.command {
        Commands::VerifyAddress {} => verify_address().await,
    };

    if let Err(err) = res {
        eprintln!("Error: {err}");
    }
}
