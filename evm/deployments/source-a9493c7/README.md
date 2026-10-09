# Code of the vault parts as built from commit a9493c7

The six test networks' vaults (Sepolia, Base Sepolia, Robinhood, Polkadot,
Hedera, HyperEVM; 2026-10-02) were deployed from commit `a9493c7`, before
the vault's parts were made in their own transactions. These files are the
`deployedBytecode` and `immutableReferences` of the five contracts that
change afterwards (`iPoWVaultNative`, `VaultHomeFactory`,
`VaultReceiptsFactory`, `VaultHome`, `VaultReceipts`), copied on 2026-10-03
from the artifacts compiled from that commit's clean tree (`git status
contracts` empty, `git log -1 -- contracts` = a9493c7), before any change.
`scripts/verify-deployments.ts` compares those deployments with them.

`Conversion.json` was added on 2026-10-03 the same way, when Conversion
changed (the tunnel's T1 and T2): the networks whose Conversion was not
redeployed (Hedera, Polkadot, Tempo) still run this one; Tempo's, deployed
from `10e5cf5`, is the same code (its deployment names this build for it).
`Conversion.sol` is the same at `a9493c7` as at `bb803a4`.

To rebuild them: `git worktree add /tmp/a9493c7 a9493c7`, then
`npx hardhat compile` in its `programmable-network/ethereum` and compare
these fields with its artifacts.
