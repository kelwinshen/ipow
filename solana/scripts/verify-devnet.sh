#!/usr/bin/env bash
# Reads each devnet program back and compares it with its record in
# deployments/devnet.json: the account's data up to the recorded length has
# the recorded sha256, and the rest is zeros. Read-only (solana program
# dump). Run from solana, for every program or the ones named:
#
#   bash scripts/verify-devnet.sh [ipow_vault ...]
#
# Exits non-zero if any program is not its record.
set -euo pipefail

RECORD="$(cd "$(dirname "$0")/.." && pwd)/deployments/devnet.json"
names=("$@")
if [ ${#names[@]} -eq 0 ]; then
  names=($(node -e 'console.log(Object.keys(require(process.argv[1]).programs).join(" "))' "$RECORD"))
fi
field() { node -e 'const p=require(process.argv[1]).programs[process.argv[2]]; if(!p) process.exit(2); console.log(p[process.argv[3]])' "$RECORD" "$1" "$2"; }

DUMP=$(mktemp)
trap 'rm -f "$DUMP"' EXIT
failed=0
for name in "${names[@]}"; do
  id=$(field "$name" id) || { echo "$name: not in the record" >&2; failed=1; continue; }
  length=$(field "$name" length)
  want=$(field "$name" sha256)
  solana program dump "$id" "$DUMP" --url devnet >/dev/null
  size=$(wc -c < "$DUMP" | tr -d ' ')
  got=$(head -c "$length" "$DUMP" | shasum -a 256 | cut -d' ' -f1)
  rest=$(tail -c +"$((length + 1))" "$DUMP" | tr -d '\0' | wc -c | tr -d ' ')
  if [ "$size" -ge "$length" ] && [ "$got" = "$want" ] && [ "$rest" = 0 ]; then
    echo "$name $id: the recorded build (source $(field "$name" source | cut -c1-7))"
  else
    echo "$name $id: NOT the recorded build (sha256 $got of $length of $size bytes)" >&2
    failed=1
  fi
done
exit "$failed"
