# Remaining Work: sync-zcash-v5.0.0-merge-v2

Branch: `sync-zcash-v5.0.0-merge-v2`
Base: `origin/sync-zcash-v4.3.0-merge` + `v5.0.0`

## What's Done

- [x] Nu6_2 network upgrade (enum, activation heights, consensus branch ID, protocol version, all match arms)
- [x] Temporary Orchard-disable soft fork infrastructure
- [x] 3-verifier architecture (PRE_NU6_2 / POST_NU6_2 / ZSA + `verifier_for` dispatch)
- [x] librustzcash patches updated to zsa-nu6.2 tip (rev `5d312c9`, includes Nu6_2)
- [x] v5.0.0 block security fixes (MAX_BLOCK_LOCATOR_LENGTH, try_fold value_balance)
- [x] Dependency version bumps for non-Zcash crates (sentry, toml, opentelemetry, etc.)
- [x] `cargo check --features tx_v6` passes

---

## 1. Fork Dependency Upgrades (Blocks Some Below)

The ZSA forks need to be rebased onto the v5.0.0-level upstream tags before the Cargo.toml versions can be bumped and the post-NU6.2 verifier can use the fixed-circuit key.

### 1.1 hhanh00/orchard: 0.13.1 → 0.14.0

- **Current**: `hhanh00/orchard` rev `7750deb` (branch `vote_v2`, orchard 0.13.1)
- **Target**: orchard 0.14.0 + ZSA patches (`OrchardZSA` flavor, `zsa-issuance`, `temporary-zebra`, generic `ShieldedData<Flavor>`)
- **Partial work**: `/tmp/orchard-zsa` has a `zsa-0.14.0` branch with the merge done but not pushed. Has halo2_gadgets version mismatch (needs 0.4→0.5 or keeping 0.4).
- **Impact**: Once done, can bump `orchard` workspace dep to 0.14 and switch `VERIFIER_POST_NU6_2` to use the fixed-circuit key (`VerifyingKey::build()` instead of `build::<OrchardVanilla>()`).

### 1.2 hhanh00/librustzcash: 0.27.0 → 0.28.0

- **Current**: `hhanh00/librustzcash` rev `5d312c9` (branch `zsa-nu6.2`, zcash_primitives 0.27.0)
- **Target**: zcash_primitives 0.28.0 + ZSA patches (`OrchardBundle` enum, `zsa-issuance`/`zip-233` features)
- **Partial work**: `/tmp/librustzcash-zsa` has a `zsa-0.28.0` branch with the merge done but uncompiled (conflict markers removed, Cargo.lock regenerated, but compilation not verified).
- **Impact**: Once done, can bump all librustzcash crate workspace deps to v5.0.0 versions. Upstream 0.28 removed the `OrchardBundle` enum — the ZSA fork must re-add it adapted to orchard 0.14's API.

### 1.3 QED-it/halo2

- **Status**: No version gap (same `0.3` in both v4.3.0 and v5.0.0). But the fork is at `0.4.0`-level APIs while Cargo.toml specifies `0.5`. The orchard fork's Cargo.toml was reverted to `0.4` for local compilation.
- **Impact**: If orchard moves to halo2_gadgets 0.5, the QED-it/halo2 fork may need updating too.

---

## 2. Cargo.toml Version Bumps (Blocked by #1)

Once the forks are at the right versions, bump these workspace dependency versions in the root `Cargo.toml`:

```
orchard:            0.13 → 0.14
zcash_primitives:   0.27 → 0.28
zcash_protocol:     0.8  → 0.9
zcash_address:      0.11 → 0.12
zcash_keys:         0.13 → 0.14
zcash_proofs:       0.27 → 0.28
zcash_transparent:  0.7  → 0.8
equihash:           0.2.2 → 0.3
```

And add new v5.0.0 workspace dependencies: `http-body`, `anyhow`, `strum`, `strum_macros`.

The `[patch.crates-io]` section must point to the newly rebased fork revs. Patch version declarations must match what the fork repos actually export.

---

## 3. VERIFIER_POST_NU6_2 — Fixed Circuit Key (Blocked by #1.1)

Currently `VERIFIER_POST_NU6_2` uses the same key as `VERIFIER_PRE_NU6_2` (both `build::<OrchardVanilla>()`). There's a TODO in `zebra-consensus/src/primitives/halo2.rs:71`.

Once the orchard fork supports it, change to use the NU6.2 fixed-circuit key:
```rust
pub static ref VERIFYING_KEY_POST_NU6_2: ItemVerifyingKey =
    ItemVerifyingKey::build();  // Fixed-circuit key (not build::<OrchardVanilla>())
```

---

## 4. v5.0.0 Source Improvements (Not Yet Carried Forward)

Many general improvements from v5.0.0 were not applied. The v5.0.0 diff vs v4.3.0 merge base touches ~117 Rust files. Only 3 were updated. The remaining fall into these categories:

### 4.1 Security Fixes (High Priority)
- `zebra-chain/src/block/header.rs` — preallocation cap for `CountedHeader`
- `zebra-chain/src/serialization.rs` — preallocation fixes, `MAX_HEADERS_PER_MESSAGE`
- `zebra-chain/src/serialization/zcash_deserialize.rs` — upfront Vec reservation cap
- `zebra-consensus/src/script.rs` — input/output alignment validation before script verification
- `zebra-rpc` — 4 security advisories fixed (details in v5.0.0 changelog)

### 4.2 Network Layer
- `zebra-network/src/peer_set/` — new stall tracker
- `zebra-network/src/protocol/external/types.rs` — Nu6_2 protocol version (already done)
- `zebra-network/src/config.rs` — various improvements
- Various handshake, connection, address book changes

### 4.3 RPC Layer
- `zebra-rpc/src/methods/types/transaction.rs` — `new_coinbase()` replacing `from_coinbase()`, uses zcash_primitives Builder API
- `zebra-rpc/src/methods/types/get_block_template/` — precomputed coinbase support
- `zebra-rpc/src/methods.rs` — new/updated RPC methods

### 4.4 State Service
- `zebra-state/src/service.rs` — chain tip, error handling changes
- `zebra-state/src/service/write.rs` — `non_finalized_rejected_sender` channel (already in our tree from v5.0.0 auto-merge)
- `zebra-state/src/service/queued_blocks.rs` — improvements

### 4.5 Consensus
- `zebra-consensus/src/block.rs` — various checks
- `zebra-consensus/src/transaction.rs` — V4/V5 validation updates

### 4.6 General
- Checkpoint updates (mainnet and testnet checkpoints)
- `orchard_note_commitments()` return type change (`pallas::Base` → `&pallas::Base`)
- Documentation, changelogs, supply-chain audits

---

## 5. Test Compilation

Library code compiles. Tests have pre-existing errors from the v4.3.0 merge:

| File | Issue |
|------|-------|
| `zebra-rpc/src/methods.rs` | `MinerAddressType`, `fetch_chain_info`, `MinerParams` imports removed |
| `zebra-rpc/src/methods/types/transaction.rs` | `from_coinbase` → `new_coinbase` API change |
| `zebra-rpc/src/methods/types/default_roots.rs` | `from_coinbase` not found |
| `zebra-rpc/tests/serialization_tests.rs` | `With` helper removed |
| `zebrad/tests/acceptance.rs` | `n_tx` method missing |
| `zebrad/tests/common/coinbase.rs` | `MinerAddressType` not found |
| `zebrad/tests/common/config.rs` | `with` method not found |
| `zebra-chain/src/block.rs` | `BLOCK_HASH_SIZE` dead code warning |
| `zebra-chain/src/block/tests/preallocate.rs` | `MAX_PROTOCOL_MESSAGE_LEN` missing |
| `zebra-consensus/src/transaction.rs` | Unused import `OrchardBundle` |

These are mostly from v5.0.0 API changes in RPC and test utilities that weren't applied to test code.

---

## 6. Orchard Note Commitments Return Type

Upstream v5.0.0 changed `orchard_note_commitments()` from returning `impl Iterator<Item = pallas::Base>` to `impl Iterator<Item = &pallas::Base>`. This change cascades through:
- `zebra-chain/src/block.rs` (not applied)
- `zebra-chain/src/transaction.rs` (not applied)
- `zebra-chain/src/parallel/tree.rs` (caller)
- `zebra-chain/src/block/arbitrary.rs` (caller)
- `zebra-state/src/service/finalized_state/disk_format/...` (caller)

Decision needed: apply the return type change or keep the ZSA-compatible version.

---

## 7. ZSA-Specific v5.0.0 Changes

Some v5.0.0 changes touch files that also have ZSA modifications. These need careful merging:
- `zebra-chain/src/orchard/shielded_data.rs` — v5.0.0 changed, ZSA added V6 variants
- `zebra-chain/src/orchard/action.rs` — v5.0.0 changed, ZSA has flavor-generic types
- `zebra-chain/src/orchard/commitment.rs` — v5.0.0 changed, ZSA has ZSA commitments
- `zebra-chain/src/transaction.rs` — v5.0.0 changed heavily, ZSA has V6 tx
- `zebra-chain/src/transaction/serialize.rs` — v5.0.0 changed, ZSA has V6 serialization
- `zebra-chain/src/primitives/zcash_primitives.rs` — transaction data mapping

---

## Summary of Blockers

```
Fork upgrades (#1)
  ├── Blocks: Cargo version bumps (#2)
  ├── Blocks: POST_NU6_2 fixed-circuit key (#3)
  └── May affect: Source improvements (#4), Return type change (#6), ZSA merges (#7)
```

The fork upgrades are the critical path. Everything else can be done incrementally after.
