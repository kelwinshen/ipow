#!/usr/bin/env bash
# Broadcasts pre-signed statement-chain anchors in order (docs/DESIGN_V2.md
# §6.14, adversarial round). Each line of the input file is:
#   <label> <txid> stmt <statement_hex> witness <signed_tx_hex>
# The anchors form a chain (each spends the previous one's outputs), so they
# must go out in file order. Unconfirmed chaining is fine for the mempool.
set -euo pipefail
FILE=${1:-$(dirname "$0")/adversarial_anchors.txt}
while read -r label txid _ stmt _ hex; do
  echo "== $label $txid"
  resp=$(curl -s -m 30 -X POST https://blockstream.info/api/tx -H 'Content-Type: text/plain' --data "$hex" || true)
  if [ "$resp" = "$txid" ]; then
    echo "   broadcast ok"
  else
    echo "   response: $resp"
    resp2=$(curl -s -m 30 -X POST https://mempool.space/api/tx -H 'Content-Type: text/plain' --data "$hex" || true)
    echo "   mempool.space: $resp2"
  fi
  sleep 2
done < "$FILE"
