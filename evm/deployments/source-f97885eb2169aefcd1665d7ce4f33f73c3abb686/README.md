# Code of the genesis vaults as built from commit f97885e

The genesis vault redeploys of 2026-10-07 (`docs/specs/ipow-vault-genesis.md`)
were made from commit `f97885e`: each network's Solana pair by
`scripts/redeploy-solana-vault.ts`, and the EVM-to-EVM pairs by
`scripts/deploy-pair.ts --genesis-days`, together with factories of their
own. These are the full artifacts of the vault-side contracts compiled from
`f97885e`'s tree, copied on 2026-10-09:

- `iPoWVaultNative`, `iPoWVaultToken` (Tempo's)
- `VaultHome`, `VaultReceipts`, `VaultReceipt`
- `VaultHomeFactory`, `VaultReceiptsFactory`

The records name this source for every vault (`vaults[].source`) and for the
networks' factories (`sources`), so `scripts/verify-deployments.ts` checks
them against these files instead of today's build. `VaultReceipt` is not
deployed directly (each `VaultReceipts` makes its own) and is kept with
the rest of that build.

At the time of copying, today's build of these contracts was the same code
byte for byte (`contracts/protocol/` did not change from `f97885e` to the
move to `evm/`, and the move kept its source paths); only the AST ids in
`immutableReferences` differ. These files keep the match once that code
changes.

The EVM-to-EVM pairs were first recorded as `"source": "current"`, as
`deploy-pair.ts` then wrote; their deployment times (2026-10-07 17:36 to
18:12 UTC) fall after `f97885e` (16:38 UTC) and before the next commit, and
`verify-deployments.ts` matched every one with these files.

To rebuild them: `git worktree add /tmp/f97885e f97885e`, then
`npx hardhat compile` in its `programmable-network/ethereum` and compare
`deployedBytecode` and `immutableReferences` of
`artifacts/contracts/protocol/{iPoWVault.sol,iPoWVaultToken.sol,vault/*.sol}/`
with these files.
