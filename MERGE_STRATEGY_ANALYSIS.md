# Merge Strategy Analysis — ZSA Sync Branches

## Current Approach

The team uses a **strictly forward-only, layered merge** strategy with explicit merge commits:

```
zsa1 ──► integration-state ──► v4.1.0-merge ──► v4.2.0-merge ──► v4.3.0-merge
                                      ▲                  ▲                  ▲
                                 zcash-v4.1.0       zcash-v4.2.0       zcash-v4.3.0
```

Changes flow rightward. Upstream Zcash releases are merged in at each sync branch level. Back-merges (right to left) are rare or absent.

## Observed Consequences

### 1. One-way ratchet
Once changes leave `zsa1`, upstream syncs never return. `zsa1` is still at the v4.1.0 sync level — v4.2.0 and v4.3.0 upstream changes were never merged back into the main branch.

### 2. Stranded commits
`zsa1` has **670 commits** that never reached `zsa-integration-state` or any sync branch (the last merge of `zsa1` → `zsa-integration-state` was at `794f1b500`). These include:
- ZSA logic fixes (NU6/NU7, lockbox disbursements, issuance, burn)
- CI/CD improvements (ECR pipelines, test fixes, lint config)
- Dependency upgrades
- Bug fixes and review feedback

### 3. Mid-point forking
New sync branches fork from mid-history of the previous sync branch, not from HEAD:
- `v4.3.0-merge` forked from `v4.2.0-merge` at `7314dffda` — *before* `zsa1` was merged into `v4.2.0-merge`
- Result: fixes merged into `v4.2.0-merge` were never forward-ported to `v4.3.0-merge`

### 4. Divergent branches with no reconciliation
Today there are three active lines, none of which contain each other:
- `zsa1` — stable/deploy, at v4.1.0 sync level, has CI/CD fixes
- `v4.2.0-merge` — has v4.2.0 upstream + zsa1 CI/CD, but not v4.3.0
- `v4.3.0-merge` — has v4.3.0 upstream, but missing the CI/CD fixes from zsa1

## Impact

| | zsa1 | v4.2.0-merge | v4.3.0-merge |
|---|---|---|---|
| Core ZSA logic | ✅ | ✅ | ✅ |
| Upstream v4.2.0 | ❌ | ✅ | ✅ |
| Upstream v4.3.0 | ❌ | ❌ | ✅ |
| Latest CI/CD fixes | ✅ | ✅ | ❌ |
| Latest ZSA fixes (670 commits) | ✅ | ❌ | ❌ |

## Questions for the Team

1. Should upstream syncs be merged back into `zsa1`? If not, what is the plan for getting `zsa1` past v4.1.0?
2. Should new sync branches always branch from the HEAD of the previous sync branch (not mid-points)?
3. Is there a single "source of truth" branch, or is the multi-branch model intentional?
4. Would a periodic "sync-back" merge help prevent drift?
