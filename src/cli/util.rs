use chia_wallet_sdk::{driver::StandardLayer, prelude::ToTreeHash, utils::Address};
use slot_machine::{CliError, SageClient, hex_string_to_pubkey};

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
