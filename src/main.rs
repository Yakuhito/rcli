use clap::{Parser, Subcommand};
use rcli::{cli_issue_cat, cli_launch_vault, cli_ping};

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

    /// Launch a medieval vault (protected by a 1-of-1 of your first address)
    LaunchVault {
        /// Transaction fee
        #[arg(long, default_value = "0.00042")]
        fee: String,

        /// Use testnet11
        #[arg(long, default_value = "false")]
        testnet11: bool,
    },

    /// Issue more of the vault rCAT
    IssueCat {
        /// The vault launcher id
        #[arg(long)]
        launcher_id: String,

        /// The amount of rCATs to issue
        #[arg(long, default_value = "1337.420")]
        cat_amount: String,

        /// Transaction fee
        #[arg(long, default_value = "0.00042")]
        fee: String,

        /// Use testnet11
        #[arg(long, default_value = "false")]
        testnet11: bool,
    },
}

#[tokio::main]
async fn main() {
    let args = Cli::parse();

    let res = match args.command {
        Commands::Ping {} => cli_ping().await,
        Commands::LaunchVault { fee, testnet11 } => cli_launch_vault(fee, testnet11).await,
        Commands::IssueCat {
            launcher_id,
            cat_amount,
            fee,
            testnet11,
        } => cli_issue_cat(launcher_id, cat_amount, fee, testnet11).await,
    };

    if let Err(err) = res {
        eprintln!("Error: {err}");
    }
}
