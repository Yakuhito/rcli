use chia::{clvm_utils::ToTreeHash, protocol::Bytes32};
use chia_puzzle_types::{cat::CatArgs, singleton::SingletonStruct};
use chia_wallet_sdk::{
    coinset::ChiaRpcClient,
    types::{
        Mod,
        puzzles::{P2DelegatedBySingletonLayerArgs, RevocationArgs},
    },
    utils::Address,
};
use slot_machine::{CliError, get_coinset_client, hex_string_to_bytes32, parse_amount};

use crate::{EverythingWithSingletonTailArgs, SpaceScanClient, revoke_coins};

pub async fn cli_revoke_bulk(
    launcher_id_str: String,
    min_coins: usize,
    max_coins: usize,
    min_coin_amount_str: String,
    exclude_addresses: String,
    fee_str: String,
    testnet11: bool,
) -> Result<(), CliError> {
    let launcher_id = hex_string_to_bytes32(&launcher_id_str)?;
    let min_coin_amount = parse_amount(&min_coin_amount_str, true)?;
    let fee = parse_amount(&fee_str, false)?;

    let tail_args = EverythingWithSingletonTailArgs::new(launcher_id, 0);
    let asset_id: Bytes32 = tail_args.curry_tree_hash().into();
    println!("rCAT asset id: {:}", hex::encode(asset_id));

    let singleton_struct_hash: Bytes32 = SingletonStruct::new(launcher_id).tree_hash().into();
    let hidden_puzzle_hash: Bytes32 =
        P2DelegatedBySingletonLayerArgs::curry_tree_hash(singleton_struct_hash, 0).into();
    println!("Hidden puzzle hash: {:}", hex::encode(hidden_puzzle_hash));

    println!("Getting top holders from the SpaceScan.io API...");
    let spacescan_client = SpaceScanClient::new(testnet11);
    let holders = spacescan_client
        .get_token_holders(asset_id, max_coins)
        .await?;
    println!("Got {} holders.", holders.tokens.len());

    println!("Fetching rCAT coin records...");
    let mut puzzle_hashes: Vec<Bytes32> = Vec::new();
    for holder in holders.tokens {
        let inner_ph = Address::decode(&holder.address)?.puzzle_hash;
        if exclude_addresses.contains(&holder.address) {
            continue;
        }

        puzzle_hashes.push(CatArgs::curry_tree_hash(asset_id, inner_ph.into()).into());
        puzzle_hashes.push(
            CatArgs::curry_tree_hash(
                asset_id,
                RevocationArgs::new(hidden_puzzle_hash, inner_ph).curry_tree_hash(),
            )
            .into(),
        );
    }

    let client = get_coinset_client(testnet11);
    let Some(mut coin_records) = client
        .get_coin_records_by_puzzle_hashes(puzzle_hashes, None, None, Some(false))
        .await?
        .coin_records
    else {
        return Err(CliError::Custom("No coin records found".to_string()));
    };

    coin_records = coin_records
        .into_iter()
        .filter(|cr| cr.coin.amount >= min_coin_amount)
        .collect::<Vec<_>>();
    coin_records.sort_unstable_by(|a, b| b.coin.amount.cmp(&a.coin.amount));

    if coin_records.len() < min_coins {
        return Err(CliError::Custom(format!(
            "Not enough coins to revoke: {} < {}",
            coin_records.len(),
            min_coins
        )));
    }
    if coin_records.len() > max_coins {
        coin_records.truncate(max_coins);
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
