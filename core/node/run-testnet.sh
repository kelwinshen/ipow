#!/usr/bin/env bash
# Runs the node on the test networks with node.testnet.yml, loading the keys
# and endpoints from where they already are, so that none is copied: the
# Ethereum package's git-ignored .env, the Solana CLI's keypair, and the
# operator's Bitcoin key in this package's git-ignored .env.operator-btc. Extra
# arguments go to the node (for example --check).
set -euo pipefail
cd "$(dirname "$0")"
set -a
. ../../programmable-network/ethereum/.env
# The other EVM test networks' endpoints and keys: each package's own .env.
for n in base hyperliquid robinhood arbitrum; do
  if [ -f "../../programmable-network/$n/.env" ]; then . "../../programmable-network/$n/.env"; fi
done
set +a
export SOLANA_RPC_URL=https://api.devnet.solana.com
SOLANA_KEY=$(cat "$HOME/.config/solana/id.json")
export SOLANA_KEY
# The operator wallet's Bitcoin key: this package's git-ignored
# .env.operator-btc (the old operator's .env, moved here when that service
# was removed).
BTC_WALLET_KEY=$(grep -E '^OPERATOR_BTC_WALLET_PRIVATE_KEY=' .env.operator-btc | head -1 | cut -d= -f2- | tr -d '"' | tr -d "'")
export BTC_WALLET_KEY
# The tunnel API's key, shared with Greatwall's server: this package's
# git-ignored .env.
# Missing, the node says so itself rather than this script stopping silently.
TUNNEL_API_KEY=$( (grep -E '^TUNNEL_API_KEY=' .env 2>/dev/null || true) | head -1 | cut -d= -f2- | tr -d '"' | tr -d "'")
export TUNNEL_API_KEY
exec cargo run -q -p ipow-node -- --settings node.testnet.yml "$@"
