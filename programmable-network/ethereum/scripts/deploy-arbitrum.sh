#!/usr/bin/env bash
# Arbitrum Sepolia as iPoW's network 9 (D141), and the tokens of Greatwall's
# index, on test networks only. Every step reads back what it made and
# skips what exists, so the script can be run again after a failure.
# Run from the repository root:
#
#   bash programmable-network/ethereum/scripts/deploy-arbitrum.sh
#
# Needs: ETH on Arbitrum Sepolia at the deployer (the same deployer as the
# other networks; at least 0.05 ETH), test ETH on Sepolia, Base Sepolia and
# Robinhood testnet, HYPE on HyperEVM testnet with the deployer on big
# blocks, and SOL on devnet for the vault upgrade. Keys and endpoints come
# from each package's git-ignored .env and the Solana CLI's keypair.
set -euo pipefail

# The same as deploy-testnets.sh: D93's height, and the largest swap.
MIN_HEIGHT=965567
MAX_SATS=100000
# The Solana vault's pair account for Arbitrum: ["config", 9].
SOLANA_PAIR_9=5rPkBfeBqcEhUUwo8dzwp19XcK5F9gYRYZ7wseusT2ek

SOL=programmable-network/solana
ETH=programmable-network/ethereum
step() { printf '\n== %s\n' "$*"; }

# A deployment records the commit its contracts were built from, and
# deploy-network.ts refuses uncommitted ones: commit first, before anything
# is sent.
if [ -n "$(git status --porcelain -- "$ETH/contracts")" ]; then
  echo "programmable-network/ethereum/contracts has uncommitted changes: commit them first" >&2
  exit 1
fi

step "1. Build: the EVM contracts, and Solana's vault taking network 9"
(cd "$ETH" && npx hardhat compile)
(cd "$SOL" && anchor build -p ipow_vault --ignore-keys)

step "2. Solana's vault upgraded on devnet, so it can pair with network 9"
VAULT_ID=$(solana-keygen pubkey "$SOL/target/deploy/ipow_vault-keypair.json")
NEW_SO="$SOL/target/deploy/ipow_vault.so"
NEW_SIZE=$(wc -c < "$NEW_SO" | tr -d ' ')
LIVE_SO=$(mktemp)
solana program dump "$VAULT_ID" "$LIVE_SO" --url devnet >/dev/null
# The program on devnet is this build when its first bytes are this file.
if cmp -s -n "$NEW_SIZE" "$NEW_SO" "$LIVE_SO"; then
  echo "the vault on devnet is this build already"
else
  HAVE_SIZE=$(solana program show "$VAULT_ID" --url devnet | awk '/Data Length/ {print $3}')
  case "$HAVE_SIZE" in ''|*[!0-9]*) echo "could not read the vault program's size on devnet" >&2; exit 1 ;; esac
  if [ "$NEW_SIZE" -gt "$HAVE_SIZE" ]; then
    solana program extend "$VAULT_ID" $((NEW_SIZE - HAVE_SIZE)) --url devnet
  fi
  solana program deploy "$NEW_SO" \
    --program-id "$SOL/target/deploy/ipow_vault-keypair.json" \
    --url devnet --max-sign-attempts 50 --use-rpc
fi
rm -f "$LIVE_SO"

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
(cd packages/sdk && pnpm run sync && pnpm run build)
(cd core/node && node scripts/sync-testnet-coins.mjs)

printf '\nDone. Next: in Greatwall, node scripts/sync-rwa.mjs, then pnpm install && pnpm run build.\n'
