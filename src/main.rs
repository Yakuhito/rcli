use clap::{Parser, Subcommand};
use rcli::{cli_issue, cli_launch_vault, cli_ping, cli_revoke, cli_revoke_bulk};

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

    /// Issue the vault's rCAT
    Issue {
        /// The vault launcher id
        #[arg(long)]
        launcher_id: String,

        /// The amount of rCATs to issue
        #[arg(long, default_value = "1337.420")]
        cat_amount: String,

        /// Transaction fee
        #[arg(long, default_value = "0")]
        fee: String,

        /// Use testnet11
        #[arg(long, default_value = "false")]
        testnet11: bool,
    },

    /// Revoke the vault's rCAT
    Revoke {
        /// The vault launcher id
        #[arg(long)]
        launcher_id: String,

        /// Comma-separated list of rCAT coin ids to revoke
        #[arg(long)]
        coin_ids: String,

        /// Transaction fee
        #[arg(long, default_value = "0")]
        fee: String,

        /// Use testnet11
        #[arg(long, default_value = "false")]
        testnet11: bool,
    },

    /// Revoke a bulk of rCATs
    RevokeBulk {
        /// The vault launcher id
        #[arg(long)]
        launcher_id: String,

        /// Minimum total number of coins to revoke
        #[arg(long, default_value = "16")]
        min_coins: usize,

        /// Maximum total number of coins to revoke
        #[arg(long, default_value = "128")]
        max_coins: usize,

        /// Minimum coin amount to revoke
        #[arg(long, default_value = "1.00")]
        min_coin_amount: String,

        /// Comma-separated list of addresses to NOT revoke from
        #[arg(long)]
        exclude_addresses: String,

        /// Transaction fee
        #[arg(long, default_value = "0")]
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
        Commands::Issue {
            launcher_id,
            cat_amount,
            fee,
            testnet11,
        } => cli_issue(launcher_id, cat_amount, fee, testnet11).await,
        Commands::Revoke {
            launcher_id,
            coin_ids,
            fee,
            testnet11,
        } => cli_revoke(launcher_id, coin_ids, fee, testnet11).await,
        Commands::RevokeBulk {
            launcher_id,
            min_coins,
            max_coins,
            min_coin_amount,
            exclude_addresses,
            fee,
            testnet11,
        } => {
            cli_revoke_bulk(
                launcher_id,
                min_coins,
                max_coins,
                min_coin_amount,
                exclude_addresses,
                fee,
                testnet11,
            )
            .await
        }
    };

    if let Err(err) = res {
        eprintln!("Error: {err}");
    }
}
