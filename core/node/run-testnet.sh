#!/usr/bin/env bash
# Runs the node on the test networks with node.testnet.yml, loading the keys
# and endpoints from where they already are, so that none is copied: the
# Ethereum package's git-ignored .env, the Solana CLI's keypair, and the
# operator's Bitcoin key in core/operator/.env. Extra
# arguments go to the node (for example --check).
set -euo pipefail
cd "$(dirname "$0")"
set -a
. ../../programmable-network/ethereum/.env
set +a
export SOLANA_RPC_URL=https://api.devnet.solana.com
SOLANA_KEY=$(cat "$HOME/.config/solana/id.json")
export SOLANA_KEY
# The operator wallet's Bitcoin key, read from the old operator's .env.
BTC_WALLET_KEY=$(grep -E '^OPERATOR_BTC_WALLET_PRIVATE_KEY=' ../operator/.env | head -1 | cut -d= -f2- | tr -d '"' | tr -d "'")
export BTC_WALLET_KEY
exec cargo run -q -p ipow-node -- --settings node.testnet.yml "$@"
