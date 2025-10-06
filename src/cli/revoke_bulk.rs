use chia::{clvm_utils::ToTreeHash, protocol::Bytes32};
use chia_puzzle_types::singleton::SingletonStruct;
use chia_wallet_sdk::{
    coinset::ChiaRpcClient,
    types::{Mod, puzzles::P2DelegatedBySingletonLayerArgs},
    utils::Address,
};
use csv::ReaderBuilder;
use hex::FromHex;
use serde::Deserialize;
use slot_machine::{CliError, get_coinset_client, hex_string_to_bytes32, parse_amount};
use std::{fs::File, path::Path};

use crate::{EverythingWithSingletonTailArgs, revoke_coins};

#[allow(clippy::too_many_arguments)]
pub async fn cli_revoke_bulk(
    launcher_id_str: String,
    csv: String,
    percentage: u8,
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

    println!("Getting holders from '{}'...", csv);
    let holders = load_holders_csv(csv)?;
    println!("Got {} holders.", holders.len());

    println!("Fetching rCAT coin records...");
    let client = get_coinset_client(testnet11);

    let mut excluded_puzzle_hashes = Vec::new();
    for address in exclude_addresses.split(',') {
        println!("Excluding address: {}", address);
        let puzzle_hash = Address::decode(address)?.puzzle_hash;
        excluded_puzzle_hashes.push(puzzle_hash);
    }

    let mut coin_names = Vec::new();
    for holder in holders {
        if excluded_puzzle_hashes.contains(&holder.puzzle_hash) {
            continue;
        }
        coin_names.push(holder.coin_name);
    }

    // Temp fix until coinset fixes their stuff
    // let Some(mut coin_records) = client
    //     .get_coin_records_by_names(coin_names, None, None, Some(false))
    //     .await?
    //     .coin_records
    // else {
    //     return Err(CliError::Custom("No coin records found".to_string()));
    // };
    let mut coin_records = Vec::new();
    for coin_name in coin_names {
        let Some(record) = client.get_coin_record_by_name(coin_name).await?.coin_record else {
            continue;
        };

        if record.spent {
            continue;
        }

        coin_records.push(record);
    }
    // end temp fix

    coin_records = coin_records
        .into_iter()
        .filter(|cr| !cr.spent && cr.coin.amount >= min_coin_amount)
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
        percentage,
        fee,
        asset_id,
        hidden_puzzle_hash,
        &client,
        coin_records,
    )
    .await
}

fn serde_hex_string_to_bytes32<'de, D>(deserializer: D) -> Result<Bytes32, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s: &str = Deserialize::deserialize(deserializer)?;
    let bytes = <[u8; 32]>::from_hex(s.replace("0x", "")).map_err(serde::de::Error::custom)?;
    Ok(Bytes32::new(bytes))
}

#[derive(Debug, Deserialize, Clone)]
pub struct HolderCoinRecord {
    #[serde(deserialize_with = "serde_hex_string_to_bytes32")]
    pub coin_name: Bytes32,
    #[serde(deserialize_with = "serde_hex_string_to_bytes32")]
    pub puzzle_hash: Bytes32,
    pub amount: u64,
}

pub fn load_holders_csv<P: AsRef<Path>>(path: P) -> Result<Vec<HolderCoinRecord>, CliError> {
    let file = File::open(path)?;
    let mut rdr = ReaderBuilder::new().has_headers(true).from_reader(file);

    let mut records = Vec::new();
    for result in rdr.deserialize() {
        let record: HolderCoinRecord = result.map_err(CliError::Csv)?;
        records.push(record);
    }

    Ok(records)
}
