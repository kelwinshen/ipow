// Sets up the new programs on Solana devnet after they are deployed (stage
// 8): the light client's lowest height (D93), the protocol, Conversion, and
// the vault's pair with each EVM network deployed (D132), naming the vault
// there. Each step is skipped when its account exists already, and every
// pair is read back. Run from programmable-network/solana:
//
//   node scripts/init-testnet.ts [--dry]
//
// With --dry it only reads devnet and prints what it would do.
//
// The key is the Solana CLI's (~/.config/solana/id.json), the programs'
// upgrade authority; it is never printed.

import anchor from "@coral-xyz/anchor";
import { Connection, Keypair, PublicKey, SystemProgram } from "@solana/web3.js";
import { readFileSync, existsSync } from "node:fs";
import { homedir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const { AnchorProvider, Program, Wallet, BN } = anchor;
const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, "..");

/** D93: about 4 weeks below Bitcoin's height on 2026-10-02, as on every EVM network. */
const MIN_HEIGHT = 965_567;
/** Small amounts for a test network, in lamports (D121, D118): 0.001 and 0.01 SOL. */
const DEPOSIT = new BN(1_000_000);
const MIN_CERTIFYING_ESCROW = new BN(10_000_000);
/** EVM networks by number (D133), with their deployments files. */
const PEERS: [string, number][] = [["ethereum", 1], ["base", 3], ["robinhood", 4], ["polkadot", 5], ["hedera", 6], ["hyperliquid", 7], ["tempo", 8], ["arbitrum", 9]];

const DRY = process.argv.includes("--dry");
const LOADER = new PublicKey("BPFLoaderUpgradeab1e11111111111111111111111");
const idl = (prog: string, name: string) => JSON.parse(readFileSync(join(root, "programs", prog, "idls", `${name}.json`), "utf8"));

const key = Keypair.fromSecretKey(Uint8Array.from(JSON.parse(readFileSync(join(homedir(), ".config/solana/id.json"), "utf8"))));
const connection = new Connection("https://api.devnet.solana.com", "confirmed");
const provider = new AnchorProvider(connection, new Wallet(key), { commitment: "confirmed" });

const lc = new Program(idl("ipow-light-client", "ipow_light_client"), provider);
const pr = new Program(idl("ipow-protocol", "ipow_protocol"), provider);
const cv = new Program(idl("conversion", "conversion"), provider);
const vt = new Program(idl("ipow-vault", "ipow_vault"), provider);

const pda = (program: PublicKey, seeds: (Buffer | Uint8Array)[]) => PublicKey.findProgramAddressSync(seeds, program)[0];
const programData = (program: PublicKey) => pda(LOADER, [program.toBuffer()]);
const exists = async (a: PublicKey) => (await connection.getAccountInfo(a)) !== null;

console.log("authority", key.publicKey.toBase58());

// The light client.
const lcConfig = pda(lc.programId, [Buffer.from("config")]);
if (await exists(lcConfig)) console.log("light client: set up already");
else if (DRY) console.log("light client: would set up, lowest height", MIN_HEIGHT);
else {
  const sig = await lc.methods
    .initialize(MIN_HEIGHT, Array(32).fill(0), Array(32).fill(0))
    .accountsStrict({ config: lcConfig, dayTable: pda(lc.programId, [Buffer.from("days")]), authority: key.publicKey, programData: programData(lc.programId), systemProgram: SystemProgram.programId })
    .rpc();
  console.log("light client: set up", sig);
}
if (await exists(lcConfig)) {
  const lcRead: any = await (lc.account as any).config.fetch(lcConfig);
  if (lcRead.minHeight !== MIN_HEIGHT) throw new Error(`light client's lowest height reads ${lcRead.minHeight}`);
  // A production build stores Bitcoin's real limits; a test build would
  // store what was passed (zeros here).
  if (lcRead.maxTarget.every((b: number) => b === 0) || lcRead.powLimit.every((b: number) => b === 0)) throw new Error("light client: a test build's limits");
  console.log("light client: lowest height", lcRead.minHeight, "with Bitcoin's limits");
}

// The protocol.
const prState = pda(pr.programId, [Buffer.from("protocol")]);
if (await exists(prState)) console.log("protocol: set up already");
else if (DRY) console.log("protocol: would set up");
else {
  const sig = await pr.methods
    .initializeProtocol()
    .accountsStrict({ protocol: prState, vault: pda(pr.programId, [Buffer.from("vault")]), payer: key.publicKey, systemProgram: SystemProgram.programId })
    .rpc();
  console.log("protocol: set up", sig);
}

// Conversion, registered with the protocol as an application.
const cvConfig = pda(cv.programId, [Buffer.from("config")]);
if (await exists(cvConfig)) console.log("conversion: set up already");
else if (DRY) console.log("conversion: would set up");
else {
  const sig = await cv.methods
    .initialize()
    .accountsStrict({ config: cvConfig, application: pda(pr.programId, [Buffer.from("application"), cvConfig.toBuffer()]), payer: key.publicKey, protocolProgram: pr.programId, systemProgram: SystemProgram.programId })
    .rpc();
  console.log("conversion: set up", sig);
}

// The vault's pair with each EVM network deployed.
for (const [network, number] of PEERS) {
  const file = join(root, "..", "ethereum", "deployments", `${network}-testnet.json`);
  if (!existsSync(file)) {
    console.log(`vault pair ${network} (${number}): not deployed there yet, skipped`);
    continue;
  }
  const d = JSON.parse(readFileSync(file, "utf8"));
  const evmVault: string = d.vaults.find((v: any) => v.peer === 2).vault;
  const peerVault = Array.from(Buffer.from(evmVault.slice(2), "hex"));
  const config = pda(vt.programId, [Buffer.from("config"), Buffer.from([number])]);
  // The EVM vault must name this account as its peer: the account recorded,
  // and the 32 bytes the vault holds, decoded here by Solana's own library.
  if (d.solanaPairAccount !== config.toBase58()) throw new Error(`${network}: its vault names ${d.solanaPairAccount}, the pair account is ${config.toBase58()}`);
  const held = "0x" + Buffer.from(config.toBytes()).toString("hex");
  if (held.toLowerCase() !== d.vaults[0].peerVault.toLowerCase()) throw new Error(`${network}: its vault holds ${d.vaults[0].peerVault}, the pair account is ${held}`);
  if (await exists(config)) console.log(`vault pair ${network} (${number}): set up already`);
  else if (DRY) {
    console.log(`vault pair ${network} (${number}): would set up ${config.toBase58()} -> ${evmVault}`);
    continue;
  } else {
    const sig = await vt.methods
      .initialize(number, peerVault, DEPOSIT, MIN_CERTIFYING_ESCROW)
      .accountsStrict({
        config,
        sol: pda(vt.programId, [Buffer.from("asset"), config.toBuffer(), Buffer.from([0, 0, 0, 0])]),
        application: pda(pr.programId, [Buffer.from("application"), config.toBuffer()]),
        payer: key.publicKey,
        programData: programData(vt.programId),
        protocolProgram: pr.programId,
        systemProgram: SystemProgram.programId,
      })
      .rpc();
    console.log(`vault pair ${network} (${number}): set up`, sig);
  }
  const c: any = await (vt.account as any).config.fetch(config);
  const named = "0x" + Buffer.from(c.peerVault).toString("hex");
  if (c.peer !== number || named.toLowerCase() !== evmVault.toLowerCase() || !c.deposit.eq(DEPOSIT) || !c.minCertifyingEscrow.eq(MIN_CERTIFYING_ESCROW)) {
    throw new Error(`vault pair ${network}: reads peer ${c.peer}, vault ${named}, deposit ${c.deposit}, escrow ${c.minCertifyingEscrow}`);
  }
  console.log(`vault pair ${network} (${number}): reads back ${config.toBase58()} -> ${named}`);
}
