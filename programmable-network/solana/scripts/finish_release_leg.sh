#!/usr/bin/env bash
# Completes the BETA v2 redeem leg once the RELEASE anchor is confirmed on
# Bitcoin mainnet (docs/DESIGN_V2.md §6.5, §6.14). Run after broadcasting
# the anchor with core/operator's `anchor_statement` example.
#   A2  = the RELEASE anchor txid (deterministic: RFC6979 signatures)
#   REL = the RELEASE statement hex printed by ACTION=statement-release
set -euo pipefail
A2=${A2:-f85336e006b16fd4ac4991a0fbdb96e056f0bed7a66e3316fadf70c7001af8e5}
REL=${REL:-02000000000000000100000000000000009784b80bc7b95753f2a2732649f3a1136b4c2bf10000000000000001}
P=0101010101010101010101010101010101010101010101010101010101010101
SOL=/Users/kelwin/ipow/programmable-network/solana
ETH=/Users/kelwin/ipow/programmable-network/ethereum

echo "waiting for $A2 to confirm..."
while :; do
  S=$(curl -s -m 20 "https://blockstream.info/api/tx/$A2/status" || echo '{}')
  if echo "$S" | grep -q '"confirmed":true'; then
    H=$(echo "$S" | sed -E 's/.*"block_height":([0-9]+).*/\1/')
    echo "confirmed in $H"
    break
  fi
  sleep 60
done

echo "== relay $H to devnet ipow (jump/extend as needed)"
(cd "$SOL" && ACTION=relay-header HEIGHT=$H npx ts-node scripts/beta_factory_e2e.ts | tail -1)
echo "== relay $H to Sepolia iPoWV1"
(cd "$ETH" && ACTION=relay-header HEIGHT=$H node scripts/beta_vault_e2e.mjs | tail -1)
echo "== Sepolia: queue the release (lock 1 FINAL -> Released, pays after T_rel=600s)"
(cd "$ETH" && ACTION=process PARTY_ID=0x$P TXID=$A2 STATEMENT=$REL node scripts/beta_vault_e2e.mjs | tail -2)
echo "== devnet: judge the release (burn 0 exists -> Exercised, burn.claimed)"
(cd "$SOL" && ACTION=process PARTY_ID=$P TXID=$A2 STATEMENT=$REL npx ts-node scripts/beta_factory_e2e.ts | tail -1)
echo "== waiting T_rel (600s) then paying 0.001 ETH to the burn's to_eth"
sleep 610
(cd "$ETH" && ACTION=execute-release TXID=$A2 node scripts/beta_vault_e2e.mjs | tail -1)
(cd "$ETH" && ACTION=status LOCK_ID=1 TXID=$A2 node scripts/beta_vault_e2e.mjs | tail -6)
