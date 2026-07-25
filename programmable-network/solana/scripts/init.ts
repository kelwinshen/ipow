// Post-deploy admin/config CLI for the deployed ipow program on this cluster.
//
// Every write action below maps 1:1 to an operator/admin-gated instruction,
// so the resolved wallet must be the program's `global_state.operator` (or,
// for `initialize` itself, whichever account you want to become both admin
// and operator — this script uses the same wallet for both, matching how
// every EVM deployment in this protocol used a single deployer/operator key).
//
// Usage (env-var driven, mirrors the EVM packages' scripts/configure.ts):
//
//   ACTION=status                                          npx ts-node scripts/init.ts
//   ACTION=initialize        [COMMIT_FEE_BPS=50]            npx ts-node scripts/init.ts
//   ACTION=add-network        NETWORK_ID=1 [MIN_ADDR_LEN=20] [MAX_ADDR_LEN=20] npx ts-node scripts/init.ts
//   ACTION=update-network      NETWORK_ID=1 MIN_ADDR_LEN=20 MAX_ADDR_LEN=20 [IS_ACTIVE=true] npx ts-node scripts/init.ts
//   ACTION=add-liquidity          AMOUNT=0.5                npx ts-node scripts/init.ts
//   ACTION=remove-liquidity        AMOUNT=0.5               npx ts-node scripts/init.ts
//   ACTION=all               [COMMIT_FEE_BPS=50] [AMOUNT=0.5] npx ts-node scripts/init.ts
//
// `all` (the default) is the original "post-deploy setup" convenience path:
// it initializes global_state if not already, idempotently registers every
// *other* iPoW-registry network (skipping ones already registered), and, if
// AMOUNT is set, adds that much liquidity.
//
// Cluster/wallet are read from ANCHOR_PROVIDER_URL/ANCHOR_WALLET if set,
// otherwise from this workspace's Anchor.toml [provider] section — so this
// works whether run via `anchor run init` or directly with `npx ts-node`.

import fs from "node:fs";
import os from "node:os";
import path from "node:path";

import * as anchor from "@anchor-lang/core";

// This workspace's TS is compiled as CommonJS (see tsconfig.json), so
// `__dirname` is used rather than `import.meta.dirname`.
const WORKSPACE_ROOT = path.join(__dirname, "..");

// iPoW protocol network ID registry (assigned by the protocol, not tied to
// each chain's own chain ID): 1 = Hedera, 2 = Ethereum, 3 = Solana, 4 = Polkadot.
// addrLen is the on-the-wire byte length of that chain's native address
// encoding (20-byte EVM addresses for Hedera/Ethereum/Polkadot; this chain's
// own 32-byte pubkeys are naturally excluded, since Solana never registers
// itself).
const NETWORK_REGISTRY = [
  { id: 1, name: "Hedera", addrLen: 20 },
  { id: 2, name: "Ethereum", addrLen: 20 },
  { id: 4, name: "Polkadot", addrLen: 20 },
];

const CLUSTER_URLS: Record<string, string> = {
  devnet: "https://api.devnet.solana.com",
  testnet: "https://api.testnet.solana.com",
  "mainnet-beta": "https://api.mainnet-beta.solana.com",
  mainnet: "https://api.mainnet-beta.solana.com",
  localnet: "http://127.0.0.1:8899",
  localhost: "http://127.0.0.1:8899",
};

function readAnchorTomlProvider(): { cluster: string; wallet: string } {
  const raw = fs.readFileSync(path.join(WORKSPACE_ROOT, "Anchor.toml"), "utf8");
  const section = raw.match(/\[provider\]([\s\S]*?)(\n\[|$)/)?.[1] ?? "";
  const cluster = section.match(/cluster\s*=\s*"([^"]+)"/)?.[1] ?? "devnet";
  const wallet = section.match(/wallet\s*=\s*"([^"]+)"/)?.[1] ?? "~/.config/solana/id.json";
  return { cluster, wallet };
}

function loadProvider(): anchor.AnchorProvider {
  if (process.env.ANCHOR_PROVIDER_URL && process.env.ANCHOR_WALLET) {
    return anchor.AnchorProvider.env();
  }

  const { cluster, wallet } = readAnchorTomlProvider();
  const url = CLUSTER_URLS[cluster] ?? cluster; // allow a raw URL in Anchor.toml too
  const walletPath = wallet.replace(/^~/, os.homedir());
  const secretKey = Uint8Array.from(
    JSON.parse(fs.readFileSync(walletPath, "utf8"))
  );
  const keypair = anchor.web3.Keypair.fromSecretKey(secretKey);
  const connection = new anchor.web3.Connection(url, "confirmed");
  return new anchor.AnchorProvider(connection, new anchor.Wallet(keypair), {
    commitment: "confirmed",
  });
}

// Returned (and threaded through below) as `any` rather than a typed
// `Program<Idl>`: the generated IDL is loaded at runtime, not imported as a
// literal type, so TS can't infer specific account/method names from it —
// and even `Program<any>` still forces excessively deep generic inference on
// `.methods`/`.account` namespace access. This script trades compile-time
// account/method-name checking for not having to hand-maintain a parallel
// type just for CLI plumbing.
function loadProgram(provider: anchor.AnchorProvider): any {
  const idl = JSON.parse(
    fs.readFileSync(path.join(WORKSPACE_ROOT, "target", "idl", "ipow.json"), "utf8")
  );
  return new anchor.Program(idl, provider);
}

function networkPda(program: any, networkId: number) {
  const [pda] = anchor.web3.PublicKey.findProgramAddressSync(
    [Buffer.from("network"), new anchor.BN(networkId).toArrayLike(Buffer, "le", 8)],
    program.programId
  );
  return pda;
}

function requireEnv(name: string): string {
  const value = process.env[name];
  if (!value) throw new Error(`Missing required env var ${name}`);
  return value;
}

async function main() {
  const provider = loadProvider();
  anchor.setProvider(provider);
  const program = loadProgram(provider);
  const wallet = provider.wallet.publicKey;

  const [globalState] = anchor.web3.PublicKey.findProgramAddressSync(
    [Buffer.from("global_state")],
    program.programId
  );
  const [escrowVault] = anchor.web3.PublicKey.findProgramAddressSync(
    [Buffer.from("escrow")],
    program.programId
  );

  console.log(
    `Connected to ipow program ${program.programId.toBase58()} as ${wallet.toBase58()}`
  );

  async function actionStatus() {
    const info = await provider.connection.getAccountInfo(globalState);
    if (!info) {
      console.log("global_state not initialized yet.");
      return;
    }
    const state = await program.account.globalState.fetch(globalState);
    const escrowBalance = await provider.connection.getBalance(escrowVault);
    console.log(`operator:        ${state.operator.toBase58()}`);
    console.log(`commitFeeBps:    ${state.commitFeeBps}`);
    console.log(`escrow balance:  ${escrowBalance / anchor.web3.LAMPORTS_PER_SOL} SOL`);
    console.log(`totalHeldCommitFees:  ${state.totalHeldCommitFees.toString()}`);
    console.log(`totalLockedDeposits:  ${state.totalLockedDeposits.toString()}`);
    console.log(`totalReservedNative:  ${state.totalReservedNative.toString()}`);
    console.log(`registered networks:`);
    for (const net of NETWORK_REGISTRY) {
      const pda = networkPda(program, net.id);
      const existing = await provider.connection.getAccountInfo(pda);
      if (!existing) {
        console.log(`  ${net.id} (${net.name}): not registered`);
        continue;
      }
      const cfg = await program.account.supportedNetwork.fetch(pda);
      console.log(
        `  ${net.id} (${net.name}): isActive=${cfg.isActive} minAddrLen=${cfg.minAddrLen} maxAddrLen=${cfg.maxAddrLen}`
      );
    }
  }

  async function actionInitialize() {
    const info = await provider.connection.getAccountInfo(globalState);
    if (info) {
      console.log("global_state already initialized, skipping.");
      return;
    }
    const commitFeeBps = Number(process.env.COMMIT_FEE_BPS ?? 50);
    console.log(`Initializing global_state (operator=${wallet.toBase58()}, commitFeeBps=${commitFeeBps})...`);
    const sig = await program.methods
      .initialize(wallet, commitFeeBps)
      .accounts({
        globalState,
        escrowVault,
        admin: wallet,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .rpc();
    console.log(`Done: ${sig}`);
  }

  async function actionAddNetwork() {
    const networkId = Number(requireEnv("NETWORK_ID"));
    const known = NETWORK_REGISTRY.find((n) => n.id === networkId);
    const minAddrLen = Number(process.env.MIN_ADDR_LEN ?? known?.addrLen);
    const maxAddrLen = Number(process.env.MAX_ADDR_LEN ?? known?.addrLen);
    if (!minAddrLen || !maxAddrLen) {
      throw new Error(`Unknown network ${networkId}: pass MIN_ADDR_LEN/MAX_ADDR_LEN explicitly`);
    }
    const networkConfig = networkPda(program, networkId);
    console.log(
      `Registering network ${networkId}${known ? ` (${known.name})` : ""} (minAddrLen=${minAddrLen}, maxAddrLen=${maxAddrLen})...`
    );
    const sig = await program.methods
      .addNetwork(new anchor.BN(networkId), minAddrLen, maxAddrLen)
      .accounts({
        globalState,
        networkConfig,
        admin: wallet,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .rpc();
    console.log(`Done: ${sig}`);
  }

  async function actionUpdateNetwork() {
    const networkId = Number(requireEnv("NETWORK_ID"));
    const minAddrLen = Number(requireEnv("MIN_ADDR_LEN"));
    const maxAddrLen = Number(requireEnv("MAX_ADDR_LEN"));
    const isActive = (process.env.IS_ACTIVE ?? "true") === "true";
    const networkConfig = networkPda(program, networkId);
    console.log(
      `Updating network ${networkId} (minAddrLen=${minAddrLen}, maxAddrLen=${maxAddrLen}, isActive=${isActive})...`
    );
    const sig = await program.methods
      .updateNetwork(new anchor.BN(networkId), minAddrLen, maxAddrLen, isActive)
      .accounts({ globalState, networkConfig, admin: wallet })
      .rpc();
    console.log(`Done: ${sig}`);
  }

  async function actionAddLiquidity() {
    const amountSol = Number(requireEnv("AMOUNT"));
    const lamports = new anchor.BN(Math.round(amountSol * anchor.web3.LAMPORTS_PER_SOL));
    console.log(`Adding ${amountSol} SOL liquidity...`);
    const sig = await program.methods
      .addLiquidity(lamports)
      .accounts({
        globalState,
        escrowVault,
        operator: wallet,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .rpc();
    console.log(`Done: ${sig}`);
  }

  async function actionRemoveLiquidity() {
    const amountSol = Number(requireEnv("AMOUNT"));
    const lamports = new anchor.BN(Math.round(amountSol * anchor.web3.LAMPORTS_PER_SOL));
    console.log(`Removing ${amountSol} SOL liquidity...`);
    const sig = await program.methods
      .removeLiquidity(lamports)
      .accounts({
        globalState,
        escrowVault,
        operator: wallet,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .rpc();
    console.log(`Done: ${sig}`);
  }

  async function actionAll() {
    await actionInitialize();

    for (const net of NETWORK_REGISTRY) {
      const pda = networkPda(program, net.id);
      const existing = await provider.connection.getAccountInfo(pda);
      if (existing) {
        console.log(`  network ${net.id} (${net.name}) already registered, skipping`);
        continue;
      }
      console.log(`  registering network ${net.id} (${net.name}), addrLen=${net.addrLen}`);
      const sig = await program.methods
        .addNetwork(new anchor.BN(net.id), net.addrLen, net.addrLen)
        .accounts({
          globalState,
          networkConfig: pda,
          admin: wallet,
          systemProgram: anchor.web3.SystemProgram.programId,
        })
        .rpc();
      console.log(`    ${sig}`);
    }

    const amountEnv = process.env.AMOUNT;
    if (amountEnv && Number(amountEnv) > 0) {
      await actionAddLiquidity();
    } else {
      console.log("  AMOUNT not set (or 0) — skipping addLiquidity");
    }

    console.log("Done.");
  }

  const ACTIONS: Record<string, () => Promise<void>> = {
    status: actionStatus,
    initialize: actionInitialize,
    "add-network": actionAddNetwork,
    "update-network": actionUpdateNetwork,
    "add-liquidity": actionAddLiquidity,
    "remove-liquidity": actionRemoveLiquidity,
    all: actionAll,
  };

  const action = process.env.ACTION ?? "all";
  const fn = ACTIONS[action];
  if (!fn) {
    throw new Error(`Unknown ACTION "${action}". Valid: ${Object.keys(ACTIONS).join(", ")}`);
  }
  await fn();
}

main().catch((err) => {
  console.error(err);
  process.exitCode = 1;
});
