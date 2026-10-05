#!/usr/bin/env bash
# The testnet deployment of stage 8, in order. Each step spends test funds
# only; keys come from each package's git-ignored .env and the Solana CLI's
# keypair. Run from the repository root:
#
#   bash programmable-network/ethereum/scripts/deploy-testnets.sh
#
# The Solana programs are built already (anchor build, production). Every
# EVM network's addresses are written to
# programmable-network/ethereum/deployments/<network>-testnet.json, each read
# back from the network.
set -euo pipefail

# D93: about 4 weeks below Bitcoin's height on 2026-10-02 (969,599 - 4,032).
MIN_HEIGHT=965567
# Conversion's largest swap on a test network: 0.001 BTC.
MAX_SATS=100000

SOL=programmable-network/solana
ETH=programmable-network/ethereum

# 1. Solana programs to devnet, at their declared ids; a program already
#    there is left as it is.
for name in ipow_light_client ipow_protocol ipow_vault conversion beta_basket; do
  id=$(solana-keygen pubkey "$SOL/target/deploy/$name-keypair.json")
  if solana program show "$id" --url devnet >/dev/null 2>&1; then
    echo "$name already at $id"
    continue
  fi
  solana program deploy "$SOL/target/deploy/$name.so" \
    --program-id "$SOL/target/deploy/$name-keypair.json" \
    --url devnet --max-sign-attempts 50 --use-rpc
done

# 2. EVM networks, each vault paired with Solana: its peer vault is the
#    Solana vault's pair account ["config", <network number>].
#    A network whose deployments file exists is done and left as it is.
deploy() {
  if [ -f "$ETH/deployments/$1-testnet.json" ]; then echo "$1 already deployed"; return; fi
  (cd "$ETH" && node scripts/deploy-network.ts "$1" testnet "$MIN_HEIGHT" "$MAX_SATS" "$2" ${3:-})
}
deploy ethereum  5Ks1r25VGX5S7dUMf8c5oP6qzXfFvckQ67wgtqywnDAN
deploy base      Gyhrq1CzHyU9BfECK5G8RW4ho75oaRPqMngXYYvLuduL
deploy robinhood Dydhoos4EgX15LFLJR5a6DvGtuzmNjegpzzynJSJbF5
deploy polkadot  BcTMgkGcknJu9GW6XQ7HoRdXdP3Jo6iRt8ZjKe6fm7r7
deploy hedera    7Wwn5bjARBmdCjNT7KUBwB7LR3JYSLFAWSSm98pxvE2S
# Arbitrum (D141, network 9) is not deployed here: Solana's vault must first
# be upgraded to the build that takes 9. scripts/deploy-arbitrum.sh does it.
# Hyperliquid only once the deployer's address uses big blocks: switch it
# with `node scripts/hyperliquid-big-blocks.ts` (from $ETH) first.
if (cd "$ETH" && node scripts/hyperliquid-big-blocks.ts --check) | grep -q "uses big blocks: true"; then
  deploy hyperliquid D73AnEY7aViFo3r6P8iPxMBxEqh6P5DiR4B6dMj7SixA --big-blocks
else
  echo "hyperliquid skipped: its deployer does not use big blocks yet"
fi

# Tempo: its own transactions, sent with viem from its package.
if [ -f "$ETH/deployments/tempo-testnet.json" ]; then echo "tempo already deployed"; else
  (cd programmable-network/tempo && node scripts/deploy-new-protocol.ts)
fi

# 3. Solana's set-up: the light client, the protocol, Conversion, and the
#    vault's pair with each EVM network deployed above; skips what exists.
(cd "$SOL" && node scripts/init-testnet.ts)
