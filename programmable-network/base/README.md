# iPoW — Base

Same Solidity contracts as [`programmable-network/ethereum`](../ethereum),
deployed to Base testnet. Base Sepolia is Coinbase's standard OP-Stack L2 testnet — no chain-specific contract changes needed, a plain EVM network from Hardhat's side. See the
[root architecture doc](../../docs/design/ipow-implementation.md) for how a conversion
actually flows, and
[design/ipow-implementation.md §8](../../docs/design/ipow-implementation.md) for BETA's composition design.

## New protocol: test network deployment

The new protocol ([`docs/design/ipow-protocol.md`](../../docs/design/ipow-protocol.md)),
deployed on Base Sepolia (chain 84532) on 2026-10-02 from the one source in
[`../ethereum`](../ethereum) with this network's settings
([`../ethereum/deploy/networks.ts`](../ethereum/deploy/networks.ts)), and read back:
each contract's code compared with the build and each setting read
(`node scripts/verify-deployments.ts` in `../ethereum`). Lowest Bitcoin height
965,567; Conversion's largest swap 100,000 sats. The vault's
pair on Solana is the account `Gyhrq1CzHyU9BfECK5G8RW4ho75oaRPqMngXYYvLuduL`.

| Contract | Address |
|---|---|
| Light client (`iPoWLightClient`) | `0x10C9C79C8c46c1f9f07D7A258Bbd42B4ED6f973E` |
| Protocol | `0x2Dd456fCe7B3574AbD76b2899d3106CaB6ff5b9B` |
| Reader of the data price (D135) | `0x15390C4901e11D3732489825c2FBa33A2E3e5b37` |
| Conversion | `0x6c5d75F830C002f4B73b4625F3E2bFaeC50b5D2E` |
| BETA (`BetaBaskets`) | `0xf35A1C4A9dE7B7FffF62c7cEfda2ac91a71cB6cc` |
| Vault home factory | `0x01BE560A22e91201C9414b634f0a61085F84dD08` |
| Vault receipts factory | `0x4265A27605cDd45b69d36f17635Bad03351387e6` |
| Vault paired with Solana | `0x4cFD981522F31A4dc12Aeb433907ebc3CBD37A55` |
| Its home part | `0xA1C38ee95A97d8F8355313c3De4D1C914B04693c` |
| Its receipts part | `0xD0bE0340C8cB132D8AF1765Af0d7358Af434984c` |

## Setup

```sh
pnpm install
cp .env.example .env   # fill in BASE_SEPOLIA_RPC_URL and BASE_SEPOLIA_PRIVATE_KEY
```

## Testing

```sh
pnpm test
```

Same test suite as the Ethereum package (the contract source is
identical) — see that package's README for the breakdown.

## Deploying

```sh
npx hardhat ignition deploy ignition/modules/IPoW.ts --network baseSepolia
```

Chain ID 84532. Current deployment: `0xB7054E399E31A2cFE181c4fD59C7235562a6d45d`.

### BetaVault (design/ipow-implementation.md §6/§8)

```sh
npx hardhat ignition deploy ignition/modules/BetaVault.ts --network baseSepolia \
  --parameters '{"BetaVaultModule":{"ipowHeaders":"<iPoW address above>"}}'
```

Current deployment: `0x6354779b4Dbb564c712ea91c179eCF521C15BE73` (plus a
`MockERC20` test token at `0xE8780640839860F9049132606d18c916956A44A5`,
deployed alongside it purely to exercise §8.13's ERC20 local-leg
support — not part of the protocol itself).

### BetaHub (design/ipow-implementation.md §8.18/§8.19)

```sh
npx hardhat ignition deploy ignition/modules/BetaHub.ts --network baseSepolia
```

Current deployment: `0x24765955eCbffAaACB48a8c3747975d6C750C075`
(`HubToken`/"iBETA" alongside it).

### iPoWConversion (design/ipow-implementation.md §1–§2/§9.2)

The permissionless-auction Conversion base primitive — its own dedicated
`iPoW` header source, deliberately separate from Beta's above (same
split every other network uses). No operator automation needed — this
one is permissionless by design.

```sh
npx hardhat ignition deploy ignition/modules/IPoWConversion.ts --network baseSepolia
```

Current deployment: `0xa43f67C41Fc6e8474278d5a7C1540862CC2a16bA`
(its own `iPoW`: `0x5F63BF3638f8E2BcBD56a4E43cEC7D39637A6f80`).
Cross-registered with every other network's own `iPoWConversion` via
`addNetwork` — see design/ipow-implementation.md §9.2.

## Post-deploy configuration

`scripts/configure.ts` follows the standard 18-decimal weibar convention
(see `hedera/README.md` if you need the tinybar-scaling caveat that
applies there instead).

```sh
ACTION=status npx hardhat run scripts/configure.ts --network baseSepolia
```
