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
`scripts/deploy_viem*.mjs`), not through Hardhat Ignition directly (Ignition
is still used for local/test-network compiles and the test suite itself).

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

## Setup

```sh
pnpm install
cp .env.example .env   # fill in TEMPO_TESTNET_RPC_URL and TEMPO_TESTNET_PRIVATE_KEY
```

## Testing

```sh
pnpm test
```

127 tests: the same Ethereum-package suite (contract source mostly
identical) plus `test/BetaHubPathUSD.test.ts` / `test/iPoWConversionPathUSD.test.ts`,
which stand up a mock PathUSD ERC20 and exercise the same scenarios
against the PathUSD variants specifically — including that the inherited,
native-value-`payable` originals stay present but are permanently
unreachable, not a live footgun.

## Deploying

Real deploys go through the viem-native scripts, never plain
`hardhat ignition deploy` (see the constraints above):

```sh
node scripts/deploy_viem.mjs              # iPoW + BetaVault + MockERC20
node scripts/deploy_viem_betahub.mjs      # BetaHub + HubToken ("iBETA")
node scripts/deploy_betavault_pathusd.mjs        # BetaVaultPathUSD
node scripts/deploy_betahub_pathusd.mjs          # BetaHubPathUSD
node scripts/deploy_ipowconversion_pathusd.mjs # iPoWConversionPathUSD
```

Each script prints the transaction's *reported* address — independently
verify it before using it anywhere (see above).

Chain ID 42431.

## Currently live (Moderato testnet)

| Contract | Address |
| --- | --- |
| `iPoW` (shared header source — Beta *and* Conversion both point at this one instance here, unlike most other networks) | `0x53e1291BdAff473694BbbB8DD257f9844e5f9F3c` |
| `BetaVaultPathUSD` (the working spoke — plain `BetaVault` is unreachable here) | `0x8b9efF66C7D93816Eadf702cC6cE273e8B2b03e7` |
| `BetaHubPathUSD` (the working hub — plain `BetaHub` can never register a party here) | `0x900D54050f9Fe47ca56cC67A281947Da2EeE2cD1` |
| `iPoWConversionPathUSD` | `0xC40003975B6ff70E46999Bac8f3b06a9908Fb333` |

`BetaHub`/`BetaVault` (the plain, native-value originals) are also
deployed here but structurally can't work — nothing can ever register a
party or fund a bond through them, since that always requires a nonzero
`msg.value`. Kept deployed for the historical record, not usable.

## Post-deploy configuration

`scripts/configure.ts` follows the standard 18-decimal weibar convention
for the plain contracts (see `hedera/README.md` if you need the
tinybar-scaling caveat that applies there instead) — irrelevant for the
PathUSD variants, whose amounts are already in PathUSD's own 6-decimal
unit.

```sh
ACTION=status npx hardhat run scripts/configure.ts --network tempoTestnet
```
