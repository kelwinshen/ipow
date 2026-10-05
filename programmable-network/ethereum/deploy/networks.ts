// What each EVM network's deployment of the protocol differs by (D134):
// every network runs the contracts of this package, built once; a network
// is only these settings. A value that is not known yet is null, and the
// deployment refuses to run with it (see deploy.ts). Real deployed addresses
// are not here; each network's README has them.

export type Env = "testnet" | "mainnet";

export type Coin =
  /** The network's native coin (D136), with its decimals as a contract sees
   *  them, and where its price of work is read: the base fee, or on Hedera,
   *  where that reads zero, the transaction's gas price (D139). */
  | { kind: "native"; decimals: number; price: "baseFee" | "gasPrice" }
  /** A token, on a network without a native coin (D136, D137). `priceScale`
   *  is the units of the gas price per unit of the token. */
  | { kind: "token"; token: Record<Env, string | null>; priceScale: bigint };

export type NetworkSettings = {
  /** Fixed network number (D133). */
  number: number;
  chainId: Record<Env, number | null>;
  /** Names of the variables in the network package's git-ignored .env
   *  (programmable-network/<network>/.env) that hold its RPC endpoints and
   *  its deployer's key. */
  rpcEnv: Record<Env, string>;
  keyEnv: Record<Env, string>;
  coin: Coin;
  /** The reader of a rollup's price of data (D135). */
  dataFee: "none" | "op" | "arb";
  /** A vault's flat deposit (D121) and the minimum escrow of a certifying
   *  job (D118), in units of the coin. Test networks are deployed with small
   *  amounts (D121). */
  vault: Record<Env, { deposit: bigint; minCertifyingEscrow: bigint } | null>;
  /** A build of its own whose price of work is scaled, as the network's
   *  gas is not Ethereum's (D140): its name, and the scale it holds. */
  scaledBuild?: { name: "iPoWProtocolPolkadot"; workScale: bigint };
  /** Deploying needs blocks larger than the network's usual one. */
  bigBlocks?: boolean;
  /** Why the protocol must not be deployed there yet, if so. */
  blocked?: string;
  /** A network whose transactions ethers cannot send. */
  sender?: "viem-tempo";
};

const ETH = 10n ** 18n;
const USD = 10n ** 6n;
/** HBAR as a contract counts it: tinybars. */
const HBAR = 10n ** 8n;

/** Ethereum's mainnet amounts, D118 and D121. */
const ETHEREUM_MAINNET = { deposit: (3n * ETH) / 100n, minCertifyingEscrow: ETH };
/** Small amounts for a test network in a coin of 18 decimals. */
const TEST_18 = { deposit: ETH / 1000n, minCertifyingEscrow: ETH / 100n };

export const NETWORKS: Record<string, NetworkSettings> = {
  ethereum: {
    number: 1,
    chainId: { testnet: 11155111, mainnet: 1 },
    rpcEnv: { testnet: "SEPOLIA_RPC_URL", mainnet: "ETHEREUM_RPC_URL" },
    keyEnv: { testnet: "SEPOLIA_PRIVATE_KEY", mainnet: "ETHEREUM_PRIVATE_KEY" },
    coin: { kind: "native", decimals: 18, price: "baseFee" },
    dataFee: "none",
    vault: { testnet: TEST_18, mainnet: ETHEREUM_MAINNET },
  },
  base: {
    number: 3,
    chainId: { testnet: 84532, mainnet: 8453 },
    rpcEnv: { testnet: "BASE_SEPOLIA_RPC_URL", mainnet: "BASE_RPC_URL" },
    keyEnv: { testnet: "BASE_SEPOLIA_PRIVATE_KEY", mainnet: "BASE_PRIVATE_KEY" },
    coin: { kind: "native", decimals: 18, price: "baseFee" },
    // GasPriceOracle predeploy (D135).
    dataFee: "op",
    vault: { testnet: TEST_18, mainnet: null },
  },
  robinhood: {
    number: 4,
    // Testnet read from its endpoint on 2026-10-02.
    chainId: { testnet: 46630, mainnet: null },
    rpcEnv: { testnet: "ROBINHOOD_TESTNET_RPC_URL", mainnet: "ROBINHOOD_RPC_URL" },
    keyEnv: { testnet: "ROBINHOOD_TESTNET_PRIVATE_KEY", mainnet: "ROBINHOOD_PRIVATE_KEY" },
    coin: { kind: "native", decimals: 18, price: "baseFee" },
    // ArbGasInfo precompile (D135).
    dataFee: "arb",
    vault: { testnet: TEST_18, mainnet: null },
  },
  polkadot: {
    number: 5,
    rpcEnv: { testnet: "POLKADOT_TESTNET_RPC_URL", mainnet: "POLKADOT_RPC_URL" },
    keyEnv: { testnet: "POLKADOT_TESTNET_PRIVATE_KEY", mainnet: "POLKADOT_PRIVATE_KEY" },
    coin: { kind: "native", decimals: 18, price: "baseFee" },
    dataFee: "none",
    vault: { testnet: TEST_18, mainnet: null },
    // Testnet read from its endpoint on 2026-10-02.
    chainId: { testnet: 420420417, mainnet: null },
    // Measured on its testnet on 2026-10-02: a contract sees the base fee
    // (10^12) and the coin in 18 decimals; the light client's work took
    // 0.08 to 0.124 of Ethereum's gas (the epoch start 0.122, streaming 1, 10
    // and 30 headers 0.107, 0.122 and 0.124, the jump 0.082), and the coin
    // spent was gas x price (D140).
    scaledBuild: { name: "iPoWProtocolPolkadot", workScale: 8n },
  },
  hedera: {
    number: 6,
    chainId: { testnet: 296, mainnet: 295 },
    rpcEnv: { testnet: "HEDERA_TESTNET_RPC_URL", mainnet: "HEDERA_RPC_URL" },
    keyEnv: { testnet: "HEDERA_TESTNET_PRIVATE_KEY", mainnet: "HEDERA_PRIVATE_KEY" },
    // Measured on its testnet on 2026-10-02 (NetworkProbe): a contract sees a
    // base fee of 0, the transaction's gas price in tinybars (80), and HBAR in
    // 8 decimals (1 HBAR sent arrived as 10^8), while its RPC shows 18 (D139).
    coin: { kind: "native", decimals: 8, price: "gasPrice" },
    dataFee: "none",
    vault: { testnet: { deposit: HBAR / 10n, minCertifyingEscrow: HBAR }, mainnet: null },
  },
  hyperliquid: {
    number: 7,
    // Each read from its network's endpoint on 2026-10-02.
    chainId: { testnet: 998, mainnet: 999 },
    rpcEnv: { testnet: "HYPEREVM_TESTNET_RPC_URL", mainnet: "HYPEREVM_RPC_URL" },
    keyEnv: { testnet: "HYPEREVM_TESTNET_PRIVATE_KEY", mainnet: "HYPEREVM_PRIVATE_KEY" },
    coin: { kind: "native", decimals: 18, price: "baseFee" },
    dataFee: "none",
    vault: { testnet: TEST_18, mainnet: null },
    // Small blocks hold 3M gas; the protocol takes about 5.4M to deploy and
    // a vault about 10.5M, so the deployer's address must use the 30M big
    // blocks (measured on its testnet on 2026-10-02).
    bigBlocks: true,
  },
  arbitrum: {
    // D141. Arbitrum One and Arbitrum Sepolia; Sepolia's chain id read from
    // its endpoint on 2026-10-06.
    number: 9,
    chainId: { testnet: 421614, mainnet: 42161 },
    rpcEnv: { testnet: "ARBITRUM_SEPOLIA_RPC_URL", mainnet: "ARBITRUM_RPC_URL" },
    keyEnv: { testnet: "ARBITRUM_SEPOLIA_PRIVATE_KEY", mainnet: "ARBITRUM_PRIVATE_KEY" },
    coin: { kind: "native", decimals: 18, price: "baseFee" },
    // ArbGasInfo precompile (D135), as on Robinhood; read on Arbitrum
    // Sepolia on 2026-10-06 (getPricesInWei answered).
    dataFee: "arb",
    vault: { testnet: TEST_18, mainnet: null },
  },
  tempo: {
    number: 8,
    chainId: { testnet: 42431, mainnet: null },
    rpcEnv: { testnet: "TEMPO_TESTNET_RPC_URL", mainnet: "TEMPO_RPC_URL" },
    keyEnv: { testnet: "TEMPO_TESTNET_PRIVATE_KEY", mainnet: "TEMPO_PRIVATE_KEY" },
    // PathUSD, 6 decimals, read on its testnet on 2026-10-02. The gas price
    // is in attodollars: a sender pays gas x price / 10^12 PathUSD, checked
    // against real receipts on its testnet the same day.
    coin: { kind: "token", token: { testnet: "0x20C0000000000000000000000000000000000000", mainnet: null }, priceScale: 10n ** 12n },
    dataFee: "none",
    vault: { testnet: { deposit: USD / 10n, minCertifyingEscrow: USD }, mainnet: null },
    // Tempo's own transactions (implementation doc, Tempo section).
    sender: "viem-tempo",
  },
};
