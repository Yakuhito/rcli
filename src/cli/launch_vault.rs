use chia_wallet_sdk::{
    coinset::ChiaRpcClient,
    driver::{
        Launcher, MedievalVaultHint, Offer, SpendContext, create_security_coin, decode_offer,
        spend_security_coin,
    },
    prelude::{SpendBundle, ToTreeHash},
    types::puzzles::P2MOfNDelegateDirectArgs,
    utils::Address,
};
use slot_machine::{
    CliError, SageClient, assets_xch_only, get_coinset_client, get_constants, get_prefix,
    no_assets, parse_amount, wait_for_coin,
};

use crate::get_first_address;

pub async fn cli_launch_vault(fee_str: String, testnet11: bool) -> Result<(), CliError> {
    let fee = parse_amount(&fee_str, false)?;

    let mut ctx = SpendContext::new();
    let wallet = SageClient::new()?;

    let layer = get_first_address(&wallet).await?;
    println!(
        "Using first address: {}",
        Address::new(layer.tree_hash().into(), get_prefix(testnet11)).encode()?
    );

    let offer_resp = wallet
        .make_offer(assets_xch_only(1), no_assets(), fee, None, None, true)
        .await?;
    println!("Offer with id {} created", offer_resp.offer_id);

    let offer = Offer::from_spend_bundle(&mut ctx, &decode_offer(&offer_resp.offer)?)?;
    let (security_sk, security_coin) =
        create_security_coin(&mut ctx, offer.offered_coins().xch[0])?;

    let launcher = Launcher::new(security_coin.coin_id(), 1);
    let launcher_coin = launcher.coin();

    let m = 1;
    let pubkeys = vec![layer.synthetic_key];
    let launch_hints = MedievalVaultHint {
        my_launcher_id: launcher_coin.coin_id(),
        m: 1,
        public_key_list: pubkeys.clone(),
    };
    println!(
        "Multisig (medieval launch) launcher id (SAVE THIS): {}",
        hex::encode(launcher_coin.coin_id().to_bytes())
    );

    let (create_conditions, _vault_coin) = launcher.spend(
        &mut ctx,
        P2MOfNDelegateDirectArgs::curry_tree_hash(m, pubkeys).into(),
        launch_hints,
    )?;

    let security_coin_sig = spend_security_coin(
        &mut ctx,
        security_coin,
        create_conditions,
        &security_sk,
        get_constants(testnet11),
    )?;

    let sb = offer.take(SpendBundle::new(ctx.take(), security_coin_sig));

    println!("Submitting transaction...");
    let client = get_coinset_client(testnet11);
    let resp = client.push_tx(sb).await?;

    println!("Transaction submitted; status='{}'", resp.status);

    wait_for_coin(&client, security_coin.coin_id(), true).await?;
    println!("Confirmed!");

    Ok(())
}
