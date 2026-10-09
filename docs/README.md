# Documentation

```
docs/
├── design/
│   └── ipow-protocol.md   — canonical: the protocol (light client, operators, jobs, claims, vault), its decisions, build status
├── specs/                 — specs of the built applications and features
├── drafts/                — the build plan, dated live-run records, unratified proposals
└── archive/               — the retired generation's design docs and other history
```

- **`design/ipow-protocol.md`** is the source of truth. Changes to it
  require review, see [`.github/CODEOWNERS`](../.github/CODEOWNERS).
  Reason and implement *from* it. Its "Build status" section says what is
  built, tested and deployed.
- **`specs/`** holds the specs of what is built on or added to the
  protocol, cited from the code; each states its own approval and status. Also
  review-gated in CODEOWNERS:
  - [`ipow-conversion-app.md`](specs/ipow-conversion-app.md) — Conversion: a network's coin or a token swapped for real BTC and back.
  - [`ipow-conversion-tunnel.md`](specs/ipow-conversion-tunnel.md) — conversion between two programmable networks: two Conversion legs linked by one Bitcoin payment.
  - [`ipow-beta-app.md`](specs/ipow-beta-app.md) — BETA: a token backed by a basket of vault receipts.
  - [`ipow-vault-claims.md`](specs/ipow-vault-claims.md) — the discussion behind the vault and its receipts (now section 11 of the design doc, which is authoritative).
  - [`ipow-vault-genesis.md`](specs/ipow-vault-genesis.md) — the testnet vaults' one-time genesis step.
  - [`ipow-stage7-networks.md`](specs/ipow-stage7-networks.md) — what each network beyond Ethereum and Solana needs.
  - [`ipow-sdk.md`](specs/ipow-sdk.md) — the TypeScript SDK.
- **`drafts/`** is not authoritative: the build plan, the dated live-run
  records, and any proposal not yet built. See
  [`drafts/README.md`](drafts/README.md).
- **`archive/`** is not authoritative: the design docs of the generation
  retired at the git tag `legacy-v1`
  ([`ipow.md`](archive/ipow.md),
  [`ipow-implementation.md`](archive/ipow-implementation.md)) and the
  earlier designs they replaced
  ([`superseded-beta-designs.md`](archive/superseded-beta-designs.md),
  [`abandoned-cpi-funding-attempt.md`](archive/abandoned-cpi-funding-attempt.md)).
  Kept for history.

## Where to start

- **[Root README](../README.md)** — what iPoW is, the repository layout,
  how to build and test.
- **[`design/ipow-protocol.md`](design/ipow-protocol.md)** — the
  protocol, section by section, and the decision index (D1 onwards).
- **[`SECURITY.md`](../SECURITY.md)** — the present trust model: what is
  permissionless today and what the deployer still controls.
- **[`CLAUDE.md`](../CLAUDE.md)** (repo root) — agent and contributor
  rules and the source-of-truth map.
- **[Contributing](../CONTRIBUTING.md)** — setup, PR process, code style.

## Per-package and per-network docs

[`evm`](../evm/README.md), [`solana`](../solana/README.md),
[`node`](../node/README.md) and [`sdk`](../sdk/README.md) each have a
README with setup, build and test. Deployed addresses are in each
network's README: Sepolia in [`evm/README.md`](../evm/README.md), Solana
devnet in [`solana/README.md`](../solana/README.md), the others in
`networks/<network>/README.md`; the records they follow are
`evm/deployments/<network>-testnet.json` and
`solana/deployments/devnet.json`. They are not in the design docs.
