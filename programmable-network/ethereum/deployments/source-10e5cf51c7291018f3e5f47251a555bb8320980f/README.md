# Code of Tempo's contracts as built from commit 10e5cf5

Tempo's test network was deployed from commit `10e5cf5`. `BetaBaskets.json`
is that build's BetaBaskets. Its vault and parts (`iPoWVaultToken`,
`VaultReceipts`, `VaultReceiptsFactory`, `VaultHomeFactory`, `VaultHome`)
were added on 2026-10-06 from the artifacts compiled from commit `63668a5`'s
tree, whose code was still the code deployed there (`scripts/verify-deployments.ts`
matched it), before D141 changed the vault and its receipts.
