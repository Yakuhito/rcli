use chia::protocol::{Bytes, Bytes32, Coin, SpendBundle};
use chia_puzzle_types::{Memos, cat::CatArgs, singleton::SingletonStruct};
use chia_wallet_sdk::{
    coinset::ChiaRpcClient,
    driver::{
        Cat, CatInfo, CatSpend, Offer, SingletonInfo, Spend, SpendContext, create_security_coin,
        decode_offer, spend_security_coin,
    },
    prelude::ToTreeHash,
    test::print_spend_bundle_to_file,
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

pub async fn cli_issue(
    launcher_id_str: String,
    cat_amount_str: String,
    fee_str: String,
    testnet11: bool,
) -> Result<(), CliError> {
    let launcher_id = hex_string_to_bytes32(&launcher_id_str)?;
    let cat_amount = parse_amount(&cat_amount_str, true)?;
    let fee = parse_amount(&fee_str, false)?;

    let mut ctx = SpendContext::new();
    let client = get_coinset_client(testnet11);

    let (MultisigSingleton::Vault(vault), _) =
        sync_multisig_singleton::<()>(&client, &mut ctx, launcher_id, None).await?
    else {
        return Err(CliError::Custom("Could not sync vault".to_string()));
    };

    println!("Latest vault coin: {:}", hex::encode(vault.coin.coin_id()));

    let tail_args = EverythingWithSingletonTailArgs::new(launcher_id, 0);
    let tail_ptr = ctx.curry(tail_args)?;
    let asset_id: Bytes32 = tail_args.curry_tree_hash().into();
    println!("rCAT asset id: {:}", hex::encode(asset_id));

    let singleton_struct_hash: Bytes32 = SingletonStruct::new(launcher_id).tree_hash().into();
    // let hidden_puzzle = P2DelegatedBySingletonLayer::new(singleton_struct_hash, 0);
    let hidden_puzzle_hash: Bytes32 =
        P2DelegatedBySingletonLayerArgs::curry_tree_hash(singleton_struct_hash, 0).into();
    println!("Hidden puzzle hash: {:}", hex::encode(hidden_puzzle_hash));

    let wallet = SageClient::new()?;
    let offer_resp = wallet
        .make_offer(
            no_assets(),
            assets_xch_only(cat_amount),
            fee,
            None,
            None,
            true,
        )
        .await?;
    println!(
        "Offer with id {} created.",
        hex::encode(offer_resp.offer_id)
    );

    // Create security coin
    let offer = Offer::from_spend_bundle(&mut ctx, &decode_offer(&offer_resp.offer)?)?;
    let (security_sk, security_coin) =
        create_security_coin(&mut ctx, offer.offered_coins().xch[0])?;

    // Spend security coin, which will create the eve CAT and assert it's spent
    // To do that, we need the eve CAT's full puzzle hash
    // The inner puzzle of the eve CAT just sends the whole amount to the user's address
    let layer = get_first_address(&wallet).await?;
    let user_ph: Bytes32 = layer.tree_hash().into();
    println!(
        "Newly-created CATs will be sent to: {}",
        Address::new(user_ph, get_prefix(testnet11)).encode()?
    );

    let eve_cat_tail_solution = ctx.alloc(&EverythingWithSingletonTailSolution {
        singleton_inner_puzzle_hash: vault.info.inner_puzzle_hash().into(),
    })?;
    let eve_cat_coin_conditions = Conditions::new()
        .create_coin(user_ph, cat_amount, ctx.hint(user_ph)?)
        .run_cat_tail(tail_ptr, eve_cat_tail_solution);
    let eve_cat_inner_puzzle = ctx.alloc(&clvm_quote!(eve_cat_coin_conditions))?;
    let eve_cat_inner_puzzle_hash: Bytes32 = ctx.tree_hash(eve_cat_inner_puzzle).into();

    let eve_cat_full_puzzle_hash = CatArgs::curry_tree_hash(
        asset_id,
        RevocationArgs::new(hidden_puzzle_hash, eve_cat_inner_puzzle_hash).curry_tree_hash(),
    );
    let eve_cat_coin = Coin::new(
        security_coin.coin_id(),
        eve_cat_full_puzzle_hash.into(),
        cat_amount,
    );

    let security_coin_sig = spend_security_coin(
        &mut ctx,
        security_coin,
        Conditions::new()
            .create_coin(eve_cat_full_puzzle_hash.into(), cat_amount, Memos::None)
            .assert_concurrent_spend(eve_cat_coin.coin_id()),
        &security_sk,
        get_constants(testnet11),
    )?;

    // Spend eve CAT
    let _ = Cat::spend_all(
        &mut ctx,
        &[CatSpend::new(
            Cat::new(
                eve_cat_coin,
                None,
                CatInfo::new(
                    asset_id,
                    Some(hidden_puzzle_hash),
                    eve_cat_inner_puzzle_hash,
                ),
            ),
            Spend::new(eve_cat_inner_puzzle, NodePtr::NIL),
        )],
    )?;

    // Spend vault - which needs to send a message to the eve CAT
    //  to approve issuance
    // Note: When issuing, message = delta = 0
    let receiver_coin_id = ctx.alloc(&eve_cat_coin.coin_id())?;
    let vault_hint = ctx.hint(launcher_id)?;
    let conditions = Conditions::new()
        .send_message(23, Bytes::new(vec![]), vec![receiver_coin_id])
        .create_coin(
            vault.info.inner_puzzle_hash().into(),
            vault.coin.amount,
            vault_hint,
        );
    vault.spend(
        &mut ctx,
        &[layer.synthetic_key],
        conditions,
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
    print_spend_bundle_to_file(
        sb.coin_spends.clone(),
        sb.aggregated_signature.clone(),
        "sb.debug",
    );

    let resp = client.push_tx(sb).await?;

    println!("Transaction submitted; status='{}'", resp.status);

    wait_for_coin(&client, security_coin.coin_id(), true).await?;
    println!("Confirmed!");

    Ok(())
}
