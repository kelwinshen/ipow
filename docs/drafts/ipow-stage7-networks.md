# Stage 7: the other networks

**Status: the recommendations below were approved by the owner on
2026-10-02 (D132 to D136 in the spec, section 11.10); N5 and N6 are
measured on their testnets first. Nothing is built yet.** The
questions stage 7 of [`ipow-build-plan.md`](ipow-build-plan.md) raises,
each with a recommendation, for the owner. The protocol's spec is
[`../design/ipow-protocol.md`](../design/ipow-protocol.md); its section 11
(the vault) is written for the pair Ethereum and Solana only, and D117
leaves more networks to this stage.

The networks: Base, Robinhood, Polkadot, Hedera, Hyperliquid, Tempo. All
six are EVM networks, so each runs the Ethereum contracts
(`programmable-network/ethereum/contracts/protocol` and `apps`), with the
changes below.

## N1. The vault across many networks

Today a vault has one peer. Records name Ethereum (1) or Solana (2); an
operator's pair chain is read by exactly those two vaults; a receipt on
Solana is backed by what is locked in the Ethereum vault.

| Option | How it works | Cost |
|---|---|---|
| **A. Pairs (recommended)** | One vault per pair of networks: Ethereum–Solana, Ethereum–Base, Solana–Base, and so on. On Ethereum, a vault for each peer; on Solana, the same program with a configuration per peer. Each pair works exactly as section 11 does today: its own operators' pair chains, its own records, its own receipts. vETH on Base comes from the Ethereum–Base pair and is backed by ETH locked in that pair's vault | 7 vaults on each network for 8 networks (28 pairs). An operator registers per pair and bonds per pair. A token of Base and its receipt exist once per other network, as now |
| B. One chain for all | One message chain per operator, processed by every network's vault; records name any network | Every vault processes every message, also those about other networks: N times the cost, and the slowest network holds up every operator's chain. A message too large for one network stops it everywhere. Section 11 would be rewritten, not extended |

**Recommendation: A.** It keeps section 11 as it is, made generic in its
two network numbers, so what is built and tested stays valid. A lie on
one pair touches only that pair's receipts. The spec gains a short rule:
"a vault is deployed per pair; every rule of section 11 is the pair's".

**Worked example.** Bob holds SOL and wants a BETA on Base made of Base's
ETH, vETH and vSOL:

1. Bob locks 1 SOL in the Solana vault for Base. An operator of the
   Solana–Base pair carries the LOCK record; after 7 days (or at once,
   with an attester) Base's Solana vault issues 1 vSOL on Base.
2. Bob locks 1 ETH in Ethereum's vault for Base. The Ethereum–Base pair
   issues 1 vETH on Base.
3. On Base, Bob mints BETA from Base's own ETH, vETH and vSOL.

Each receipt on Base is backed in its own pair's home vault. The
Solana–Ethereum pair is not involved.

**One naming point.** Base's own coin is ETH bridged by Base, not
Ethereum's ETH. In the vault it is Base's asset 0, and its receipt on
Solana would be "vETH from Base", a different token from "vETH from
Ethereum". A wallet must name them apart (for example vETH and
vETH.base). The vault does not merge them: each is backed in its own
home.

## N2. Network numbers in records

Records give a network in one byte (D131). With pairs, the byte stays.
Recommended numbers, fixed for good:

| 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 |
|---|---|---|---|---|---|---|---|
| Ethereum | Solana | Base | Robinhood | Polkadot | Hedera | Hyperliquid | Tempo |

A vault knows its own number and its peer's from its deployment. The
registration commitment (section 11.3) gains the two numbers, so no
registration of one pair can serve another.

## N3. One source for every EVM network

The old contracts were copied into each network's package, and the
copies drifted (Hyperliquid's routers, Tempo's variants). Recommended:
one Solidity source in `programmable-network/ethereum`, built and tested
once; each network is a deployment setting (its network number, its
coin, its fee oracle; see below). The other packages keep only their
deployment configuration and README. The old copies are removed in stage
8, as planned.

## N4. Base and Robinhood: the cost of posting data to Ethereum

On these rollups a transaction also pays for its data on Ethereum, which
the gas price does not show. The commitment fee (D24: amount of work x
current price x 1.5) would then pay an operator less than it spends.

| Network | What the chain itself reports, on-chain |
|---|---|
| Base (OP Stack) | The `GasPriceOracle` predeploy at `0x4200…000F`: `getL1FeeUpperBound(size)` gives the data fee of a transaction of that size |
| Robinhood (Arbitrum Orbit) | The `ArbGasInfo` precompile at `0x…6C`: `getPricesInWei()` gives the price of a byte of data on the parent chain |

**Recommendation:** the commitment fee adds the data cost of the
transactions of the work, read from the chain's own oracle when the job is
opened, before the 1.5: `(work gas x price + work bytes x data price) x
1.5`. Nobody sets a value (as D24 wants). The work's bytes are fixed by
the protocol, as its gas is. A network without such an oracle reads zero.
The alternative, a fixed multiplier per network set at deployment, is
simpler but goes stale as Ethereum's fees move.

## N5. Hedera (V5)

V5 is open: Hedera's gas price may read as zero to a contract, which
makes the commitment fee zero, and the spec says `iPoWProtocol.sol` must
not be deployed there as it is. Hedera's documentation is not clear on
which units a contract sees (`msg.value`, balances and the gas price, in
8 or 18 decimals).

**Recommendation:** measure on Hedera testnet before deciding: deploy a
small probe contract that records `tx.gasprice`, `msg.value` and a
balance for a known payment. Then either the price is read as on other
networks, or the price is a constant fixed at deployment (Hedera prices
its gas in fixed US dollars, so a constant there does not go stale as it
would elsewhere). If amounts are in 8 decimals inside contracts, every
place that multiplies a record unit by a token unit is checked for it.

## N6. Hyperliquid (HyperEVM)

The old contracts were split into routers and facets because HyperEVM's
blocks held 3M gas. HyperEVM now has two kinds of block: small ones, 2M
gas, about every second, and big ones, 30M gas, about every minute, which
an address chooses for its transactions. (Measured on its testnet on
2026-10-02: small blocks hold 3M, and a big one comes every 60 seconds;
see the spec's Build status.)

**Recommendation:** no split. Deploy through big blocks, and measure the
gas of every call a node or user makes (submitting a message, a proof, a
challenge, a mint of BETA). A call above the small block (3M, measured) goes through big blocks, at
the cost of up to a minute's wait; the node chooses per call. To be
confirmed by measurement on HyperEVM testnet before building.

## N7. Tempo: no native coin

Tempo has no native coin; fees are paid in USD stablecoins, and a
transaction carrying native value is refused. The protocol takes bonds,
escrows, deposits and fees in the network's coin, and the vault's asset 0
is the network's coin.

**Recommendation:** every contract takes "the network's coin" as a
deployment setting: native value on every other network, a token
(PathUSD) on Tempo. One source, not Tempo variants (N3). The vault's asset
0 on Tempo is then PathUSD, and its receipt elsewhere vUSD.tempo. Tempo's
deploy receipts can report the wrong address (see
[`../design/ipow-implementation.md`](../design/ipow-implementation.md),
Tempo section): every address is read back from the network, as stage 8
does everywhere.

## N8. Polkadot

Polkadot Hub runs EVM bytecode as other networks do. Nothing known;
tested on its testnet like the others.

## N9. The node

The node's vault roles work on the pair Ethereum and Solana. With pairs
(N1), a node runs the same roles for each pair it is set for; its
settings list the pairs. Its EVM adapter takes the network number from
the settings instead of assuming Ethereum.

## Order

Polkadot, Base and Robinhood first (no or small changes), then Tempo (the
coin setting), then Hedera and Hyperliquid (measured first). The vault
pairs (N1) and the coin setting (N7) are built once, in the shared
source, before any network.

## Sources

- HyperEVM blocks: [Chainstack, HyperEVM RPC guide 2026](https://chainstack.com/learn/how-to/how-to-get-hyperliquid-rpc-endpoint-defi-2026/), [blocmates, dual-block architecture](https://www.blocmates.com/news-posts/hyperevm-adopts-dual-block-architecture--faster-transactions-bigger-blocks-and-a-bold-new-vision)
- OP Stack data fee: [Optimism, estimating fees](https://docs.optimism.io/app-developers/guides/transactions/estimates), [OP Stack predeploys](https://specs.optimism.io/protocol/predeploys.html)
- Arbitrum data price: [Arbitrum precompiles reference](https://docs.arbitrum.io/build-decentralized-apps/precompiles/reference)
- Hedera units: [Hedera, decimal handling](https://docs.hedera.com/hedera/core-concepts/smart-contracts/understanding-hederas-evm-differences-and-compatibility/for-evm-developers-migrating-to-hedera/decimal-handling-8-vs.-18-decimals), [Hedera, gas and fees](https://docs.hedera.com/hedera/core-concepts/smart-contracts/gas-and-fees)
- Tempo fees: [Tempo, how fees work](https://tempo.xyz/developers/docs/protocol/fees), [Tempo FAQ](https://tempo.xyz/faq/)
