#!/usr/bin/env bash
# Runs the node on the test networks with node.testnet.yml, loading the keys
# and endpoints from where they already are, so that none is copied: the
# Ethereum package's git-ignored .env, the Solana CLI's keypair, and the
# operator's Bitcoin key in this package's git-ignored .env.operator-btc. Extra
# arguments go to the node (for example --check).
set -euo pipefail
cd "$(dirname "$0")"
set -a
. ../evm/.env
# The other EVM test networks' endpoints and keys: each package's own .env.
for n in base hyperliquid robinhood arbitrum; do
  if [ -f "../networks/$n/.env" ]; then . "../networks/$n/.env"; fi
done
set +a
# HyperEVM: the endpoint in networks/hyperliquid/.env reads well but
# refuses every transaction, and the public RPC sends but rate-limits a
# node's reads (both 2026-10-10). scripts/rpc-split.mjs, on this machine,
# sends the sends to the public RPC and the reads to the .env endpoint.
: "${HYPEREVM_TESTNET_RPC_URL:?HYPEREVM_TESTNET_RPC_URL is not set (networks/hyperliquid/.env)}"
READS="$HYPEREVM_TESTNET_RPC_URL" SENDS=https://rpc.hyperliquid-testnet.xyz/evm PORT=8546 node scripts/rpc-split.mjs > rpc-split.log 2>&1 &
SPLIT=$!
# The router answers before the node starts, or the script stops here.
for _ in $(seq 1 50); do
  curl -s -m 2 -o /dev/null -X POST -H 'content-type: application/json' --data '{"jsonrpc":"2.0","id":1,"method":"eth_chainId","params":[]}' http://127.0.0.1:8546 && break
  kill -0 $SPLIT 2>/dev/null || { echo "the HyperEVM router stopped: see rpc-split.log" >&2; exit 1; }
  sleep 0.2
done
export HYPEREVM_TESTNET_RPC_URL=http://127.0.0.1:8546
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
# Not exec, so the router stops with the node: a stop signal is passed on to
# the node, and the router is stopped once the node has.
cargo run -q -p ipow-node -- --settings node.testnet.yml "$@" &
NODE=$!
trap 'kill -TERM $NODE 2>/dev/null' TERM INT
trap 'kill $SPLIT 2>/dev/null' EXIT
wait $NODE
wait $NODE 2>/dev/null || true
