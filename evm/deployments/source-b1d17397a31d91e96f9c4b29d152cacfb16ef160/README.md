# Code of Conversion and BetaBaskets as built from commit b1d1739

Moving the sources to `evm/` (`contracts/apps/` became
`contracts/applications/{conversion,beta}/`) changed the source files of
`Conversion` and `BetaBaskets`: their paths, `Conversion`'s import lines
(`../protocol/` became `../../protocol/`) and comments in both (the docs
they cite moved to `docs/specs/`). None of it changes what the code does,
but all of it is in the solc metadata hash at the end of their bytecode, and
so their bytes changed.
Their deployments on Sepolia, Base Sepolia, HyperEVM, Robinhood and
Arbitrum were made before the move. These are the full artifacts of the two
contracts compiled from commit `b1d1739`'s tree (the last before the move),
copied on 2026-10-09; `scripts/verify-deployments.ts` matched every one of
those deployments with them. Those networks' records name this source for
the two (`sources`).

Arbitrum's two were deployed from `c9a2a60`; that commit's build of them
is the same code, byte for byte (`Conversion.sol` and `BetaBaskets.sol` did
not change between the two commits), so its record names this one too.

To rebuild them: `git worktree add /tmp/b1d1739 b1d1739`, then
`npx hardhat compile` in its `programmable-network/ethereum` and compare
`deployedBytecode` and `immutableReferences` of
`artifacts/contracts/apps/{Conversion,BetaBaskets}.sol/` with these files.
