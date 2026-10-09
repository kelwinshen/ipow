# Security and trust model

A direct statement of what is true today, not of the end state. Each
claim names the file or record where it can be checked. Read on
2026-10-09.

## Status

- **Test networks only.** Ethereum Sepolia, Base Sepolia, Arbitrum
  Sepolia, Robinhood Chain testnet, HyperEVM testnet, Tempo testnet,
  Hedera testnet, Polkadot Hub TestNet and Solana devnet. There is no
  mainnet deployment (`docs/design/ipow-protocol.md`, Build status row 8).
  The amounts set at deployment are test amounts (D121); only Ethereum's
  and Solana's mainnet amounts are decided (D118, D121).
- **Not audited.** No external review of the contracts, programs, node or
  SDK has been done.
- The light clients take Bitcoin mainnet blocks, so the operator's Bitcoin
  transactions are real, with real (small) amounts of BTC
  (`node/README.md`, "Running").

## What the code enforces with no one in control

- **The EVM contracts have no owner and no key**, and nothing in them can
  be changed after deployment (D59): the light client
  (`evm/contracts/protocol/iPoWLightClient.sol`), the protocol
  (`evm/contracts/protocol/iPoWProtocol.sol`), the vault
  (`evm/contracts/protocol/iPoWVault.sol`), Conversion
  (`evm/contracts/applications/conversion/Conversion.sol`) and BETA
  (`evm/contracts/applications/beta/BetaBaskets.sol`). The one exception is
  the vault's genesis key, below.
- **Bitcoin is verified on chain.** The light client checks each header's
  proof-of-work and difficulty, and each Merkle proof, itself; anyone can
  add a block that passes (D64). An operator cannot make a false proof
  pass; a proof on blocks that are not Bitcoin's best chain can be
  challenged by anyone (a guardian) during its lock, and a won challenge
  slashes the operator (spec sections 2, 6.3).
- **Anyone can be an operator** by locking a bond (D12), and anyone can
  register an application (D63). An operator that misses a job's deadline
  loses that job's escrow: 80% to the application, the value it asked to
  be covered, and 20% to the guardian who reported it (D7, D35, D53; spec
  section 6).
- **Vault claims are not trustless.** A receipt on one network for an
  asset locked on another rests on the operator's claim and its bond. It
  is safe while at least one honest guardian watches and objects to a
  false claim within its 7-day window (D105, D111; spec section 11.1).
  Nothing on chain makes it safe without that watcher.

## What the deployer still controls

The deployer is one person (the project owner) holding the EVM key
`0x9784B80BC7b95753f2a2732649F3a1136b4c2BF1` (the `deployer` of each
`evm/deployments/<network>-testnet.json`) and the Solana key
`G8vkKtPzQfm8dTVnFcgFis5LgRHqwCk2bT29731sXU6D`.

- **The Solana programs can be replaced.** The upgrade authority of every
  program on devnet is `G8vkKtPz…sXU6D`: the light client, the protocol,
  the vault, Conversion and BETA (ids in `solana/Anchor.toml`), and the
  earlier vault program `2e1ZUB5e…VAm7p` that still holds Hedera's pair
  (check with `solana program show <id> -u devnet`). Whoever holds it can change any
  rule of those programs, and so any balance they hold. It is kept on
  devnet for fixes and removed on mainnet (D59; Build status row 8). The
  same key initialized the light client and each vault pair, which only
  the upgrade authority may do
  (`solana/programs/protocol/ipow-light-client/src/initialize.rs`,
  `solana/programs/protocol/ipow-vault/src/initialize.rs`).
- **The vaults are in genesis until 2026-10-21.** Each vault's receipt
  side has a genesis key that may, without a claim, make a receipt and
  issue the receipts of a lock on the other network
  ([`docs/specs/ipow-vault-genesis.md`](docs/specs/ipow-vault-genesis.md),
  G1 and G2). During genesis those receipts are only as good as that key:
  it could issue a lock that does not exist, and nothing on chain stops it.
  Genesis ends when the key finalizes it or at `genesisEnd`, after which
  the key has no power.
  - EVM: the genesis key is the deployer, `0x9784…2BF1`, with `genesisEnd`
    on 2026-10-21 between 16:56 and 18:07 UTC, per vault (`genesisEnd` in
    `evm/deployments/<network>-testnet.json`; on chain `genesisKey()`,
    `genesisEnd()` and `genesisOpen()` of each vault's receipts part).
  - Solana: each pair's configuration names `G8vkKtPz…sXU6D` with
    `genesis_end` 2026-10-21 17:29:45 UTC (the `["config", peer]` accounts
    of the vault program).
  - Read on 2026-10-09: `genesisOpen()` true on every vault of Sepolia,
    Base Sepolia and Arbitrum Sepolia, and `genesis_done` false on every
    Solana pair. The other networks' vaults were not read.
  - Hedera's vault was not redeployed in genesis and has no genesis key;
    it is paired with the earlier Solana vault program
    `2e1ZUB5eqA6bQfH3f9pbfvibfie7WnvEJeKif53VAm7p` (`solana/README.md`).
  - The genesis run's ledger is
    [`evm/deployments/genesis-testnet.json`](evm/deployments/genesis-testnet.json):
    409 receipts made and 470 locks issued, each with the lock that backs
    it. Anyone can check every genesis receipt against its lock from the
    chains with `sdk/scripts/genesis-check.ts`; at the run (commit
    `479d93e`, where it was `packages/sdk/scripts/genesis-check.ts` and the
    ledger `programmable-network/ethereum/deployments/genesis-testnet.json`)
    it found every one backed. Each paired network's README
    states the same result with that network's share of it.
- **There is one operator.** Anyone may bond and take jobs, but today the
  only operator is the deployer's own node, with the deployer's keys
  (`node/run-testnet.sh`, `node/node.testnet.yml`; the first live job,
  [`docs/drafts/ipow-testnet-run-2026-10-02.md`](docs/drafts/ipow-testnet-run-2026-10-02.md),
  "One key, both sides"). The same node is the only known guardian. Its
  vault roles are not switched on (`vault` is commented out in
  `node/node.testnet.yml`), so no one is known to watch vault claims.
- **The operator's Bitcoin side is one key.** The node signs every
  Bitcoin transaction (chain heads, tagged transactions, Conversion
  payments) with one mainnet WIF key, loaded from the git-ignored
  `node/.env.operator-btc` (`node/run-testnet.sh`). Bitcoin does not
  enforce the protocol's rules; the protocol can only slash the operator's
  bond on the programmable network when its Bitcoin transaction is missing
  or not as required. For a Conversion buy, the user pays BTC to an
  address derived from that key (`node/README.md`, "Conversion swaps").
- **The mock RWA tokens are the deployer's.** On the EVM networks only the
  deployer can mint them (`minter` in `evm/contracts/testnet/MockRWA.sol`);
  on Solana a separate mint-authority key held by Greatwall's faucet
  (`solana/scripts/deploy-mock-rwa.ts`). They have no value and exist only
  on test networks.

## Known gaps

The design doc lists what is not built or not yet tested in its Build
status section and under "Not decided" and "To verify during the
detailed design" (`docs/design/ipow-protocol.md`), and the genesis spec's
open questions ([`docs/specs/ipow-vault-genesis.md`](docs/specs/ipow-vault-genesis.md);
its decisions D142 to D147 are not yet in the design doc). Treat anything
listed there as unprotected.

## Reporting a vulnerability

If you find a security issue, please don't open a public GitHub issue for
it. Use GitHub's private security advisory feature on this repository
instead, so it can be addressed before details are public.
