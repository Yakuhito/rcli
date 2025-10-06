use chia::{
    protocol::{Bytes32, Coin},
    traits::Streamable,
};
use chia_puzzle_types::{Memos, singleton::SingletonStruct};
use chia_wallet_sdk::{
    driver::{Layer, P2DelegatedBySingletonLayer, SingletonInfo, SpendContext},
    prelude::ToTreeHash,
    test::print_spend_bundle_to_file,
    types::{
        Conditions, Mod,
        puzzles::{P2DelegatedBySingletonLayerArgs, P2DelegatedBySingletonLayerSolution},
    },
};
use clvm_traits::clvm_quote;
use clvmr::NodePtr;
use slot_machine::{
    CliError, MultisigSingleton, SageClient, get_coinset_client, get_constants,
    hex_string_to_bytes32, hex_string_to_signature, sync_multisig_singleton,
};

use crate::get_first_address;

pub async fn cli_generate_send_message_bundle(
    launcher_id_str: String,
    message: u64,
    receiver_puzzle_hash_str: String,
    output_file: String,
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

    println!("Latest vault coin: {:}", hex::encode(vault.coin.coin_id()));

    // Get wallet
    let wallet = SageClient::new()?;
    let user_layer = get_first_address(&wallet).await?;
    let singleton_struct_hash: Bytes32 = SingletonStruct::new(launcher_id).tree_hash().into();
    let p2_layer = P2DelegatedBySingletonLayer::new(singleton_struct_hash, 0);

    // Spend p2 coin
    let p2_coin = Coin::new(
        vault.coin.coin_id(),
        P2DelegatedBySingletonLayerArgs::new(singleton_struct_hash, 0)
            .curry_tree_hash()
            .into(),
        0,
    );

    let receiver_puzzle_hash_ptr = ctx.alloc(&receiver_puzzle_hash)?;
    let p2_delegated_puzzle = ctx.alloc(&clvm_quote!(Conditions::new().send_message(
        18,
        message.to_bytes().unwrap().into(),
        vec![receiver_puzzle_hash_ptr]
    )))?;

    let p2_delegated_puzzle_hash: Bytes32 = ctx.tree_hash(p2_delegated_puzzle).into();

    let p2_spend = p2_layer.construct_spend(
        &mut ctx,
        P2DelegatedBySingletonLayerSolution {
            singleton_inner_puzzle_hash: vault.info.inner_puzzle_hash().into(),
            delegated_puzzle: p2_delegated_puzzle,
            delegated_solution: NodePtr::NIL,
        },
    )?;
    ctx.spend(p2_coin, p2_spend)?;

    // Spend vault to create p2 coin
    let vault_hint = ctx.hint(launcher_id)?;
    let vault_conditions = Conditions::new()
        .create_coin(p2_coin.puzzle_hash, 0, Memos::None)
        .send_message(
            23,
            p2_delegated_puzzle_hash.into(),
            vec![ctx.alloc(&p2_coin.coin_id())?],
        )
        .create_coin(
            vault.info.inner_puzzle_hash().into(),
            vault.coin.amount,
            vault_hint,
        );
    vault.spend(
        &mut ctx,
        &[user_layer.synthetic_key],
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

    // Print final bundle
    print_spend_bundle_to_file(spends, vault_sig, &output_file);
    println!("Spend bundle saved to '{}'", output_file);

    Ok(())
}
