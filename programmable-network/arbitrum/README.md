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

## Addresses

Not deployed yet. Once deployed, `deployments/arbitrum-testnet.json` and
`deployments/arbitrum-testnet-rwa.json` in
[`programmable-network/ethereum`](../ethereum/deployments) hold them, read
back from the network.
