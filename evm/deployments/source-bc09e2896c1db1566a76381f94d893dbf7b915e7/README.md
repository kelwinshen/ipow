# Code of Tempo's contracts as built from commit bc09e28

Tempo's test network was deployed from commit `bc09e28`. `BetaBaskets.json`
is that build's BetaBaskets. Its vault and parts (`iPoWVaultToken`,
`VaultReceipts`, `VaultReceiptsFactory`, `VaultHomeFactory`, `VaultHome`)
were added on 2026-10-06 from the artifacts compiled from commit `eafa7ac`'s
tree, whose code was still the code deployed there (`scripts/verify-deployments.ts`
matched it), before D141 changed the vault and its receipts.
