# iPoW — Polkadot

Same Solidity contracts as [`programmable-network/ethereum`](../ethereum),
deployed to Polkadot Hub TestNet. Polkadot Hub's REVM backend runs standard,
unmodified EVM bytecode behind an Ethereum-JSON-RPC-compatible adapter
(`pallet-revive`'s `eth-rpc`), so this is a plain EVM network from Hardhat's
side — deliberately not using `@parity/hardhat-polkadot`, since that plugin
targets PVM/RISC-V via the `resolc` compiler and only supports Hardhat 2.x,
not the Hardhat 3 setup used here. See the
[root architecture doc](../../docs/design/ipow-implementation.md) for how a conversion
actually flows.

## New protocol: test network deployment

The new protocol ([`docs/design/ipow-protocol.md`](../../docs/design/ipow-protocol.md)),
deployed on Polkadot testnet (chain 420420417) on 2026-10-02 from the one source in
[`../ethereum`](../ethereum) with this network's settings
([`../ethereum/deploy/networks.ts`](../ethereum/deploy/networks.ts)), and read back:
each contract's code compared with the build and each setting read
(`node scripts/verify-deployments.ts` in `../ethereum`). Lowest Bitcoin height
965,567; Conversion's largest swap 100,000 sats. The vault's
pair on Solana is the account `BcTMgkGcknJu9GW6XQ7HoRdXdP3Jo6iRt8ZjKe6fm7r7`.

| Contract | Address |
|---|---|
| Light client (`iPoWLightClient`) | `0x2E2b4eFf60659AFc08ae85E8A124a515008AEb9f` |
| Protocol | `0x5D38c6Bec531A3290f121C39C1f88231fa4B49b9` |
| Conversion | `0x3221102aC4159048960e061FC0ADb81De3133496` |
| BETA (`BetaBaskets`) | `0xB001c1Eb3B2D6dF4380682E119dDF1e3a9F18D89` |
| Vault home factory | `0x2ab2c9487cC8f83816d556Da149132c705B82e48` |
| Vault receipts factory | `0x895584abec0b2971A97c5De927BBE46cFAb9A618` |
| Vault paired with Solana | `0x77ef03207173b08A82d6511a35f970033E185d6c` |
| Its home part | `0xC69EBc691174a55E240552674896bBE651ebBad2` |
| Its receipts part | `0x6708f6882d9a0F0335aDAbe944c8436936679435` |

## Setup and deploying

This package holds only this README, `.env.example`, and the `.gitignore`
that keeps the network's `.env` out of git
(`cp .env.example .env`, then fill in `POLKADOT_TESTNET_RPC_URL` and `POLKADOT_TESTNET_PRIVATE_KEY`). The
contracts, their tests and the deployment scripts are the one source in
[`../ethereum`](../ethereum); see its README for deploying.

The old contract generation that was deployed here (`iPoW`,
`iPoWConversion`, `BetaHub`, `BetaVault`), with its Hardhat package, was
removed; it stays in git at the tag `legacy-v1`.
