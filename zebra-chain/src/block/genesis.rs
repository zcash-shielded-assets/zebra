//! Regtest genesis block

use std::sync::Arc;

use hex::FromHex;

use crate::{
    block::{Block, Hash},
    serialization::ZcashDeserializeInto,
};

/// Genesis block for Regtest, copied from zcashd via `getblock 0 0` RPC method
pub fn regtest_genesis_block() -> Arc<Block> {
    let regtest_genesis_block_bytes =
        <Vec<u8>>::from_hex(include_str!("genesis/block-regtest-0-000-000.txt").trim())
            .expect("Block bytes are in valid hex representation");

    regtest_genesis_block_bytes
        .zcash_deserialize_into()
        .map(Arc::new)
        .expect("hard-coded Regtest genesis block data must deserialize successfully")
}

/// Genesis block for ZSA Testnet 1.
///
/// Network magic: `ZSA1` (0x5A534131)
/// Activation: NU6 @ height 1
/// Timestamp: 2026-05-15 09:09:00 UTC
pub fn zsa1_genesis_block() -> Arc<Block> {
    let zsa1_genesis_block_bytes =
        <Vec<u8>>::from_hex(include_str!("genesis/block-zsa1-testnet.txt").trim())
            .expect("Block bytes are in valid hex representation");

    zsa1_genesis_block_bytes
        .zcash_deserialize_into()
        .map(Arc::new)
        .expect("hard-coded ZSA1 genesis block data must deserialize successfully")
}

/// Returns the genesis block for a known hardcoded genesis hash,
/// or `None` if the hash doesn't match any built-in genesis block.
pub fn known_genesis_block(hash: Hash) -> Option<Arc<Block>> {
    let zsa1 = zsa1_genesis_block();
    if zsa1.hash() == hash {
        return Some(zsa1);
    }

    let regtest = regtest_genesis_block();
    if regtest.hash() == hash {
        return Some(regtest);
    }

    None
}
