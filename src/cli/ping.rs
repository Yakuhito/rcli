use chia_wallet_sdk::{prelude::ToTreeHash, utils::Address};
use slot_machine::{CliError, SageClient};

use crate::get_first_address;

pub async fn cli_ping() -> Result<(), CliError> {
    let wallet = SageClient::new()?;

    let layer = get_first_address(&wallet).await?;

    println!(
        "Connected wallet first address (mainnet): {}",
        Address::new(layer.tree_hash().into(), "xch".to_string()).encode()?
    );
    println!(
        "Connected wallet first address (testnet): {}",
        Address::new(layer.tree_hash().into(), "txch".to_string()).encode()?
    );
    println!("PONG!");

    Ok(())
}
