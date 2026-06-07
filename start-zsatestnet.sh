#!/usr/bin/env bash
set -euo pipefail

# Start a ZSA testnet zebrad node.
#
# The ZSA testnet has all Network Upgrades active at height 1, a custom genesis
# block, and network magic b"ZSA1". It is isolated from the public testnet.
#
# Usage:
#   ./start-zsatestnet.sh               # debug build, start node
#   ./start-zsatestnet.sh --release     # release build, start node
#   ./start-zsatestnet.sh -- --help     # pass flags to zebrad
#
# Config: zsatestnet.toml
# Data:   ~/.cache/zebra/state/v*/zsatestnet/

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
CONFIG="${SCRIPT_DIR}/zsatestnet.toml"

# Enable NU7 support.
export RUSTFLAGS='--cfg zcash_unstable="nu7"'

# Enable internal miner (Equihash solver) and V6 transaction support.
FEATURES="internal-miner,tx_v6"

# Separate cargo flags from zebrad flags.
CARGO_FLAGS=""
ZEBRAD_FLAGS=""
PASSED_DASH=false
for arg in "$@"; do
    if [ "$PASSED_DASH" = true ]; then
        ZEBRAD_FLAGS="$ZEBRAD_FLAGS $arg"
    elif [ "$arg" = "--" ]; then
        PASSED_DASH=true
    elif [ "$arg" = "--release" ]; then
        CARGO_FLAGS="$CARGO_FLAGS $arg"
    else
        ZEBRAD_FLAGS="$ZEBRAD_FLAGS $arg"
    fi
done

echo "=============================================="
echo "  ZSA Testnet — Zebra Node"
echo "=============================================="
echo ""
echo "  Config:    ${CONFIG}"
echo "  Features:  ${FEATURES}"
echo "  RUSTFLAGS: ${RUSTFLAGS}"
echo "  Network:   ZSATestnet (magic: ZSA1)"
echo "  Genesis:   0845415f37f19496586e7e84a37477..."
echo ""
echo "  All NUs active at height 1."
echo "=============================================="
echo ""

cd "${SCRIPT_DIR}"

# shellcheck disable=SC2086
exec cargo run --features "${FEATURES}" ${CARGO_FLAGS} -- \
  --config "${CONFIG}" \
  start \
  ${ZEBRAD_FLAGS}
