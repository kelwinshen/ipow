# iPoW — Tempo

Mostly the same Solidity contracts as
[`programmable-network/ethereum`](../ethereum), deployed to Tempo testnet
("Moderato") — a Stripe/Paradigm-incubated L1 purpose-built for stablecoin
payments, EVM-compatible but a genuinely different kind of network than a
general-purpose L2. Two real chain-level constraints forced Tempo-only
contract variants, not just config differences:

- **Tempo rejects any transaction carrying native value outright** — a
  hard, confirmed chain-level rule (`"Revm error: value transfer in
  Tempo Transaction not allowed"`), not a gas or nonce quirk. Every real
  value movement (fees included) goes through Tempo's real settlement
  ERC20, **PathUSD** (`0x20c0000000000000000000000000000000000000`, 6
  decimals). `BetaVaultPathUSD.sol`, `BetaHubPathUSD.sol`, and
  `iPoWConversionPathUSD.sol` convert every bond/fee/payout that would
  otherwise use `msg.value` to move PathUSD instead — the
  judging/verification/auction logic itself is byte-identical to the
  reference contracts (`BetaVault.sol`/`BetaHub.sol`/
  `iPoWConversion.sol`), untouched.
- **A confirmed, unfixed chain-level bug**: a deploy transaction's own
  receipt can report the wrong `contractAddress` (contract-creation
  addresses ignore the nonce *lane* Tempo's own 2D-nonce model uses,
  deriving only from the raw nonce number, which can collide with an
  earlier create). **Never trust a Tempo deploy/write receipt here** —
  every deploy in this package was verified by independently deriving
  candidate `CREATE` addresses for nearby nonces and reading real
  contract state back, not by trusting what the transaction reported.
  See [design/ipow-implementation.md §8.15](../../docs/design/ipow-implementation.md) for the full story.

Plain Hardhat/ethers can't speak Tempo's fee-sponsored, two-dimensional-
nonce transaction model at all — every deploy/write against real Tempo in
this package goes through `viem`'s native Tempo support instead (see
`scripts/`).

See the [root architecture doc](../../docs/design/ipow-implementation.md) for how a
conversion flows, and [design/ipow-implementation.md §8](../../docs/design/ipow-implementation.md) /
[§9](../../docs/design/ipow-implementation.md) for BETA's composition design and this
package's own PathUSD variants specifically.

## New protocol: test network deployment

The new protocol ([`docs/design/ipow-protocol.md`](../../docs/design/ipow-protocol.md)),
deployed on Tempo's Moderato testnet (chain 42431) on 2026-10-03 from the one
source in [`../ethereum`](../ethereum), commit `10e5cf5`, with Tempo's settings
([`../ethereum/deploy/networks.ts`](../ethereum/deploy/networks.ts)): the token builds,
with PathUSD (`0x20C0000000000000000000000000000000000000`) as the coin. Sent with
viem's Tempo support by [`scripts/deploy-new-protocol.ts`](scripts/deploy-new-protocol.ts)
in the deployer's ordinary nonce lane, each address worked out from the nonce, and
read back: each contract's code compared with the build and each setting read
(`node scripts/verify-deployments.ts` in `../ethereum`). Lowest Bitcoin height
965,567; Conversion's largest swap 100,000 sats. The vault's pair
on Solana is the account `7wvWJdeTW9WynaP17Le7dnWcBkV7qpVz78sR9TrA977Q`.

| Contract | Address |
|---|---|
| Light client (`iPoWLightClient`) | `0x2dD223DcD7F69539Ea895A29095c69c16b088aDb` |
| Protocol (token build, `iPoWProtocolToken`) | `0x0496e48C51E3783F5a059AC82B70F5D398448D3A` |
| Conversion | `0xc729b1a6d0325ae559614b6826127e11E51703c1` |
| BETA (`BetaBaskets`) | `0x6AA1F2dd1a0A28F5FC3a88ec8A219e89987A57aB` |
| Vault home factory | `0xd6b425c7908E171a33dF2a4e6C5687eDF0D2d6c3` |
| Vault receipts factory | `0xb856906fEBAFBB21A06DdC35E9BCe476139A86BA` |
| Vault paired with Solana (`iPoWVaultToken`) | `0x10C9C79C8c46c1f9f07D7A258Bbd42B4ED6f973E` |
| Its home part | `0xCC87ec659D54802077b6004901A1A72C646bF07A` |
| Its receipts part | `0x6a4E25f0506C9cddfB1a469c9ED93633edC3eee0` |

## Setup and deploying

The contracts and their tests are the one source in
[`../ethereum`](../ethereum). This package holds this README, the network's
git-ignored `.env` (`cp .env.example .env`, then fill in
`TEMPO_TESTNET_RPC_URL` and `TEMPO_TESTNET_PRIVATE_KEY`), and Tempo's own
deployment scripts, which send Tempo's transaction type with `viem`
(`pnpm install` here for `viem` and `ethers`; compile in `../ethereum`
first). Each script's header comment says how to run it:

- [`scripts/deploy-new-protocol.ts`](scripts/deploy-new-protocol.ts) — the protocol, and vaults paired with Solana.
- [`scripts/deploy-mock-rwa.ts`](scripts/deploy-mock-rwa.ts) — Greatwall's mock RWA tokens.
- [`scripts/genesis-tempo.ts`](scripts/genesis-tempo.ts) — Tempo's part of the vaults' genesis.

Chain ID 42431. Never trust a Tempo deploy receipt's address (above); the
scripts work it out from the nonce and read the code back.

The old contract generation that was deployed here (`iPoW`, `BetaVault`,
`BetaHub` and their PathUSD variants, `iPoWConversionPathUSD`), with its
Hardhat package and `scripts/deploy_*.mjs`, was removed; it stays in git at
the tag `legacy-v1`.
