#!/usr/bin/env bash
set -euo pipefail

DIR="$(cd "$(dirname "$0")" && pwd)"

echo "==> Killing any existing zebrad..."
pkill -f "zebrad" 2>/dev/null || true
sleep 1

echo "==> Cleaning old state..."
rm -rf /tmp/zebrad-nu7-test

echo "==> Building and starting zebrad with NU7 + internal miner..."
cd "$DIR"
exec env RUSTFLAGS='--cfg zcash_unstable="nu7"' \
  cargo run --package zebrad --features tx_v6,internal-miner -- \
  -c "$DIR/nu7-test.toml" start
