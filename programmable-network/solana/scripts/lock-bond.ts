// Locks an operator's bond in the protocol on Solana devnet: the node never
// locks a bond itself. The key is the Solana CLI's. Run from
// programmable-network/solana:
//
//   node scripts/lock-bond.ts <lamports> [--check]
//
// With --check it only reads the bond.

import anchor from "@coral-xyz/anchor";
import { Connection, Keypair, PublicKey, SystemProgram } from "@solana/web3.js";
import { readFileSync } from "node:fs";
import { homedir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const { AnchorProvider, Program, Wallet, BN } = anchor;
const here = dirname(fileURLToPath(import.meta.url));
const key = Keypair.fromSecretKey(Uint8Array.from(JSON.parse(readFileSync(join(homedir(), ".config/solana/id.json"), "utf8"))));
const connection = new Connection("https://api.devnet.solana.com", "confirmed");
const provider = new AnchorProvider(connection, new Wallet(key), { commitment: "confirmed" });
const pr = new Program(JSON.parse(readFileSync(join(here, "..", "programs", "ipow-protocol", "idls", "ipow_protocol.json"), "utf8")), provider);
const pda = (seeds: Buffer[]) => PublicKey.findProgramAddressSync(seeds, pr.programId)[0];
const operator = pda([Buffer.from("operator"), key.publicKey.toBuffer()]);

const show = async () => {
  const info = await connection.getAccountInfo(operator);
  if (!info) return console.log(`solana: ${key.publicKey.toBase58()} has no bond yet`);
  const o: any = await (pr.account as any).operator.fetch(operator);
  console.log(`solana: ${key.publicKey.toBase58()} bond ${o.bond?.toString()} lamports, locked ${o.locked?.toString()}`);
};
await show();
if (process.argv.includes("--check")) process.exit(0);
const lamports = new BN(process.argv.slice(2).find((a) => !a.startsWith("--")));
const sig = await pr.methods
  .lockBond(lamports)
  .accountsStrict({ protocol: pda([Buffer.from("protocol")]), operator, vault: pda([Buffer.from("vault")]), owner: key.publicKey, systemProgram: SystemProgram.programId })
  .rpc();
console.log("lock_bond", sig);
await show();
