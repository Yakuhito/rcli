use std::borrow::Cow;

use chia::{
    clvm_utils::{ToTreeHash, TreeHash},
    protocol::Bytes32,
};
use chia_puzzle_types::singleton::SingletonStruct;
use chia_puzzles::SINGLETON_TOP_LAYER_V1_1_HASH;
use chia_wallet_sdk::types::Mod;
use clvm_traits::{FromClvm, ToClvm};
use hex_literal::hex;

// https://github.com/greimela/chia-blockchain/blob/b29d87fcbecf817bb0eda9c4bd8e823facf5a359/chia/wallet/revocable_cats/everything_with_singleton.clsp
pub const EVERYTHING_WITH_SINGLETON_TAIL: [u8; 283] = hex!(
    "
    ff02ffff01ff04ffff04ff04ffff04ffff0117ffff04ff82017fffff04ffff0b
    ff2effff0bff0affff0bff0aff36ff0580ffff0bff0affff0bff3effff0bff0a
    ffff0bff0aff36ff0b80ffff0bff0affff0bff3effff0bff0affff0bff0aff36
    ff8209ff80ffff0bff0aff36ff26808080ff26808080ff26808080ff80808080
    80ff8080ffff04ffff01ff43ff02ffffa04bf5122f344554c53bde2ebb8cd2b7
    e3d1600ad631c385a5d7cce23c7785459aa09dcf97a184f32623d11a73124ceb
    99a5709b083721e878a16d78f596718ba7b2ffa102a12871fee210fb8619291e
    aea194581cbd2531e4b23759d225f6806923f63222a102a8d5dd63fba471ebcb
    1f3e8f7c1e1879b7152a6e7298a91ce119a63400ade7c5ff018080
    "
);

pub const EVERYTHING_WITH_SINGLETON_TAIL_HASH: TreeHash = TreeHash::new(hex!(
    "
    0876da2005fe6262d4504c27a1b6379227aba8adbbad3758cb0e329a4e74c6cc
    "
));

#[derive(ToClvm, FromClvm, Debug, Clone, Copy, PartialEq, Eq)]
#[clvm(curry)]
pub struct EverythingWithSingletonTailArgs {
    pub singleton_mod_hash: Bytes32,
    pub singleton_struct_hash: Bytes32,
    pub nonce: u64,
}

impl EverythingWithSingletonTailArgs {
    pub fn new(singleton_launcher_id: Bytes32, nonce: u64) -> Self {
        Self {
            singleton_mod_hash: SINGLETON_TOP_LAYER_V1_1_HASH.into(),
            singleton_struct_hash: SingletonStruct::new(singleton_launcher_id)
                .tree_hash()
                .into(),
            nonce,
        }
    }
}

#[derive(FromClvm, ToClvm, Debug, Clone, PartialEq, Eq)]
#[clvm(list)]
pub struct EverythingWithSingletonTailSolution {
    pub singleton_inner_puzzle_hash: Bytes32,
}

impl Mod for EverythingWithSingletonTailArgs {
    fn mod_reveal() -> Cow<'static, [u8]> {
        Cow::Borrowed(&EVERYTHING_WITH_SINGLETON_TAIL)
    }

    fn mod_hash() -> TreeHash {
        EVERYTHING_WITH_SINGLETON_TAIL_HASH
    }
}
