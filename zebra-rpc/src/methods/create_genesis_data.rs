//! `create_genesis_data` RPC method.
//!
//! Creates a genesis block (including Equihash solution when built with `internal-miner`)
//! based on user-provided parameters such as network magic number, name, and activation heights.

use std::collections::BTreeMap;

use zebra_chain::{
    block::{self, merkle, Block, Header, Hash},
    parameters::NetworkUpgrade,
    serialization::ZcashSerialize,
    transaction::{Transaction, UnminedTx, LockTime},
    transparent::{self, Script, GENESIS_COINBASE_DATA},
    work::{
        difficulty::CompactDifficulty,
        equihash::Solution,
    },
};

/// Returns the transaction effecting and authorizing roots
/// for `coinbase_txn`, which are used in the block header.
fn calculate_transaction_roots(
    coinbase_txn: &UnminedTx,
) -> (merkle::Root, merkle::AuthDataRoot) {
    let block_transactions = || std::iter::once(coinbase_txn);

    let merkle_root = block_transactions().cloned().collect();
    let auth_data_root = block_transactions().cloned().collect();

    (merkle_root, auth_data_root)
}

/// Parameters for the `create_genesis_data` RPC.
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct CreateGenesisParams {
    /// Name for the testnet (e.g., "ZSA1", "MyTestnet").
    pub network_name: String,

    /// Network magic number as a 4-byte hex string (e.g., "5B9E2850").
    pub network_magic: String,

    /// Activation heights for network upgrades.
    /// Keys are network upgrade names (e.g., "NU5", "NU6"), values are block heights.
    /// If omitted, defaults to `{ "NU6": 1 }`.
    #[serde(default = "default_activation_heights")]
    pub activation_heights: BTreeMap<String, u32>,

    /// If true, skip Equihash mining and use a placeholder zero solution.
    /// This is useful when the binary is not compiled with `internal-miner` support.
    /// Default: `false` (attempt to mine).
    #[serde(default)]
    pub disable_pow: bool,

    /// Optional Unix timestamp for the genesis block.
    /// If omitted, the current time is used.
    #[serde(default)]
    pub genesis_time: Option<u64>,
}

fn default_activation_heights() -> BTreeMap<String, u32> {
    let mut heights = BTreeMap::new();
    heights.insert("NU6".to_string(), 1);
    heights
}

/// Response from `create_genesis_data`.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct CreateGenesisResponse {
    /// The complete genesis block serialized as hex.
    pub genesis_block_hex: String,

    /// The genesis block hash (double-SHA256 of the 80-byte header in reverse byte order).
    pub genesis_hash: String,

    /// The 140-byte header preimage (pre-nonce + nonce), for external Equihash mining.
    pub header_preimage_hex: String,

    /// The Equihash solution as hex (empty if `disable_pow` was true).
    pub equihash_solution_hex: String,

    /// The nonce used for mining (always zero if `disable_pow` was true).
    pub nonce_hex: String,

    /// A ready-to-use `[network.testnet_parameters]` TOML config section
    /// that can be dropped into a `zebrad.toml`.
    pub config_toml: String,
}

/// The standard Zcash genesis P2PK script (used in Mainnet, Testnet, Regtest genesis blocks).
///
/// This is: `OP_PUSHDATA(65) ++ <uncompressed pubkey> ++ OP_CHECKSIG`
/// The pubkey is 0x04678afdb0... (the Bitcoin genesis block pubkey).
const GENESIS_P2PK_SCRIPT_BYTES: [u8; 67] = [
    0x41,
    0x04, 0x67, 0x8a, 0xfd, 0xb0, 0xfe, 0x55, 0x48,
    0x27, 0x19, 0x67, 0xf1, 0xa6, 0x71, 0x30, 0xb7,
    0x10, 0x5c, 0xd6, 0xa8, 0x28, 0xe0, 0x39, 0x09,
    0xa6, 0x79, 0x62, 0xe0, 0xea, 0x1f, 0x61, 0xde,
    0xb6, 0x49, 0xf6, 0xbc, 0x3f, 0x4c, 0xef, 0x38,
    0xc4, 0xf3, 0x55, 0x04, 0xe5, 0x1e, 0xc1, 0x12,
    0xde, 0x5c, 0x38, 0x4d, 0xf7, 0xba, 0x0b, 0x8d,
    0x57, 0x8a, 0x4c, 0x70, 0x2b, 0x6b, 0xf1, 0x1d,
    0x5f,
    0xac,
];

/// The standard genesis difficulty bits (PowLimit).
const GENESIS_BITS: u32 = 0x2007ffff;

/// Build a genesis block based on the provided parameters.
///
/// This is a standalone function (not tied to the RPC server state)
/// so it can be called from any context.
#[allow(clippy::too_many_arguments)]
pub fn create_genesis_block(
    network_name: &str,
    network_magic_hex: &str,
    activation_heights: &BTreeMap<String, u32>,
    disable_pow: bool,
    genesis_time: Option<u64>,
) -> std::result::Result<CreateGenesisResponse, String> {
    // --- Parse network magic ---
    let magic_bytes = hex::decode(network_magic_hex).map_err(|e| {
        format!("invalid network_magic hex: {e}")
    })?;
    if magic_bytes.len() != 4 {
        return Err(
            "network_magic must be exactly 4 bytes (8 hex chars)".to_string()
        );
    }
    let magic_arr = [magic_bytes[0], magic_bytes[1], magic_bytes[2], magic_bytes[3]];

    // --- Parse activation heights ---
    let mut heights = BTreeMap::new();
    for (name, height) in activation_heights {
        let nu = match name.to_uppercase().as_str() {
            "NU5" => NetworkUpgrade::Nu5,
            "NU6" => NetworkUpgrade::Nu6,
            "NU6.1" | "NU6_1" => NetworkUpgrade::Nu6_1,
            "NU7" => NetworkUpgrade::Nu7,
            other => {
                return Err(format!(
                    "unsupported network upgrade: {other}"
                ));
            }
        };
        heights.insert(block::Height(*height), nu);
    }

    // Default: enable NU6 at height 1 (like ZSA1 testnet)
    if heights.is_empty() {
        heights.insert(block::Height(1), NetworkUpgrade::Nu6);
    }

    // --- Build coinbase transaction ---
    // Uses the standard Zcash genesis coinbase data.
    let coinbase_input = transparent::Input::new_coinbase(
        block::Height(0),
        GENESIS_COINBASE_DATA.to_vec(),
        None, // use default sequence
    );

    // Build the P2PK script for the genesis output.
    let p2pk_script = Script::new(&GENESIS_P2PK_SCRIPT_BYTES);

    // Genesis coinbase output: value 0 with P2PK script.
    let coinbase_output = transparent::Output::new(
        zebra_chain::amount::Amount::zero(),
        p2pk_script,
    );

    let coinbase_tx = Transaction::V1 {
        inputs: vec![coinbase_input],
        outputs: vec![coinbase_output],
        lock_time: LockTime::unlocked(),
    };

    // --- Compute merkle root from coinbase ---
    let (merkle_root, _auth_data_root) = calculate_transaction_roots(
        &UnminedTx::from(coinbase_tx.clone()),
    );

    // --- Build block header ---
    let genesis_time = match genesis_time {
        Some(ts) => chrono::DateTime::from_timestamp(ts as i64, 0)
            .expect("valid genesis timestamp"),
        None => chrono::Utc::now(),
    };

    // Use the standard genesis difficulty (PowLimit).
    let genesis_bits_be = GENESIS_BITS.to_be_bytes();
    let difficulty_threshold = CompactDifficulty::from_bytes_in_display_order(&genesis_bits_be)
        .expect("standard genesis difficulty is always valid");

    // The commitment bytes: all zeros for genesis (no prior chain history).
    let commitment_arr: [u8; 32] = [0; 32];
    let nonce_arr: [u8; 32] = [0; 32];

    #[cfg_attr(not(feature = "internal-miner"), allow(unused_mut))]
    let mut header = Header {
        version: block::ZCASH_BLOCK_VERSION,
        previous_block_hash: Hash([0; 32]),
        merkle_root,
        commitment_bytes: commitment_arr.into(),
        time: genesis_time,
        difficulty_threshold,
        nonce: nonce_arr.into(),
        solution: Solution::for_proposal(),
    };

    // --- Mine Equihash solution ---
    let equihash_solution_hex;
    let final_nonce_hex;

    #[cfg(feature = "internal-miner")]
    if !disable_pow {
        // Attempt to mine a real Equihash solution.
        let solve_result = Solution::solve_genesis(header, || Ok(()))
            .map_err(|e| format!("equihash solver error: {e}"))?;

        let solved_headers = solve_result.to_vec();
        if let Some(solved_header) = solved_headers.into_iter().next() {
            header = solved_header;
            equihash_solution_hex = match &header.solution {
                Solution::Common(sol) => hex::encode(sol),
                Solution::Regtest(sol) => hex::encode(sol),
            };
            final_nonce_hex = hex::encode(&header.nonce[..]);
        } else {
            equihash_solution_hex = String::new();
            final_nonce_hex = hex::encode(&[0u8; 32]);
        }
    } else {
        equihash_solution_hex = String::new();
        final_nonce_hex = hex::encode(&[0u8; 32]);
    }

    #[cfg(not(feature = "internal-miner"))]
    {
        let _ = disable_pow;
        equihash_solution_hex = String::new();
        final_nonce_hex = hex::encode(&[0u8; 32]);
    }

    // --- Build the 140-byte header preimage for external mining ---
    // Preimage format: version(4) + prev_hash(32) + merkle(32) + commitment(32)
    //                + time(4) + bits(4) + nonce(32) = 140 bytes
    let mut header_preimage = Vec::with_capacity(140);
    header_preimage.extend_from_slice(&header.version.to_le_bytes());
    header_preimage.extend_from_slice(&header.previous_block_hash.0);
    header_preimage.extend_from_slice(&header.merkle_root.0);
    header_preimage.extend_from_slice(&header.commitment_bytes[..]);
    header_preimage.extend_from_slice(&(header.time.timestamp() as u32).to_le_bytes());
    // Use the known genesis bits value since CompactDifficulty.0 is private
    header_preimage.extend_from_slice(&GENESIS_BITS.to_le_bytes());
    header_preimage.extend_from_slice(&header.nonce[..]);

    // --- Assemble the complete genesis block ---
    let block = Block {
        header: std::sync::Arc::new(header),
        transactions: vec![std::sync::Arc::new(coinbase_tx)],
    };

    let genesis_block_bytes = block
        .zcash_serialize_to_vec()
        .expect("block serialization should not fail");
    let genesis_block_hex = hex::encode(&genesis_block_bytes);

    // Compute genesis hash (double-SHA256 of 80-byte header, reversed for display)
    let genesis_hash = block.hash();

    // --- Build the zebrad.toml config section ---
    let network_name_lower = network_name.to_lowercase().replace(' ', "_");
    let magic_array = format!(
        "[0x{:02X}, 0x{:02X}, 0x{:02X}, 0x{:02X}]",
        magic_arr[0], magic_arr[1], magic_arr[2], magic_arr[3]
    );

    let activation_str = activation_heights
        .iter()
        .map(|(name, height)| format!("{} = {}", name.to_uppercase(), height))
        .collect::<Vec<_>>()
        .join("\n");

    let config_toml = format!(
        "[network.testnet_parameters]\n\
         slow_start_interval = 0\n\
         network_magic = {magic_array}\n\
         network_name = \"{network_name}\"\n\
         genesis_hash = \"{genesis_hash}\"\n\
         no_peers_required = true\n\n\
         [network.testnet_parameters.activation_heights]\n\
         {activation_str}\n\n\
         [consensus]\n\
         checkpoint_sync = false\n\n\
         [network]\n\
         cache_dir = true\n\
         listen_addr = \"[::]:18233\"\n\
         network = \"Testnet\"\n\
         initial_testnet_peers = []\n\n\
         [rpc]\n\
         listen_addr = \"127.0.0.1:18232\"\n\n\
         [state]\n\
         cache_dir = \"./testnet_{network_name_lower}\"\n\
         delete_old_database = true\n\
         ephemeral = false\n\n\
         [mining]\n\
         internal_miner = true\n\
         miner_address = \"t27eWDgjFYn9aV3M3kHVkRMdJ7kJ1PWFZyk\"\n\n\
         [mempool]\n\
         debug_enable_at_height = 0\n"
    );

    Ok(CreateGenesisResponse {
        genesis_block_hex,
        genesis_hash: genesis_hash.to_string(),
        header_preimage_hex: hex::encode(&header_preimage),
        equihash_solution_hex,
        nonce_hex: final_nonce_hex,
        config_toml,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    #[ignore = "requires internal-miner feature and takes ~30s"]
    fn create_zsa_testnet_1_genesis_mined() {
        let mut heights = BTreeMap::new();
        heights.insert("NU6_1".to_string(), 1);

        // ZSA1 as hex bytes: Z=0x5A, S=0x53, A=0x41, 1=0x31
        // Timestamp matches regular testnet genesis: 2026-05-15 09:09:00 UTC
        let result = create_genesis_block(
            "ZSA_Testnet_1",
            "5A534131",
            &heights,
            false, // mine a real equihash solution
            Some(1778836140), // fixed timestamp matching regular testnet
        )
        .expect("genesis block creation should succeed");

        println!("=== Genesis Block ===");
        println!("Name:           ZSA Testnet 1");
        println!("Magic:          5A534131 (ZSA1)");
        println!("Network Upgrade: NU6.1 @ height 1");
        println!();
        println!("Genesis Hash:   {}", result.genesis_hash);
        println!();
        println!("Header Preimage (140 bytes):");
        println!("{}", result.header_preimage_hex);
        println!();
        println!("Equihash solution hex (first 80 chars):");
        println!("{}", &result.equihash_solution_hex[..80.min(result.equihash_solution_hex.len())]);
        println!("Nonce hex:      {}", result.nonce_hex);
        println!();

        // Save genesis block to file
        let output_dir = "testnet_zsa1_output";
        std::fs::create_dir_all(output_dir).expect("create output dir");
        std::fs::write(
            format!("{output_dir}/genesis_block.hex"),
            &result.genesis_block_hex,
        )
        .expect("write genesis block");
        std::fs::write(
            format!("{output_dir}/genesis_hash.txt"),
            &result.genesis_hash,
        )
        .expect("write genesis hash");
        std::fs::write(
            format!("{output_dir}/zebrad.toml"),
            &result.config_toml,
        )
        .expect("write zebrad config");
        println!("Saved genesis block to {output_dir}/");
        println!();
        println!("=== zebrad.toml config ===");
        println!("{}", result.config_toml);
    }

    #[test]
    fn create_zsa_testnet_1_genesis_no_pow() {
        let mut heights = BTreeMap::new();
        heights.insert("NU6".to_string(), 1);

        // ZSA1 as hex bytes: Z=0x5A, S=0x53, A=0x41, 1=0x31
        let result = create_genesis_block(
            "ZSA_Testnet_1",
            "5A534131",
            &heights,
            true, // disable_pow
            Some(1778836140),
        )
        .expect("genesis block creation should succeed");

        assert!(!result.genesis_hash.is_empty());
        assert!(result.equihash_solution_hex.is_empty());
        println!("Genesis Hash (no PoW): {}", result.genesis_hash);
    }
}
