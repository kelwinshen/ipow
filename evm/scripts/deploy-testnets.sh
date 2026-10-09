#!/usr/bin/env bash
# The testnet deployment of stage 8, in order. Each step spends test funds
# only; keys come from each package's git-ignored .env and the Solana CLI's
# keypair. Run from the repository root:
#
#   bash evm/scripts/deploy-testnets.sh
#
# The Solana programs are built already (anchor build, production). Every
# EVM network's addresses are written to
# evm/deployments/<network>-testnet.json, each read
# back from the network.
set -euo pipefail

# D93: about 4 weeks below Bitcoin's height on 2026-10-02 (969,599 - 4,032).
MIN_HEIGHT=965567
# Conversion's largest swap on a test network: 0.001 BTC.
MAX_SATS=100000

SOL=solana
ETH=evm

# 1. Solana programs to devnet, at the ids recorded in
#    solana/deployments/devnet.json (the same as Anchor.toml's and each
#    program's declare_id!); a program already there is left as it is, and
#    scripts/verify-devnet.sh checks it against its record. Not
#    target/deploy/*-keypair.json: the vault's there is the retired
#    program's. A program not on devnet yet is deployed with the keypair
#    of its recorded id, from target/deploy or .keys, or refused.
for name in ipow_light_client ipow_protocol ipow_vault conversion beta_basket; do
  id=$(node -e 'console.log(require(process.argv[1]).programs[process.argv[2]].id)' "$PWD/$SOL/deployments/devnet.json" "$name")
  if solana program show "$id" --url devnet >/dev/null 2>&1; then
    echo "$name already at $id"
    continue
  fi
  key=""
  for k in "$SOL/target/deploy/$name-keypair.json" "$SOL"/.keys/"$name"*-keypair.json; do
    if [ -f "$k" ] && [ "$(solana-keygen pubkey "$k")" = "$id" ]; then key=$k; break; fi
  done
  if [ -z "$key" ]; then echo "$name: no keypair for its recorded id $id" >&2; exit 1; fi
  solana program deploy "$SOL/target/deploy/$name.so" \
    --program-id "$key" \
    --url devnet --max-sign-attempts 50 --use-rpc
done

# 2. EVM networks, each vault paired with Solana: its peer vault is the
#    pair account ["config", <network number>] of the vault program
#    3TZ1LJ4fVZVKbBUyzVMRjJ9FNPGaqVmGX56JuT5QdXEy (the genesis vault
#    program; each record's solanaPairAccount).
#    A network whose deployments file exists is done and left as it is.
deploy() {
  if [ -f "$ETH/deployments/$1-testnet.json" ]; then echo "$1 already deployed"; return; fi
  (cd "$ETH" && node scripts/deploy-network.ts "$1" testnet "$MIN_HEIGHT" "$MAX_SATS" "$2" ${3:-})
}
deploy ethereum  Cztcuoj5XAhZ35ky6Ud3WMq9R7ijcvPTnnvSjGQ88sEg
deploy base      EEyy1MZhpDXurack8QhkeoAVTDmsHkn2zZ1H9QynppMN
deploy robinhood GCpZ6DFQyeVLNBzKbeV4okqW745kioHvJmGGAjs1W1So
deploy polkadot  36fbxW7aLxS5VFBo4HUcNFweBo4WRQTm2SorKuMzqSfX
# Hedera keeps its pair with the earlier vault program on purpose: this is
# ["config", 6] of the retired 2e1ZUB5eqA6bQfH3f9pbfvibfie7WnvEJeKif53VAm7p,
# as its record says (Hedera's vault was not redeployed in genesis).
deploy hedera    7Wwn5bjARBmdCjNT7KUBwB7LR3JYSLFAWSSm98pxvE2S
# Arbitrum (D141, network 9) is not deployed here: scripts/deploy-arbitrum.sh
# deploys it, with its pairs with Solana and the other EVM networks.
# Hyperliquid only once the deployer's address uses big blocks: switch it
# with `node scripts/hyperliquid-big-blocks.ts` (from $ETH) first.
if (cd "$ETH" && node scripts/hyperliquid-big-blocks.ts --check) | grep -q "uses big blocks: true"; then
  deploy hyperliquid 8nJB7bCtN6DpeZGHNtLxaE11rdKnRkFhGR4LJUTsCffo --big-blocks
else
  echo "hyperliquid skipped: its deployer does not use big blocks yet"
fi

# Tempo: its own transactions, sent with viem from its package.
if [ -f "$ETH/deployments/tempo-testnet.json" ]; then echo "tempo already deployed"; else
  (cd networks/tempo && node scripts/deploy-new-protocol.ts)
fi

# 3. Solana's set-up: the light client, the protocol, Conversion, and the
#    vault's pair with each EVM network deployed above; skips what exists.
(cd "$SOL" && node scripts/init-testnet.ts)
