#!/usr/bin/env bash
# Arbitrum Sepolia as iPoW's network 9 (D141), and the tokens of Greatwall's
# index, on test networks only. Every step reads back what it made and
# skips what exists, so the script can be run again after a failure.
# Run from the repository root:
#
#   bash evm/scripts/deploy-arbitrum.sh
#
# Needs: ETH on Arbitrum Sepolia at the deployer (the same deployer as the
# other networks; at least 0.05 ETH), test ETH on Sepolia, Base Sepolia and
# Robinhood testnet, HYPE on HyperEVM testnet with the deployer on big
# blocks, and SOL on devnet for Solana's side of the pair. Keys and
# endpoints come from each package's git-ignored .env and the Solana CLI's
# keypair.
set -euo pipefail

# The same as deploy-testnets.sh: D93's height, and the largest swap.
MIN_HEIGHT=965567
MAX_SATS=100000
# The Solana vault's pair account for Arbitrum: ["config", 9] of the vault
# program 3TZ1LJ4fVZVKbBUyzVMRjJ9FNPGaqVmGX56JuT5QdXEy (the genesis vault
# program; arbitrum-testnet.json's solanaPairAccount).
SOLANA_PAIR_9=AizTkFPMioKpZEMxQ1fsBBbjTi9U8KHmwXX32Z6pBwsh

SOL=solana
ETH=evm
step() { printf '\n== %s\n' "$*"; }

# A deployment records the commit its contracts were built from, and
# deploy-network.ts refuses uncommitted ones: commit first, before anything
# is sent.
if [ -n "$(git status --porcelain -- "$ETH/contracts")" ]; then
  echo "evm/contracts has uncommitted changes: commit them first" >&2
  exit 1
fi

step "1. Build: the EVM contracts"
(cd "$ETH" && npx hardhat compile)

step "2. Solana's vault on devnet takes network 9: the recorded deployment"
# Upgraded to the build that takes 9 on 2026-10-06, and since replaced by
# the genesis vault program (Anchor.toml), which takes it too. Not upgraded
# from here: a fresh build is other bytes (it embeds its source paths), and
# target/deploy's vault keypair is the retired program's. The program on
# devnet must be its record (solana/deployments/devnet.json).
(cd "$SOL" && bash scripts/verify-devnet.sh ipow_vault)

step "3. The protocol on Arbitrum Sepolia, its vault paired with Solana"
if [ -f "$ETH/deployments/arbitrum-testnet.json" ]; then
  echo "arbitrum already deployed"
else
  (cd "$ETH" && node scripts/deploy-network.ts arbitrum testnet "$MIN_HEIGHT" "$MAX_SATS" "$SOLANA_PAIR_9")
fi

step "4. Solana's side of the pair with Arbitrum (skips the pairs set up)"
(cd "$SOL" && node scripts/init-testnet.ts)

step "5. Arbitrum paired with each EVM network (one vault each side)"
has_pair() { node -e 'const d=require("./'"$ETH"'/deployments/'"$1"'-testnet.json"); process.exit(d.vaults.some(v=>v.peer===9)?0:1)'; }
for peer in ethereum base robinhood hyperliquid; do
  if has_pair "$peer"; then echo "arbitrum-$peer: paired already"; continue; fi
  extra=""
  [ "$peer" = hyperliquid ] && extra="--big-blocks"
  # Arbitrum's vault is predicted from the deployer's nonce there: send
  # nothing else from it on Arbitrum while this runs.
  (cd "$ETH" && node scripts/deploy-pair.ts "$peer" arbitrum $extra)
done

step "6. Tokens: Arbitrum's set, and the index's parts elsewhere, each registered with every pair's vault"
(cd "$ETH" && node scripts/deploy-mock-rwa.ts arbitrum)
(cd "$ETH" && node scripts/deploy-mock-rwa.ts ethereum)
(cd "$ETH" && node scripts/deploy-mock-rwa.ts base)
(cd "$ETH" && node scripts/deploy-mock-rwa.ts robinhood)
(cd "$ETH" && node scripts/deploy-mock-rwa.ts hyperliquid --big-blocks)

step "7. Everything read back against its build"
(cd "$ETH" && node scripts/verify-deployments.ts)

step "8. The SDK's deployments, and the node's settings"
(cd sdk && pnpm run sync && pnpm run build)
(cd node && node scripts/sync-testnet-coins.mjs)

printf '\nDone. Next: in Greatwall, node scripts/sync-rwa.mjs, then pnpm install && pnpm run build.\n'
