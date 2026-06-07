//! Consensus parameters for the ZSA testnet.
//!
//! The ZSA testnet is a persistent test network for Zcash Shielded Assets (ZSA) testing.
//! All Network Upgrades activate at height 1, providing a clean-slate chain where
//! all modern Zcash protocol features are active from the start.

use crate::parameters::network::testnet::ConfiguredActivationHeights;

/// The network name for the ZSA testnet, used by the `Display` impl and for
/// state database path isolation.
pub const ZSA_TESTNET_NETWORK_NAME: &str = "ZSATestnet";

/// Returns the [`ConfiguredActivationHeights`] for the ZSA testnet, with all
/// Network Upgrades activating at height 1. Genesis stays at height 0.
pub(crate) fn zsa_testnet_activation_heights() -> ConfiguredActivationHeights {
    ConfiguredActivationHeights {
        before_overwinter: Some(1),
        overwinter: Some(1),
        sapling: Some(1),
        blossom: Some(1),
        heartwood: Some(1),
        canopy: Some(1),
        nu5: Some(1),
        nu6: Some(1),
        nu6_1: Some(1),
        nu6_2: Some(1),
        nu7: Some(1),
        #[cfg(zcash_unstable = "zfuture")]
        zfuture: None,
    }
}

/// The block hash of the ZSA Testnet genesis block (TBD placeholder).
///
/// Replace with the actual genesis block hash once the block is constructed/mined.
pub const ZSA_TESTNET_GENESIS_HASH: &str =
    "03536e258babad397d59ce2995d9015d0791ac5272b8348e8f07f94143a72bb7";
