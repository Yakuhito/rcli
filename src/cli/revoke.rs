use chia::protocol::{Bytes32, SpendBundle};
use chia_puzzle_types::singleton::SingletonStruct;
use chia_wallet_sdk::{
    coinset::ChiaRpcClient,
    driver::{
        Cat, CatSpend, Layer, Offer, P2DelegatedBySingletonLayer, Puzzle, SingletonInfo,
        SpendContext, create_security_coin, decode_offer, spend_security_coin,
    },
    prelude::ToTreeHash,
    test::print_spend_bundle_to_file,
    types::{
        Conditions, Mod,
        puzzles::{P2DelegatedBySingletonLayerArgs, P2DelegatedBySingletonLayerSolution},
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

use crate::{EverythingWithSingletonTailArgs, get_first_address};

pub async fn cli_revoke(
    launcher_id_str: String,
    coin_ids_str: String,
    fee_str: String,
    testnet11: bool,
) -> Result<(), CliError> {
    let launcher_id = hex_string_to_bytes32(&launcher_id_str)?;
    let fee = parse_amount(&fee_str, false)?;
    let coin_ids = coin_ids_str
        .split(',')
        .map(hex_string_to_bytes32)
        .collect::<Result<Vec<Bytes32>, CliError>>()?;

    let mut ctx = SpendContext::new();
    let client = get_coinset_client(testnet11);

    let (MultisigSingleton::Vault(vault), _) =
        sync_multisig_singleton::<()>(&client, &mut ctx, launcher_id, None).await?
    else {
        return Err(CliError::Custom("Could not sync vault".to_string()));
    };

    println!("Latest vault coin: {:}", hex::encode(vault.coin.coin_id()));

    let tail_args = EverythingWithSingletonTailArgs::new(launcher_id, 0);
    let asset_id: Bytes32 = tail_args.curry_tree_hash().into();
    println!("rCAT asset id: {:}", hex::encode(asset_id));

    let singleton_struct_hash: Bytes32 = SingletonStruct::new(launcher_id).tree_hash().into();
    let hidden_puzzle_hash: Bytes32 =
        P2DelegatedBySingletonLayerArgs::curry_tree_hash(singleton_struct_hash, 0).into();
    println!("Hidden puzzle hash: {:}", hex::encode(hidden_puzzle_hash));

    println!("Fetching rCAT coin records...");
    let coin_ids_len = coin_ids.len();
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

    let mut total_cat_amount = 0;
    let mut cats: Vec<Cat> = Vec::with_capacity(coin_ids_len);
    for coin_record in coin_records {
        println!(
            "Parsing parent spend for coin 0x{}...",
            hex::encode(coin_record.coin.coin_id())
        );
        let Some(parent_spend) = client
            .get_puzzle_and_solution(
                coin_record.coin.coin_id(),
                Some(coin_record.confirmed_block_index),
            )
            .await?
            .coin_solution
        else {
            return Err(CliError::CoinNotSpent(coin_record.coin.coin_id()));
        };

        let parent_puzzle = ctx.alloc(&parent_spend.puzzle_reveal)?;
        let parent_puzzle = Puzzle::parse(&ctx, parent_puzzle);
        let parent_solution = ctx.alloc(&parent_spend.solution)?;
        let Some(children) =
            Cat::parse_children(&mut ctx, parent_spend.coin, parent_puzzle, parent_solution)?
        else {
            return Err(CliError::Custom(
                "Failed to parse parent CAT spend".to_string(),
            ));
        };

        let cat_coin_id = coin_record.coin.coin_id();
        let cat = children
            .into_iter()
            .find(|c| c.coin.coin_id() == cat_coin_id)
            .unwrap();

        if cat.info.asset_id != asset_id || cat.info.hidden_puzzle_hash != Some(hidden_puzzle_hash)
        {
            return Err(CliError::Custom(format!(
                "Coin {} is a CAT but has the wrong asset id/puzzle hash",
                hex::encode(cat_coin_id)
            )));
        }

        cats.push(cat);
        total_cat_amount += cat.coin.amount;
    }

    println!(
        "Revoking {} rCATs (total amount {:.3})...",
        cats.len(),
        total_cat_amount as f64 / 1000.0
    );

    let wallet = SageClient::new()?;
    let offer_resp = wallet
        .make_offer(no_assets(), assets_xch_only(1), fee, None, None, true)
        .await?;
    println!(
        "Offer with id {} created.",
        hex::encode(offer_resp.offer_id)
    );

    // Create security coin
    let offer = Offer::from_spend_bundle(&mut ctx, &decode_offer(&offer_resp.offer)?)?;
    let (security_sk, security_coin) =
        create_security_coin(&mut ctx, offer.offered_coins().xch[0])?;

    // Spend security coin, which will create the p2 singleton coin that
    //   does the messaging
    let layer = get_first_address(&wallet).await?;
    let user_ph: Bytes32 = layer.tree_hash().into();
    println!(
        "Revoked CATs will be sent to: {}",
        Address::new(user_ph, get_prefix(testnet11)).encode()?
    );

    let security_coin_sig = spend_security_coin(
        &mut ctx,
        security_coin,
        Conditions::new().assert_concurrent_spend(cats[0].coin.coin_id()),
        &security_sk,
        get_constants(testnet11),
    )?;

    // Spend rCATs
    let singleton_struct_hash: Bytes32 = SingletonStruct::new(launcher_id).tree_hash().into();
    let hidden_puzzle_layer = P2DelegatedBySingletonLayer::new(singleton_struct_hash, 0);
    let singleton_inner_puzzle_hash: Bytes32 = vault.info.inner_puzzle_hash().into();

    let mut cat_spends: Vec<CatSpend> = Vec::with_capacity(cats.len());
    let mut vault_conditions = Conditions::new();
    for (i, cat) in cats.into_iter().enumerate() {
        let delegated_puzzle = if i == 0 {
            let user_hint = ctx.hint(user_ph)?;
            ctx.alloc(&clvm_quote!(Conditions::new().create_coin(
                user_ph,
                total_cat_amount,
                user_hint
            )))?
        } else {
            NodePtr::NIL
        };

        let delegated_puzzle_hash: Bytes32 = ctx.tree_hash(delegated_puzzle).into();
        vault_conditions = vault_conditions.send_message(
            23,
            delegated_puzzle_hash.into(),
            vec![ctx.alloc(&cat.coin.coin_id())?],
        );

        let inner_spend = hidden_puzzle_layer.construct_spend(
            &mut ctx,
            P2DelegatedBySingletonLayerSolution {
                singleton_inner_puzzle_hash,
                delegated_puzzle,
                delegated_solution: NodePtr::NIL,
            },
        )?;
        cat_spends.push(CatSpend::revoke(cat, inner_spend));
    }

    let _ = Cat::spend_all(&mut ctx, &cat_spends)?;

    // Spend vault
    let vault_hint = ctx.hint(launcher_id)?;
    vault_conditions = vault_conditions.create_coin(
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
