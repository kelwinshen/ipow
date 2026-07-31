# iPoW — Hedera

Same Solidity contracts as [`programmable-network/ethereum`](../ethereum),
deployed to Hedera Testnet. Hedera's Smart Contract Service is reached
through the Hashio JSON-RPC relay, so from Hardhat's perspective this is
just another standard EVM network — no chain-specific contract changes were
needed. See the [root architecture doc](../../docs/ARCHITECTURE.md) for how
a conversion actually flows.

## Setup

```sh
pnpm install
cp .env.example .env   # fill in HEDERA_TESTNET_RPC_URL and HEDERA_TESTNET_PRIVATE_KEY
```

## Testing

```sh
pnpm test
```

Same 83-test suite as the Ethereum package (the contract source is
identical) — see that package's README for the breakdown.

## Deploying

```sh
npx hardhat ignition deploy ignition/modules/IPoWV1.ts --network hederaTestnet
```

Chain ID 296. Current Hedera Testnet deployment:
`0x36D7F82F8B2E800C877592F8DFFF0E8CFAc96CF3`.

## Post-deploy configuration

`scripts/configure.ts` works the same way as the Ethereum package's, with
one real difference worth knowing about: on Hedera, `msg.value` as seen
*inside* executing contract code is scaled to HBAR's native 8-decimal
tinybar unit, not the usual 18-decimal weibar convention that Hashio
presents everywhere else (e.g. `eth_getBalance`). The script accounts for
this when reading/writing `nativeLiquidity`, but it's worth knowing about if
you're calling the contract directly rather than through the script.

```sh
ACTION=status npx hardhat run scripts/configure.ts --network hederaTestnet
```
