//! Defending a proof on real Bitcoin against a challenge. The operator must
//! (D90, D99); an attester, whose money is locked for the job, does too.
//! Anyone may show a parent and add blocks to either side.

use ipow_bitcoin::view::BitcoinView;
use ipow_protocol_core::network::ProtocolNetwork;
use ipow_protocol_core::types::{BlockRef, ChallengeKind, EPOCH_BLOCKS};
use tracing::{info, warn};

use crate::bitcoin::{STEP, crossing, stream_real};
use crate::light::epoch_time_at;

/// Does what a challenge needs now. `shown` is the last block shown on the
/// operator's side of a competing-branch challenge (the proof's tip until
/// something is shown). Returns true when the challenge is over.
pub async fn defend(net: &dyn ProtocolNetwork, btc: &dyn BitcoinView, id: u64, shown: &mut Option<BlockRef>, now: i64) -> anyhow::Result<bool> {
    let Some(c) = net.challenge(id).await? else {
        return Ok(true);
    };
    let job = net.job(c.job_id).await?;
    match c.kind {
        ChallengeKind::Parent => {
            let asked = c.asked.ok_or_else(|| anyhow::anyhow!("a question without its block"))?;
            let prev = net.stored_block(&asked).await?.ok_or_else(|| anyhow::anyhow!("the asked block is not stored"))?.prev_hash;
            let header = btc.header(&prev).await?;
            let prev_epoch_time = if asked.height % EPOCH_BLOCKS == 0 { epoch_time_at(btc, asked.height - 1).await? } else { 0 };
            net.extend_back(&asked, &header, prev_epoch_time).await?;
            net.show_parent(id, prev_epoch_time).await?;
            info!(network = net.name(), challenge = id, job = c.job_id, "parent shown");
            Ok(true)
        }
        ChallengeKind::Fork => {
            if now >= job.lock_end {
                net.resolve_challenge(id).await?;
                info!(network = net.name(), challenge = id, "challenge resolved");
                return Ok(true);
            }
            // Grow from the last block shown; when a reorganisation took it
            // away, from the proof's tip or block, which the contract keeps
            // as checkpoints of this side.
            let mut start = None;
            for b in [*shown, job.tip, job.proof_block].into_iter().flatten() {
                if let Some(h) = btc.best_chain_height(&b.hash).await? {
                    start = Some((b, h));
                    break;
                }
            }
            let Some((mut from, real)) = start else {
                warn!(network = net.name(), challenge = id, "the proof's branch left Bitcoin's best chain");
                return Ok(false);
            };
            // In steps the light client can walk: after hours away the gap
            // can be larger than one walk.
            let mut more = btc.tip_height().await?.saturating_sub(real);
            while more > 0 {
                let step = more.min(STEP);
                let Some(refs) = stream_real(net, btc, &from, step).await? else { break };
                let new_tip = *refs.last().unwrap();
                net.extend_branch(id, false, &from, &new_tip, crossing(&[from, new_tip])).await?;
                from = new_tip;
                *shown = Some(new_tip);
                more -= step;
            }
            Ok(false)
        }
    }
}
