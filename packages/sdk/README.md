# @ipow/sdk

Build on the iPoW protocol ([`docs/design/ipow-protocol.md`](../../docs/design/ipow-protocol.md)).
Design and plan: [`docs/drafts/ipow-sdk.md`](../../docs/drafts/ipow-sdk.md).

**Status:** step 1 of the plan, on EVM networks: each deployed network as
data, quotes with their margin stated, jobs by stage with their times and
what they wait for, the job's Bitcoin transaction (found before its proof,
through the operator's chain head and the job's tag), and opening a vault
checkpoint. Tested on a local chain (`programmable-network/ethereum/test/Sdk.test.ts`),
and read against the first live job on Sepolia. The vault, Solana, BETA and
Conversion follow, in that order.

```ts
import { network, quoteCheckpoint, openCheckpoint, getJob, findJobTx } from "@ipow/sdk";

const d = network("ethereum-testnet");
const quote = await quoteCheckpoint(provider, d);   // fee, escrow, margin, total
const { jobId } = await openCheckpoint(signer, d, quote);
const job = await getJob(provider, d, jobId);        // job.stage, job.waitingFor, job.times
const btc = await findJobTx(provider, d, job);       // btc.confirmations of job.confirmations, btc.link
```

The SDK never holds keys: it takes the app's provider and signer. After a
deployment or a contract change, `node scripts/sync.ts` regenerates its
networks and ABIs from this repository.
