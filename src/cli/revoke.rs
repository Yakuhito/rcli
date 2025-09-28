use chia::{clvm_utils::ToTreeHash, protocol::Bytes32};
use chia_puzzle_types::singleton::SingletonStruct;
use chia_wallet_sdk::{
    coinset::ChiaRpcClient,
    types::{Mod, puzzles::P2DelegatedBySingletonLayerArgs},
};
use slot_machine::{CliError, get_coinset_client, hex_string_to_bytes32, parse_amount};

use crate::{EverythingWithSingletonTailArgs, revoke_coins};

pub async fn cli_revoke(
    launcher_id_str: String,
    coin_ids_str: String,
    fee_str: String,
    testnet11: bool,
) -> Result<(), CliError> {
    let launcher_id = hex_string_to_bytes32(&launcher_id_str)?;
    let fee = parse_amount(&fee_str, false)?;
    let coin_ids = coin_ids_str
        .replace("0x", "")
        .split(',')
        .map(hex_string_to_bytes32)
        .collect::<Result<Vec<Bytes32>, CliError>>()?;

    let tail_args = EverythingWithSingletonTailArgs::new(launcher_id, 0);
    let asset_id: Bytes32 = tail_args.curry_tree_hash().into();
    println!("rCAT asset id: {:}", hex::encode(asset_id));

    let singleton_struct_hash: Bytes32 = SingletonStruct::new(launcher_id).tree_hash().into();
    let hidden_puzzle_hash: Bytes32 =
        P2DelegatedBySingletonLayerArgs::curry_tree_hash(singleton_struct_hash, 0).into();
    println!("Hidden puzzle hash: {:}", hex::encode(hidden_puzzle_hash));

    println!("Fetching rCAT coin records...");
    let coin_ids_len = coin_ids.len();
    let client = get_coinset_client(testnet11);
    let Some(coin_records) = client
        .get_coin_records_by_names(coin_ids, None, None, Some(true))
        .await?
        .coin_records
    else {
        return Err(CliError::Custom("Error fetching coin records".to_string()));
    };
    if coin_records.len() != coin_ids_len {
        return Err(CliError::Custom(
            "Could not find one or more rCAT coins on-chain".to_string(),
        ));
    }

    revoke_coins(
        launcher_id,
        testnet11,
        fee,
        asset_id,
        hidden_puzzle_hash,
        &client,
        coin_records,
    )
    .await
}
