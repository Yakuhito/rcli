use chia::protocol::{Bytes, Bytes32, Coin, SpendBundle};
use chia_puzzle_types::{Memos, cat::CatArgs, singleton::SingletonStruct};
use chia_wallet_sdk::{
    coinset::ChiaRpcClient,
    driver::{
        Cat, CatInfo, CatSpend, Offer, SingletonInfo, Spend, SpendContext, create_security_coin,
        decode_offer, spend_security_coin,
    },
    prelude::ToTreeHash,
    types::{
        Conditions, Mod,
        puzzles::{P2DelegatedBySingletonLayerArgs, RevocationArgs},
    },
    utils::Address,
};
use clvm_traits::clvm_quote;
use clvmr::NodePtr;
use slot_machine::{
    CliError, MultisigSingleton, SageClient, assets_xch_only, get_coinset_client, get_constants,
    get_prefix, hex_string_to_bytes32, hex_string_to_signature, no_assets, parse_amount,
    sync_multisig_singleton, wait_for_coin,
};

use crate::{
    EverythingWithSingletonTailArgs, EverythingWithSingletonTailSolution, get_first_address,
};

pub async fn cli_generate_send_message_bundle(
    launcher_id_str: String,
    message: u64,
    receiver_puzzle_hash_str: String,
    p2_vault_coin_parent_id_str: String,
    testnet11: bool,
) -> Result<(), CliError> {
    let launcher_id = hex_string_to_bytes32(&launcher_id_str)?;
    let receiver_puzzle_hash = hex_string_to_bytes32(&receiver_puzzle_hash_str)?;

    let mut ctx = SpendContext::new();
    let client = get_coinset_client(testnet11);

    let (MultisigSingleton::Vault(vault), _) =
        sync_multisig_singleton::<()>(&client, &mut ctx, launcher_id, None).await?
    else {
        return Err(CliError::Custom("Could not sync vault".to_string()));
    };

    // Get wallet
    let wallet = SageClient::new()?;
    let layer = get_first_address(&wallet).await?;

    // Spend vault
    let vault_hint = ctx.hint(launcher_id)?;
    let vault_conditions = Conditions::new().create_coin(
        vault.info.inner_puzzle_hash().into(),
        vault.coin.amount,
        vault_hint,
    );
    vault.spend(
        &mut ctx,
        &[layer.synthetic_key],
        vault_conditions,
        get_constants(testnet11).genesis_challenge,
    )?;

    // Sign vault spend using wallet
    let spends = ctx.take();
    let vault_spend = spends.last().unwrap().clone();
    let vault_sig = hex_string_to_signature(
        &wallet
            .sign_coin_spends(vec![vault_spend], false, true)
            .await?
            .spend_bundle
            .aggregated_signature,
    )?;

    // Assemble final bundle and submit
    let sb = offer.take(SpendBundle::new(spends, security_coin_sig + &vault_sig));

    println!("Submitting transaction...");
    let resp = client.push_tx(sb).await?;

    println!("Transaction submitted; status='{}'", resp.status);

    wait_for_coin(client, security_coin.coin_id(), true).await?;
    println!("Confirmed!");

    Ok(())
}
