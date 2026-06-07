//! Regtest genesis block

use std::sync::Arc;

use hex::FromHex;

use crate::{block::Block, serialization::ZcashDeserializeInto};

/// Genesis block for the ZSA testnet, mined via `mine_zsa_testnet_genesis_block()`.
pub fn zsa_testnet_genesis_block() -> Arc<Block> {
    let zsa_testnet_genesis_block_bytes =
        <Vec<u8>>::from_hex(include_str!("genesis/block-zsatestnet-0-000-000.txt").trim())
            .expect("Block bytes are in valid hex representation");

    zsa_testnet_genesis_block_bytes
        .zcash_deserialize_into()
        .map(Arc::new)
        .expect("hard-coded ZSA testnet genesis block data must deserialize successfully")
}

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

/// Mines a ZSA testnet genesis block and returns the serialized block bytes and block hash.
///
/// Uses the Equihash internal miner to find a valid solution.
/// The genesis block has a single V1 coinbase transaction containing `coinbase_message`.
///
/// Computing the transaction hash via `Transaction::hash()` requires
/// `zcash_primitives` format compatibility, which may panic on some branches.
/// To avoid this, we serialize the coinbase transaction ourselves and compute
/// the double-SHA256 directly to get the merkle root.
///
/// Returns `(serialized_block_bytes, block_hash_hex_string)`.
///
/// # Panics
///
/// If the block cannot be mined or serialized.
#[cfg(feature = "internal-miner")]
pub fn mine_zsa_testnet_genesis_block() -> (Vec<u8>, String) {
    use crate::{
        amount::Amount,
        block::{header::ZCASH_BLOCK_VERSION, merkle::Root, Header},
        serialization::{AtLeastOne, CompactSizeMessage, ZcashSerialize},
        transaction::Transaction,
        transparent,
        work::{
            difficulty::{CompactDifficulty, ExpandedDifficulty, U256},
            equihash::{Solution, SOLUTION_SIZE},
        },
    };
    use chrono::{TimeZone, Utc};
    use sha2::{Digest, Sha256};

    // Build the coinbase transaction at height 0.
    // The genesis coinbase script is hardcoded in Zebra's serialization code.
    // We must use the exact same bytes as the original Zcash genesis blocks
    // for the block to serialize and deserialize correctly.
    // These bytes match `zebra_chain::transparent::serialize::GENESIS_COINBASE_SCRIPT_SIG`.
    let genesis_coinbase_data: [u8; 77] = [
        4, 255, 255, 7, 31, 1, 4, 69, 90, 99, 97, 115, 104, 48, 98, 57, 99, 52, 101, 101, 102, 56,
        98, 55, 99, 99, 52, 49, 55, 101, 101, 53, 48, 48, 49, 101, 51, 53, 48, 48, 57, 56, 52, 98,
        54, 102, 101, 97, 51, 53, 54, 56, 51, 97, 55, 99, 97, 99, 49, 52, 49, 97, 48, 52, 51, 99,
        52, 50, 48, 54, 52, 56, 51, 53, 100, 51, 52,
    ];
    let coinbase_tx = Transaction::V1 {
        inputs: vec![transparent::Input::Coinbase {
            height: crate::block::Height(0),
            data: genesis_coinbase_data.to_vec(),
            sequence: u32::MAX,
        }],
        outputs: vec![transparent::Output::new(
            Amount::new(1),
            transparent::Script::new(&[0x51]), // OP_TRUE
        )],
        lock_time: crate::transaction::LockTime::unlocked(),
    };

    // Serialize the transaction and compute its double-SHA256 hash directly,
    // avoiding `Transaction::hash()` which calls into zcash_primitives.
    let mut tx_bytes = Vec::new();
    coinbase_tx
        .zcash_serialize(&mut tx_bytes)
        .expect("coinbase transaction must serialize");

    let tx_hash: [u8; 32] = Sha256::digest(Sha256::digest(&tx_bytes)).into();

    // For a single-transaction block, the merkle root is the transaction hash.
    let merkle_root = Root(tx_hash);

    // Build the block header template.
    let header = Header {
        version: ZCASH_BLOCK_VERSION,
        previous_block_hash: crate::block::Hash([0; 32]),
        merkle_root,
        commitment_bytes: [0; 32].into(),
        time: Utc
            .timestamp_opt(1_700_000_000, 0)
            .single()
            .expect("timestamp must be valid"),
        // Testnet difficulty: 2^251 - 1, matching ZSA testnet network parameters.
        difficulty_threshold: ExpandedDifficulty::from((U256::one() << 251) - 1).to_compact(),
        nonce: [0; 32].into(),
        solution: Solution::Common([0; SOLUTION_SIZE]),
    };

    // Mine the block using the Equihash solver.
    let mined_solutions: AtLeastOne<Header> =
        Solution::solve(header, || Ok(())).expect("must be able to mine ZSA testnet genesis block");
    let mined_header = mined_solutions
        .into_iter()
        .next()
        .expect("at least one solution");

    let block_hash = mined_header.hash();

    // Build the full serialized block bytes: header + compactsize(tx_count) + transactions.
    let mut block_bytes = Vec::new();
    mined_header
        .zcash_serialize(&mut block_bytes)
        .expect("header must serialize");

    let tx_count = CompactSizeMessage::try_from(1usize).expect("1 is below the message size limit");
    tx_count
        .zcash_serialize(&mut block_bytes)
        .expect("tx count must serialize");
    block_bytes.extend_from_slice(&tx_bytes);

    (block_bytes, hex::encode(block_hash.0))
}

#[cfg(test)]
mod tests {
    /// Mines a ZSA testnet genesis block and prints the hex-encoded bytes and block hash.
    ///
    /// Run with:
    /// ```sh
    /// cargo test -p zebra-chain --features internal-miner -- \
    ///   block::genesis::tests::generate_zsa_testnet_genesis --nocapture
    /// ```
    #[cfg(feature = "internal-miner")]
    #[test]
    fn generate_zsa_testnet_genesis() {
        let _init_guard = zebra_test::init();

        let (block_bytes, hash_hex) = super::mine_zsa_testnet_genesis_block();

        let hex_block = hex::encode(&block_bytes);

        println!();
        println!("=== ZSA Testnet Genesis Block ===");
        println!("Block hash: {hash_hex}");
        println!();
        println!("Hex-encoded block bytes (store as block-zsatestnet-0-000-000.txt):");
        println!("{hex_block}");
        println!();
        println!("Update ZSA_TESTNET_GENESIS_HASH in zsa_testnet.rs to:");
        println!("\"{hash_hex}\"");
    }

    /// Verifies that the ZSA testnet genesis block fixture deserializes correctly
    /// and its hash matches the genesis hash in the ZSA testnet parameters.
    #[test]
    fn check_zsa_testnet_genesis_block_fixture() {
        let _init_guard = zebra_test::init();

        let block = super::zsa_testnet_genesis_block();
        let network = crate::parameters::Network::new_zsa_testnet();

        // Verify the hash matches the network's genesis hash
        let hash = block.header.hash();
        let expected_hash = network.genesis_hash();

        assert_eq!(
            hash, expected_hash,
            "ZSA testnet genesis block hash must match network genesis hash"
        );

        // Verify it has exactly one coinbase transaction
        assert_eq!(
            block.transactions.len(),
            1,
            "genesis block must have exactly one coinbase transaction"
        );

        // Verify coinbase height is 0
        assert_eq!(
            block.coinbase_height(),
            Some(crate::block::Height(0)),
            "genesis block coinbase must be at height 0"
        );

        // Verify previous block hash is null
        assert_eq!(
            block.header.previous_block_hash,
            crate::block::Hash([0; 32]),
            "genesis block previous hash must be null"
        );

        // Verify the block hash satisfies the difficulty target.
        let threshold = block
            .header
            .difficulty_threshold
            .to_expanded()
            .expect("ZSA testnet genesis difficulty must decode");
        assert!(
            block.header.hash() <= threshold,
            "ZSA testnet genesis block hash must meet the difficulty target (hash must be <= threshold)"
        );
    }
}
