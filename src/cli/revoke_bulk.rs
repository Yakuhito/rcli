use chia::protocol::Bytes32;
use chia_wallet_sdk::types::Mod;
use slot_machine::{CliError, hex_string_to_bytes32, parse_amount};

use crate::{EverythingWithSingletonTailArgs, SpaceScanClient};

pub async fn cli_revoke_bulk(
    launcher_id_str: String,
    min_coins: usize,
    max_coins: usize,
    min_coin_amount_str: String,
    fee_str: String,
    testnet11: bool,
) -> Result<(), CliError> {
    let launcher_id = hex_string_to_bytes32(&launcher_id_str)?;
    let min_coin_amount = parse_amount(&min_coin_amount_str, true)?;
    let fee = parse_amount(&fee_str, false)?;

    let tail_args = EverythingWithSingletonTailArgs::new(launcher_id, 0);
    let asset_id: Bytes32 = tail_args.curry_tree_hash().into();
    println!("rCAT asset id: {:}", hex::encode(asset_id));

    println!("Getting top holders from the SpaceScan.io API...");
    let spacescan_client = SpaceScanClient::new(testnet11);
    let holders = spacescan_client
        .get_token_holders(asset_id, max_coins)
        .await?;
    println!("Got {} holders.", holders.count);

    et (MultisigSingleton::Vault(vault), _) =
        sync_multisig_singleton::<()>(&client, &mut ctx, launcher_id, None).await?
    else {
        return Err(CliError::Custom("Could not sync vault".to_string()));
    };

    println!("Latest vault coin: {:}", hex::encode(vault.coin.coin_id()));

    Ok(())
}
