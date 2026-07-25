# iPoW — Ethereum

Solidity implementation of the iPoW protocol, deployed to Sepolia. See the
[root architecture doc](../../docs/ARCHITECTURE.md) for how a conversion
actually flows; this covers building, testing, and deploying this package.

## Contracts

- `contracts/iPoWV1.sol` — main contract: commit/approve/settle logic,
  header relay, liquidity management.
- `contracts/base/iPoWV1Types.sol` — shared errors, events, structs,
  enums, and constants. `iPoWV1` inherits this, so it all compiles into the
  same deployed contract.
- `contracts/libraries/BitcoinPrimitives.sol` — Bitcoin transaction/header
  parsing and proof-of-work verification, as an internal library.

## Setup

```sh
pnpm install
cp .env.example .env   # fill in SEPOLIA_RPC_URL and SEPOLIA_PRIVATE_KEY
```

## Testing

```sh
pnpm test
```

83 tests across five suites (`test/iPoWV1.Admin.test.ts`,
`test/iPoWV1.CommitValidation.test.ts`, `test/iPoWV1.HeaderRelay.test.ts`,
`test/iPoWV1.Liquidity.test.ts`, `test/BitcoinPrimitives.test.ts`) — the
last one runs against `contracts/test/BitcoinPrimitivesHarness.sol`, a
thin harness that exposes the library's `internal` functions for testing,
since Solidity libraries can't be called directly from outside a contract.

## Deploying

Deployment is managed by Hardhat Ignition (`ignition/modules/IPoWV1.ts`):

```sh
npx hardhat ignition deploy ignition/modules/IPoWV1.ts --network sepolia
```

Ignition tracks deployments by network under
`ignition/deployments/chain-<id>/`; re-running the same module against a
network that already has a tracked deployment reuses it rather than
redeploying. Pass `--reset` to force a fresh deployment instead.

Current Sepolia deployment: `0x88fB10c02Dff4F2f2F87064b5DA97fc31de74e10`.

## Post-deploy configuration

`scripts/configure.ts` is the admin CLI for a deployed contract — it's the
one place to register other iPoW networks, manage liquidity, adjust fees,
or hand off the operator role. It's driven entirely by environment
variables (see the comment block at the top of the file for the full
command list), for example:

```sh
ACTION=status npx hardhat run scripts/configure.ts --network sepolia
ACTION=add-liquidity AMOUNT=0.5 npx hardhat run scripts/configure.ts --network sepolia
```
