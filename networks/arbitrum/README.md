# Arbitrum

iPoW's network 9 (D141 in [`docs/design/ipow-protocol.md`](../../docs/design/ipow-protocol.md)).
Arbitrum runs the same contracts as every EVM network, from
[`evm`](../../evm), with its own settings in
[`deploy/networks.ts`](../../evm/deploy/networks.ts): the native coin (ETH),
and the `ArbGasInfo` price of posting data to Ethereum in the commitment fee
(D135), as on Robinhood. This package holds only its git-ignored `.env`:

| Variable | What |
|---|---|
| `ARBITRUM_SEPOLIA_RPC_URL` | Arbitrum Sepolia's endpoint |
| `ARBITRUM_SEPOLIA_PRIVATE_KEY` | The deployer, the same address as on the other test networks |

## Deploying

From the repository root, once the deployer holds ETH on Arbitrum Sepolia:

```sh
bash evm/scripts/deploy-arbitrum.sh
```

It checks that Solana devnet's vault is its recorded deployment, which
takes network 9 (`solana/deployments/devnet.json`; upgraded on 2026-10-06,
since replaced by the genesis vault program), deploys the protocol here
with its vault paired with Solana, sets up Solana's side of that pair,
pairs Arbitrum with Sepolia, Base Sepolia, Robinhood and HyperEVM, deploys
Greatwall's mock RWA tokens, reads everything back, and updates the SDK and
the node's settings. Each step skips what exists, so it can be run again.

## Addresses (Arbitrum Sepolia, chain 421614)

Deployed on 2026-10-06 from commit `c9a2a60`; its vaults were redeployed in
genesis on 2026-10-07 from commit `f97885e`
([`docs/specs/ipow-vault-genesis.md`](../../docs/specs/ipow-vault-genesis.md)),
replacing those of 2026-10-06 (kept in
[`evm/deployments/replaced/`](../../evm/deployments/replaced/)). Read back
from the network (`scripts/verify-deployments.ts`: 32 contracts, all match).
The full record, with each vault's parts and factories, is
[`deployments/arbitrum-testnet.json`](../../evm/deployments/arbitrum-testnet.json);
the mock RWA tokens are in `deployments/arbitrum-testnet-rwa.json`.

| Contract | Address |
|---|---|
| Light client | `0x53e1291BdAff473694BbbB8DD257f9844e5f9F3c` |
| Price of data (`ArbDataFee`) | `0xF43DF008d31995690C75982937a368545953564A` |
| Protocol | `0x35e564d74B90a3A5bfcA8Dec65b1325C83d2e822` |
| Conversion | `0xB7054E399E31A2cFE181c4fD59C7235562a6d45d` |
| BetaBaskets | `0xE8780640839860F9049132606d18c916956A44A5` |
| Vault with Solana (its pair account `AizTkFPMioKpZEMxQ1fsBBbjTi9U8KHmwXX32Z6pBwsh`, `["config", 9]` of the vault program `3TZ1LJ4f…`) | `0x170685feEe5ac2bCddAE20DAe66747658E6fA7c0` |
| Vault with Ethereum (Sepolia) | `0x3221102aC4159048960e061FC0ADb81De3133496` |
| Vault with Base (Sepolia) | `0x51cD31AC42a838d225f3B42F830Af72d99Eb003a` |
| Vault with Robinhood (testnet) | `0x7063AA65e06d6d5759997c50c0134c1aF20be59D` |
| Vault with Hyperliquid (HyperEVM testnet) | `0xDa8964472dd82C3a2d222aC352BF377ed364d33D` |

**Genesis.** These vaults' receipts started in genesis for the deployer,
ending between 2026-10-21 16:59 and 2026-10-21 18:07 UTC at the latest
(`genesisEnd` in the record). `sdk/scripts/genesis-check.ts`, run at commit
`2897c06` (then `packages/sdk/scripts/genesis-check.ts`) with the genesis
run's ledger
([`evm/deployments/genesis-testnet.json`](../../evm/deployments/genesis-testnet.json)),
read every genesis receipt and issue back from the chains: 409 receipts and
470 issues across all the pairs, every one backed by its lock. Of these, 66
of the receipts and 76 of the issues are on this network's receipts parts,
and 70 of the locks issued are here.