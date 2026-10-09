// A local Solana validator for the SDK's tests, running the production
// builds of the programs at their declared ids, upgradeable by `authority`
// (the vault's set-up demands the upgrade authority), in its own process
// group so that stopping it stops nothing else.

import { spawn, type ChildProcess } from "node:child_process";
import { mkdtempSync, openSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { Connection, Keypair, LAMPORTS_PER_SOL } from "@solana/web3.js";

const deploy = join(dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "solana", "target", "deploy");

export type Local = { connection: Connection; authority: Keypair; stop(): void };

/** Metaplex Token Metadata, as deployed: a basket's token is named through
 *  it. Dumped from mainnet on 2026-10-05 into the basket program's test
 *  fixtures. */
export const TOKEN_METADATA = { id: "metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s", file: join(deploy, "..", "..", "programs", "applications", "beta-basket", "tests", "fixtures", "mpl_token_metadata.so") };

/** Starts a validator with `programs` (id → build name in target/deploy, or
 *  a path to a .so). */
export async function startValidator(port: number, programs: Record<string, string>): Promise<Local> {
  const dir = mkdtempSync(join(tmpdir(), "ipow-sdk-validator-"));
  const authority = Keypair.generate();
  const auth = join(dir, "authority.json");
  writeFileSync(auth, JSON.stringify(Array.from(authority.secretKey)));
  const args = ["--reset", "--ledger", join(dir, "ledger"), "--rpc-port", String(port), "--faucet-port", String(port + 101)];
  for (const [id, name] of Object.entries(programs)) args.push("--upgradeable-program", id, name.endsWith(".so") ? name : join(deploy, `${name}.so`), auth);
  const log = join(dir, "validator.log");
  const validator: ChildProcess = spawn("solana-test-validator", args, { detached: true, stdio: ["ignore", openSync(log, "w"), openSync(log, "a")] });
  const connection = new Connection(`http://127.0.0.1:${port}`, "confirmed");
  let up = false;
  for (let i = 0; i < 60 && !up; i++) {
    try {
      await connection.getVersion();
      up = true;
    } catch {
      await new Promise((r) => setTimeout(r, 1000));
    }
  }
  const stop = () => {
    if (validator.pid) process.kill(-validator.pid);
  };
  if (!up) {
    stop();
    throw new Error(`the local validator did not start; see ${log}`);
  }
  await connection.confirmTransaction(await connection.requestAirdrop(authority.publicKey, 100 * LAMPORTS_PER_SOL));
  return { connection, authority, stop };
}
