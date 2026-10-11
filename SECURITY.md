# Security and trust model

iPoW is designed so that no one is in control of it: not the people who
built it, not an operator, not an app. Bitcoin is verified on chain, the
rules are enforced by code no key can change (D59), and every role is open
to anyone. This page says what that rests on, and, while iPoW is in
development on test networks, which keys exist for now, what each can do,
and when each goes away. Each claim names the file or record where it can
be checked. Read on 2026-10-11.

## Status

- **Test networks only.** Ethereum Sepolia, Base Sepolia, Arbitrum
  Sepolia, Robinhood Chain testnet, HyperEVM testnet, Tempo testnet,
  Hedera testnet, Polkadot Hub TestNet and Solana devnet. There is no
  mainnet deployment (`docs/design/ipow-protocol.md`, Build status row 8).
  The amounts set at deployment are test amounts (D121).
- **Not audited yet.** No external review of the contracts, programs, node
  or SDK has been done. An audit comes before any mainnet deployment.
- **Bitcoin is real.** The light clients take Bitcoin mainnet blocks, so
  the operator's Bitcoin transactions, and a user's BTC payment to
  Conversion, are real BTC (`node/README.md`, "Running").

## What the design guarantees

- **No key can change the rules (D59).** The EVM contracts have no owner,
  no admin and no upgrade path: the light client
  (`evm/contracts/protocol/iPoWLightClient.sol`), the protocol
  (`evm/contracts/protocol/iPoWProtocol.sol`), the vault
  (`evm/contracts/protocol/iPoWVault.sol`), Conversion
  (`evm/contracts/applications/conversion/Conversion.sol`) and BETA
  (`evm/contracts/applications/beta/BetaBaskets.sol`). What is deployed is
  what runs, for good. The Solana programs follow the same rule on
  mainnet (see "Development keys" below).
- **Bitcoin is verified on chain, not reported.** The light client checks
  each header's proof-of-work and difficulty, and each Merkle proof,
  itself; anyone can add a block that passes (D64). An operator cannot
  make a false proof pass, and a proof on blocks that are not Bitcoin's
  best chain can be challenged by anyone during its lock; a won challenge
  slashes the operator (spec sections 2, 6.3).
- **Every role is open.** Anyone can be an operator by locking a bond
  (D12), a guardian by watching and challenging, an attester by putting up
  collateral, and anyone can register an application (D63). The node that
  runs these roles is in this repository (`node/README.md`).
- **Operators answer with their bond.** An operator that misses a job's
  deadline loses its escrow: 80% to the application and 20% to the
  guardian who reported it (D7, D35, D53; spec section 6). A false vault
  claim is refused by a guardian's objection within its 7-day window
  (D105, D111), and a false record proven on chain costs the operator
  every bond it has there (D109; spec section 11.1).
- **What the vault relies on.** A receipt on one network for an asset
  locked on another rests on the operator's claim and its bond, and is
  safe while at least one honest guardian watches the claims. Anyone can
  be that guardian; the more who run one, the stronger it is.

## Development keys on the test networks

While iPoW is built and tested, a few keys exist on purpose. Each is
listed with what it can do today and when it ends. They belong to the
project owner: the EVM key `0x9784B80BC7b95753f2a2732649F3a1136b4c2BF1`
(the `deployer` of each `evm/deployments/<network>-testnet.json`) and the
Solana key `G8vkKtPzQfm8dTVnFcgFis5LgRHqwCk2bT29731sXU6D`.

| Key | What it can do today | When it ends |
|---|---|---|
| Solana programs' upgrade authority | Replace a devnet program, and so change its rules | Removed once audited, before mainnet: the programs become immutable, like the EVM contracts (D59) |
| Vault genesis key | Seed receipts while a vault starts | Ends by itself on 2026-10-21, or earlier when finalized |
| Test-token minters | Mint the test networks' stand-in tokens, which have no value | Never on mainnet: real assets are used there |

- **Solana upgrade authority.** Every devnet program's upgrade authority
  is `G8vkKtPz…sXU6D`: the light client, the protocol, the vault,
  Conversion and BETA (ids in `solana/Anchor.toml`), and the earlier vault
  program `2e1ZUB5e…VAm7p` that still holds Hedera's pair (check with
  `solana program show <id> -u devnet`). It is kept on devnet so fixes can
  be deployed in place (Build status row 8). The same key initialized the
  light client and each vault pair, which only the upgrade authority may
  do (`solana/programs/protocol/ipow-light-client/src/initialize.rs`,
  `solana/programs/protocol/ipow-vault/src/initialize.rs`). Before mainnet,
  after the audit, the authority is removed, and no one can change the
  programs after that.
- **Vault genesis.** Each vault's receipt side starts with a genesis key
  that may make receipts and issue the receipts of a lock on the other
  network without a claim
  ([`docs/specs/ipow-vault-genesis.md`](docs/specs/ipow-vault-genesis.md),
  G1 and G2), so a new vault has assets to work with from its first day.
  During genesis those receipts are only as good as that key; when genesis
  ends the key has no power, for good.
  - EVM: the genesis key is `0x9784…2BF1`, with `genesisEnd` on
    2026-10-21 between 16:56 and 18:07 UTC, per vault (`genesisEnd` in
    `evm/deployments/<network>-testnet.json`; on chain `genesisKey()`,
    `genesisEnd()` and `genesisOpen()` of each vault's receipts part).
  - Solana: each pair's configuration names `G8vkKtPz…sXU6D` with
    `genesis_end` 2026-10-21 17:29:45 UTC (the `["config", peer]` accounts
    of the vault program).
  - Every genesis receipt is public and checkable. The ledger
    [`evm/deployments/genesis-testnet.json`](evm/deployments/genesis-testnet.json)
    lists 409 receipts made and 470 locks issued, each with the lock that
    backs it, and `sdk/scripts/genesis-check.ts` checks every one against
    its lock from the chains; at the run (commit `479d93e`) it found every
    one backed.
  - Hedera's vault was not redeployed in genesis and has no genesis key.
- **Test tokens.** The mock real-world-asset tokens stand in for real ones
  and have no value. On the EVM networks only the deployer can mint them
  (`minter` in `evm/contracts/testnet/MockRWA.sol`), and it minted a stock
  to Greatwall's faucet; on Solana the mints' authority is Greatwall's
  faucet key (`mintAuthority` in `solana/testnet-rwa.json`). The faucet's
  keys are its own, apart from the deployer's and the operator's.

## Who runs the roles today

The roles are open, and today one node fills them: the project's own, run
with the deployer's keys (`node/run-testnet.sh`, `node/node.testnet.yml`).
It is the operator on the 15 pairs it can serve (all but Tempo–Solana) and
the one guardian known to be watching. That is a matter of who has
started a node so far, not a permission: anyone can run an operator or a
guardian with the node in this repository, and each independent guardian
makes a false claim harder to pass.

- **The fast path on test networks.** Until an operator's bond counts on
  the other network, the node pays locks and burns at once from its own
  stock, never repaid (design section 11.7, the owner's choice for test
  networks). Each receipt it issues stays backed by the asset locked, so
  this costs the operator, not the holders.
- **The operator's Bitcoin key.** The node signs its Bitcoin transactions
  (chain heads, tagged transactions, Conversion payments) with one mainnet
  key (`node/.env.operator-btc`, git-ignored). Bitcoin itself does not
  enforce the protocol's rules; the protocol slashes the operator's bond
  when its Bitcoin transaction is missing or not as required. For a
  Conversion buy, the user pays BTC to an address derived from that key
  (`node/README.md`, "Conversion swaps").

## Known gaps

The design doc lists what is not built or not yet tested in its Build
status section and under "Not decided" and "To verify during the detailed
design" (`docs/design/ipow-protocol.md`), and the genesis spec has its
open questions ([`docs/specs/ipow-vault-genesis.md`](docs/specs/ipow-vault-genesis.md);
its decisions D142 to D147 are not yet in the design doc). Treat anything
listed there as unprotected.

## Reporting a vulnerability

If you find a security issue, please don't open a public GitHub issue for
it. Use GitHub's private security advisory feature on this repository
instead, so it can be addressed before details are public.
