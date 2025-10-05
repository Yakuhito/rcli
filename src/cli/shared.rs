use chia::protocol::{Bytes32, SpendBundle};
use chia_puzzle_types::singleton::SingletonStruct;
use chia_wallet_sdk::{
    coinset::{ChiaRpcClient, CoinRecord, CoinsetClient},
    driver::{
        Asset, Cat, CatSpend, Layer, Offer, P2DelegatedBySingletonLayer, Puzzle, SingletonInfo,
        SpendContext, StandardLayer, create_security_coin, decode_offer, spend_security_coin,
    },
    prelude::ToTreeHash,
    types::{
        Conditions, Mod,
        puzzles::{P2DelegatedBySingletonLayerSolution, RevocationArgs},
    },
    utils::Address,
};
use clvm_traits::clvm_quote;
use clvmr::NodePtr;
use slot_machine::{
    CliError, MultisigSingleton, SageClient, assets_xch_only, get_constants, get_prefix,
    hex_string_to_pubkey, hex_string_to_signature, no_assets, sync_multisig_singleton,
    wait_for_coin,
};

pub async fn get_first_address(wallet: &SageClient) -> Result<StandardLayer, CliError> {
    let first_derivation_record = &wallet.get_derivations(false, 0, 1).await?.derivations[0];
    let puzzle_hash_from_record = Address::decode(&first_derivation_record.address)?;

    let layer = StandardLayer::new(hex_string_to_pubkey(&first_derivation_record.public_key)?);

    if puzzle_hash_from_record.puzzle_hash != layer.tree_hash().into() {
        return Err(CliError::Custom(
            "Puzzle hash from record does not match standard layer hash".to_string(),
        ));
    }

    Ok(layer)
}

#[allow(clippy::too_many_arguments)]
pub async fn revoke_coins(
    launcher_id: Bytes32,
    testnet11: bool,
    percentage: u8,
    fee: u64,
    asset_id: Bytes32,
    hidden_puzzle_hash: Bytes32,
    client: &CoinsetClient,
    coin_records: Vec<CoinRecord>,
) -> Result<(), CliError> {
    println!("Revoking {} coins...", coin_records.len());

    let mut ctx = SpendContext::new();

    let (MultisigSingleton::Vault(vault), _) =
        sync_multisig_singleton::<()>(client, &mut ctx, launcher_id, None).await?
    else {
        return Err(CliError::Custom("Could not sync vault".to_string()));
    };

    println!("Latest vault coin: {:}", hex::encode(vault.coin.coin_id()));

    let mut total_cat_amount = 0;
    let mut total_revoked_amount = 0;
    let mut amount_to_revoke: Vec<u64> = Vec::with_capacity(coin_records.len());
    let mut cats: Vec<Cat> = Vec::with_capacity(coin_records.len());
    for coin_record in coin_records {
        if coin_record.spent {
            return Err(CliError::Custom(format!(
                "Coin {} already spent",
                hex::encode(coin_record.coin.coin_id())
            )));
        }
        println!(
            "Parsing parent spend for coin 0x{}...",
            hex::encode(coin_record.coin.coin_id())
        );
        let Some(parent_spend) = client
            .get_puzzle_and_solution(
                coin_record.coin.parent_coin_info,
                Some(coin_record.confirmed_block_index),
            )
            .await?
            .coin_solution
        else {
            return Err(CliError::CoinNotSpent(coin_record.coin.parent_coin_info));
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

        let amount_to_keep = cat.coin.amount * percentage as u64 / 100;
        let to_revoke = cat.coin.amount - amount_to_keep;

        amount_to_revoke.push(to_revoke);
        total_revoked_amount += to_revoke;
    }

    println!(
        "Revoking {} rCATs (total amount {:.3}; total revoked amount {:.3})...",
        cats.len(),
        total_cat_amount as f64 / 1000.0,
        total_revoked_amount as f64 / 1000.0
    );

    let wallet = SageClient::new()?;
    let offer_resp = wallet
        .make_offer(no_assets(), assets_xch_only(1), fee, None, None, false)
        .await?;
    println!("Offer with id {} created.", offer_resp.offer_id);

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
        let owner_refund_ph = RevocationArgs::new(hidden_puzzle_hash, cat.p2_puzzle_hash())
            .curry_tree_hash()
            .into();
        let owner_refund_hint = ctx.hint(owner_refund_ph)?;

        let base_condition = if amount_to_revoke[i] == cat.coin.amount {
            Conditions::new()
        } else {
            Conditions::new().create_coin(
                owner_refund_ph,
                cat.coin.amount - amount_to_revoke[i],
                owner_refund_hint,
            )
        };

        let delegated_puzzle = if i == 0 {
            let target_puzzle_hash = RevocationArgs::new(hidden_puzzle_hash, user_ph)
                .curry_tree_hash()
                .into();
            let user_hint = ctx.hint(user_ph)?;

            ctx.alloc(&clvm_quote!(base_condition.create_coin(
                target_puzzle_hash,
                total_revoked_amount,
                user_hint
            )))?
        } else {
            ctx.alloc(&clvm_quote!(base_condition))?
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
    let resp = client.push_tx(sb).await?;

    println!("Transaction submitted; status='{}'", resp.status);

    wait_for_coin(client, security_coin.coin_id(), true).await?;
    println!("Confirmed!");

    Ok(())
}
