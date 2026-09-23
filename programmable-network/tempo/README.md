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
  `iPoWV1ConversionPathUSD.sol` convert every bond/fee/payout that would
  otherwise use `msg.value` to move PathUSD instead — the
  judging/verification/auction logic itself is byte-identical to the
  reference contracts (`BetaVault.sol`/`BetaHub.sol`/
  `iPoWV1Conversion.sol`), untouched.
- **A confirmed, unfixed chain-level bug**: a deploy transaction's own
  receipt can report the wrong `contractAddress` (contract-creation
  addresses ignore the nonce *lane* Tempo's own 2D-nonce model uses,
  deriving only from the raw nonce number, which can collide with an
  earlier create). **Never trust a Tempo deploy/write receipt here** —
  every deploy in this package was verified by independently deriving
  candidate `CREATE` addresses for nearby nonces and reading real
  contract state back, not by trusting what the transaction reported.
  See [DESIGN_V2.md §8.15](../../docs/DESIGN_V2.md) for the full story.

Plain Hardhat/ethers can't speak Tempo's fee-sponsored, two-dimensional-
nonce transaction model at all — every deploy/write against real Tempo in
this package goes through `viem`'s native Tempo support instead (see
`scripts/deploy_viem*.mjs`), not through Hardhat Ignition directly (Ignition
is still used for local/test-network compiles and the test suite itself).

See the [root architecture doc](../../docs/ARCHITECTURE.md) for how a
conversion flows, and [DESIGN_V2.md §8](../../docs/DESIGN_V2.md) /
[§9](../../docs/DESIGN_V2.md) for BETA's composition design and this
package's own PathUSD variants specifically.

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
identical) plus `test/BetaHubPathUSD.test.ts` / `test/iPoWV1ConversionPathUSD.test.ts`,
which stand up a mock PathUSD ERC20 and exercise the same scenarios
against the PathUSD variants specifically — including that the inherited,
native-value-`payable` originals stay present but are permanently
unreachable, not a live footgun.

## Deploying

Real deploys go through the viem-native scripts, never plain
`hardhat ignition deploy` (see the constraints above):

```sh
node scripts/deploy_viem.mjs              # iPoWV1 + BetaVault + MockERC20
node scripts/deploy_viem_betahub.mjs      # BetaHub + HubToken ("iBETA")
node scripts/deploy_betavault_pathusd.mjs        # BetaVaultPathUSD
node scripts/deploy_betahub_pathusd.mjs          # BetaHubPathUSD
node scripts/deploy_ipowv1conversion_pathusd.mjs # iPoWV1ConversionPathUSD
```

Each script prints the transaction's *reported* address — independently
verify it before using it anywhere (see above).

Chain ID 42431.

## Currently live (Moderato testnet)

| Contract | Address |
| --- | --- |
| `iPoWV1` (shared header source — Beta *and* Conversion both point at this one instance here, unlike most other networks) | `0x53e1291BdAff473694BbbB8DD257f9844e5f9F3c` |
| `BetaVaultPathUSD` (the working spoke — plain `BetaVault` is unreachable here) | `0x8b9efF66C7D93816Eadf702cC6cE273e8B2b03e7` |
| `BetaHubPathUSD` (the working hub — plain `BetaHub` can never register a party here) | `0x900D54050f9Fe47ca56cC67A281947Da2EeE2cD1` |
| `iPoWV1ConversionPathUSD` | `0xC40003975B6ff70E46999Bac8f3b06a9908Fb333` |

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
