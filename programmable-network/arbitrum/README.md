# Arbitrum

iPoW's network 9 (D141 in [`docs/design/ipow-protocol.md`](../../docs/design/ipow-protocol.md)).
Arbitrum runs the same contracts as every EVM network, from
[`programmable-network/ethereum`](../ethereum), with its own settings in
[`deploy/networks.ts`](../ethereum/deploy/networks.ts): the native coin (ETH),
and the `ArbGasInfo` price of posting data to Ethereum in the commitment fee
(D135), as on Robinhood. This package holds only its git-ignored `.env`:

| Variable | What |
|---|---|
| `ARBITRUM_SEPOLIA_RPC_URL` | Arbitrum Sepolia's endpoint |
| `ARBITRUM_SEPOLIA_PRIVATE_KEY` | The deployer, the same address as on the other test networks |

## Deploying

From the repository root, once the deployer holds ETH on Arbitrum Sepolia:

```sh
bash programmable-network/ethereum/scripts/deploy-arbitrum.sh
```

It upgrades Solana devnet's vault to the build that takes network 9,
deploys the protocol here with its vault paired with Solana, sets up
Solana's side of that pair, pairs Arbitrum with Sepolia, Base Sepolia,
Robinhood and HyperEVM, deploys Greatwall's mock RWA tokens, reads
everything back, and updates the SDK and the node's settings. Each step
skips what exists, so it can be run again.

## Addresses (Arbitrum Sepolia, chain 421614)

Deployed on 2026-10-06 from commit `c9a2a60` and read back from the network
(`scripts/verify-deployments.ts`: 30 contracts, all match). The full record,
with each vault's parts and factories, is
[`deployments/arbitrum-testnet.json`](../ethereum/deployments/arbitrum-testnet.json);
the mock RWA tokens are in `deployments/arbitrum-testnet-rwa.json`.

| Contract | Address |
|---|---|
| Light client | `0x53e1291BdAff473694BbbB8DD257f9844e5f9F3c` |
| Price of data (`ArbDataFee`) | `0xF43DF008d31995690C75982937a368545953564A` |
| Protocol | `0x35e564d74B90a3A5bfcA8Dec65b1325C83d2e822` |
| Conversion | `0xB7054E399E31A2cFE181c4fD59C7235562a6d45d` |
| BetaBaskets | `0xE8780640839860F9049132606d18c916956A44A5` |
| Vault with Solana (its pair account `5rPkBfeB…T2ek`) | `0x8b9efF66C7D93816Eadf702cC6cE273e8B2b03e7` |
| Vault with Ethereum | `0xcE962Aa05135638f82eCFC15e3f114EEda4DF823` |
| Vault with Base | `0x0496e48C51E3783F5a059AC82B70F5D398448D3A` |
| Vault with Robinhood | `0x10C9C79C8c46c1f9f07D7A258Bbd42B4ED6f973E` |
| Vault with Hyperliquid | `0x01BE560A22e91201C9414b634f0a61085F84dD08` |
