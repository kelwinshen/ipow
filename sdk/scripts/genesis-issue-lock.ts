// Issues one lock made on an EVM network on Solana by genesis's G2
// (docs/specs/ipow-vault-genesis.md): for a lock someone made in the app
// while the pair is still in genesis and no operator can carry it yet (an
// operator's first registration waits for a real block, a checkpoint job's
// lock of 36 hours at least). It mints exactly what the lock names, its
// amount and fast fee, to its own recipient, and marks it issued with the
// slow path's mark, so no claim or attest can issue it again (D143). The
// genesis key is the Solana CLI's key; nothing secret is printed. Run from sdk:
//
//   node scripts/genesis-issue-lock.ts <network> <lock id> [--dry]
//
// It reads the lock on its home vault (the network's vault paired with
// Solana) and its mark on Solana first, and refuses a lock returned,
// issued, given up or attested. Each issue is written to
// evm/deployments/genesis-testnet.json beside the genesis run's own, so
// scripts/genesis-check.ts matches it to its lock like the rest.

import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { FetchRequest, JsonRpcProvider } from "ethers";
import { Connection, Keypair, PublicKey, Transaction } from "@solana/web3.js";

import { NETWORKS, envFile } from "../../evm/deploy/networks.ts";
import { SolanaVault, getLock, network } from "../src/index.ts";

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, "..", "..");
const args = process.argv.slice(2).filter((a) => !a.startsWith("--"));
const DRY = process.argv.includes("--dry");
const [home, idText] = args;
const s = home ? NETWORKS[home] : undefined;
if (!s || !/^\d+$/.test(idText ?? "")) throw new Error("usage: genesis-issue-lock.ts <EVM network> <lock id> [--dry]");
const lockId = BigInt(idText);
const SOLANA = 2;

const vars: Record<string, string> = {};
for (const line of readFileSync(envFile(home), "utf8").split("\n")) {
  const m = line.match(/^\s*([A-Z0-9_]+)\s*=\s*"?([^"\n]*)"?/);
  if (m) vars[m[1]] = m[2].trim();
}
const request = new FetchRequest(vars[s.rpcEnv.testnet!]);
request.setHeader("user-agent", "ipow-genesis-issue");
const provider = new JsonRpcProvider(request, undefined, { staticNetwork: true });
const d = network(`${home}-testnet` as any);
const pair = (d.vaults as readonly { peer: number }[]).findIndex((v) => v.peer === SOLANA);
if (pair < 0) throw new Error(`${home}: no vault paired with Solana`);

const key = Keypair.fromSecretKey(Uint8Array.from(JSON.parse(readFileSync(join(homedir(), ".config/solana/id.json"), "utf8"))));
const connection = new Connection(process.env.SOLANA_RPC_URL ?? "https://api.devnet.solana.com", "confirmed");
const wallet = {
  publicKey: key.publicKey,
  signTransaction: async <T extends Transaction>(t: T) => (t.partialSign(key), t),
  signAllTransactions: async <T extends Transaction>(ts: T[]) => ts.map((t) => (t.partialSign(key), t)),
};
const vault = new SolanaVault(connection, d.number, wallet as any);

// The lock as its home vault holds it.
const l = await getLock(provider, d, lockId, pair);
const recipient = new PublicKey(Buffer.from(l.recipient.slice(2), "hex")).toBase58();
const value = l.amount + l.fastFee;
console.log(`${home} lock ${lockId}: asset ${l.asset}, amount ${l.amount} + fast fee ${l.fastFee} = ${value} record units, to ${recipient} on Solana; ${l.stage}`);
if (l.stage === "Returned") throw new Error("the lock was returned to its owner");

// Its mark on Solana: not issued, given up or attested.
const mark = await vault.evmLockMark(lockId);
if (mark?.issued) throw new Error("issued on Solana already");
if (mark?.givenUp) throw new Error("given up on Solana");
if (mark && mark.attests > 0) throw new Error("attested on Solana: the fast path issues it");
if ((await connection.getAccountInfo(vault.receiptMint(l.asset))) === null) throw new Error(`asset ${l.asset}'s receipt is not made on Solana`);
console.log("  not issued, given up or attested on Solana; its receipt is made");
if (DRY) {
  console.log("dry: nothing sent");
  process.exit(0);
}

const tx = await vault.genesisIssue(lockId, l.asset, value, recipient);
console.log(`issued on Solana: ${tx}`);
const after = await vault.evmLockMark(lockId);
if (!after?.issued) throw new Error("the mark does not read issued after the transaction");
console.log("read back: the lock's mark on Solana is issued");

const LEDGER = join(root, "evm", "deployments", "genesis-testnet.json");
const ledger = existsSync(LEDGER) ? JSON.parse(readFileSync(LEDGER, "utf8")) : { started: new Date().toISOString(), entries: [] };
ledger.entries.push({ home, receiptSide: "solana", asset: l.asset, symbol: `asset ${l.asset}`, token: "", recordDecimals: 0, lockId: String(lockId), value: String(value), recipient, issueTx: tx, note: "a lock made in the app, issued by genesis-issue-lock.ts" });
writeFileSync(LEDGER, JSON.stringify(ledger, null, 2) + "\n");
console.log(`wrote ${LEDGER}`);
