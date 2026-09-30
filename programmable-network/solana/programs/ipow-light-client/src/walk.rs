use anchor_lang::prelude::*;

use crate::bitcoin::{retarget, work, U256, EPOCH_BLOCKS};
use crate::constants::{CONFIG_SEED, MAX_WALK, NODE_SEED, WALK_SEED};
use crate::errors::LightClientError;
use crate::state::{BlockRef, Config, Walk};
use crate::utils::{load_node, target};

/// Starts a walk down from `descendant` to `ancestor`. The walk finishes
/// only if it reaches exactly that block, as `walk` in the Ethereum contract
/// returns true only then.
/// The walk is done by `walk_step` calls, a few blocks each, because a
/// Solana transaction cannot hold 100 blocks. The checks are those of
/// `walk` in `iPoWLightClient.sol`.
pub fn begin(
    ctx: Context<BeginWalk>,
    _walk_id: u64,
    descendant: BlockRef,
    ancestor: BlockRef,
    prev_epoch_time: u32,
) -> Result<()> {
    require!(descendant.height >= ancestor.height, LightClientError::InvalidHeight);
    require!(descendant.height - ancestor.height <= MAX_WALK, LightClientError::WalkTooLong);
    let walk = &mut ctx.accounts.walk;
    walk.owner = ctx.accounts.payer.key();
    walk.descendant = descendant;
    walk.prev_epoch_time = prev_epoch_time;
    walk.ancestor = ancestor;
    walk.next = descendant;
    walk.child_bits = 0;
    walk.child_first_of_epoch = false;
    walk.read_any = false;
    walk.finished = false;
    walk.last = BlockRef::default();
    walk.work = [0u8; 32];
    walk.bump = ctx.bumps.walk;
    Ok(())
}

/// Reads the next blocks of a walk, given as the remaining accounts in order,
/// from the newest down. About 25 blocks fit one transaction. Every block must be stored, and have the difficulty
/// Bitcoin's rules give it after the block before it (D72). Anyone may call.
pub fn step<'info>(ctx: Context<'info, WalkStep<'info>>) -> Result<()> {
    let walk = &mut ctx.accounts.walk;
    let pow_limit = U256::from_be_bytes(&ctx.accounts.config.pow_limit);
    let mut total = U256::from_be_bytes(&walk.work);

    for account in ctx.remaining_accounts.iter() {
        require!(!walk.finished, LightClientError::WalkFinished);
        let next = walk.next;
        let node = load_node(ctx.program_id, account)?.ok_or(LightClientError::UnknownBlock)?;
        let address = Pubkey::create_program_address(
            &[NODE_SEED, &next.hash, &next.height.to_le_bytes(), &next.epoch_time.to_le_bytes(), &[node.bump]],
            ctx.program_id,
        )
        .map_err(|_| error!(LightClientError::WrongAccount))?;
        require_keys_eq!(account.key(), address, LightClientError::WrongAccount);

        // The first block of an epoch gives the epoch its time.
        require!(
            next.height % EPOCH_BLOCKS != 0 || node.time == next.epoch_time,
            LightClientError::EpochTimeMismatch
        );
        if walk.read_any {
            let expected = if walk.child_first_of_epoch {
                retarget(node.bits, next.epoch_time, node.time, &pow_limit)
                    .map_err(|_| error!(LightClientError::InvalidBits))?
            } else {
                node.bits
            };
            require!(expected == walk.child_bits, LightClientError::DifficultyMismatch);
        }

        walk.last = next;
        walk.read_any = true;
        if next.height == walk.ancestor.height {
            // The block reached must be the stated ancestor.
            require!(next == walk.ancestor, LightClientError::NotLinked);
            walk.finished = true;
            break;
        }

        total = total
            .checked_add(&work(&target(node.bits)?))
            .ok_or(LightClientError::InvalidBits)?;
        walk.child_bits = node.bits;
        walk.child_first_of_epoch = next.height % EPOCH_BLOCKS == 0;
        walk.next = BlockRef {
            hash: node.prev_hash,
            height: next.height - 1,
            epoch_time: if walk.child_first_of_epoch { walk.prev_epoch_time } else { next.epoch_time },
        };
    }
    walk.work = total.to_be_bytes();
    Ok(())
}

#[derive(Accounts)]
#[instruction(walk_id: u64)]
pub struct BeginWalk<'info> {
    #[account(
        init,
        payer = payer,
        space = 8 + Walk::INIT_SPACE,
        seeds = [WALK_SEED, payer.key().as_ref(), &walk_id.to_le_bytes()],
        bump
    )]
    pub walk: Account<'info, Walk>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct WalkStep<'info> {
    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, Config>,
    #[account(mut)]
    pub walk: Account<'info, Walk>,
}

#[derive(Accounts)]
pub struct CloseWalk<'info> {
    #[account(mut, close = owner, has_one = owner)]
    pub walk: Account<'info, Walk>,
    #[account(mut)]
    pub owner: Signer<'info>,
}
