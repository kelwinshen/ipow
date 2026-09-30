//! The challenge of a proof (D49, D81, D88, D90 to D92, D98, D99). The same
//! rules as the Ethereum contract. The mining work of a branch comes from
//! finished walks of the light client.

use anchor_lang::prelude::*;

use crate::constants::*;
use crate::duty::{credit_to, read_node, read_walk, slash, status, SlashAccounts, Status};
use crate::errors::ProtocolError;
use crate::money::pay_in;
use crate::notes::require_note;
use crate::state::{BlockRef, Challenge, ChallengeKind, Checkpoint, Job, Note, Operator, Protocol};
use ipow_light_client::bitcoin::{retarget, target_from_bits, work, U256, EPOCH_BLOCKS};
use ipow_light_client::state::Config as LcConfig;

/// What a guardian's note holds when it asks for the parent of a block.
pub fn parent_evidence(child: &[u8; 32]) -> [u8; 32] {
    crate::duty::sha256(&[b"parent", child])
}

/// What a guardian's note holds when it shows a branch starting with `block`.
pub fn fork_evidence(block: &[u8; 32]) -> [u8; 32] {
    crate::duty::sha256(&[b"fork", block])
}

/// The deposit of the next question for a parent (D91).
pub fn parent_deposit(job: &Job) -> u64 {
    job.commitment_fee.saturating_mul(job.parents_shown as u64 + 1)
}

fn block_work(bits: u32) -> Result<U256> {
    Ok(work(&target_from_bits(bits).map_err(|_| error!(ProtocolError::UnknownBlock))?))
}

/// The checks every challenge starts with (D81, D88, D99).
fn open(job: &mut Job, protocol: &mut Protocol, deposit: u64, paid: u64) -> Result<u64> {
    let now = Clock::get()?.unix_timestamp;
    require!(status(job, now) == Status::Proven, ProtocolError::NotProven);
    // D99: the lock never moves, and the last challenge opens 12 hours
    // before it ends, so every challenge is decided within it.
    require!(now + RESPONSE_TIME <= job.lock_end, ProtocolError::ChallengeWindowClosed);
    require!(paid == deposit, ProtocolError::WrongDeposit);
    protocol.challenge_count += 1;
    job.open_challenges += 1;
    Ok(protocol.challenge_count)
}

/// A guardian asks for the parent of the oldest block of the operator's
/// branch. Anyone may show it within 12 hours. If nobody does, the proof is
/// false (D81, D90). Question k costs k times the commitment fee (D91).
pub fn ask_parent(ctx: Context<AskParent>, note: [u8; 32], salt: [u8; 32], paid: u64) -> Result<()> {
    let job = &mut ctx.accounts.job;
    require!(job.parents_shown < MAX_PARENT_QUESTIONS, ProtocolError::TooManyQuestions);
    let guardian = ctx.accounts.guardian.key();
    require_note(&ctx.accounts.note_record, &note, &guardian, job.id, &parent_evidence(&job.deepest.hash), &salt)?;
    let deposit = parent_deposit(job);
    let id = open(job, &mut ctx.accounts.protocol, deposit, paid)?;
    pay_in(
        &ctx.accounts.guardian.to_account_info(),
        &ctx.accounts.vault.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
        paid,
    )?;
    let c = &mut ctx.accounts.challenge;
    c.id = id;
    c.kind = ChallengeKind::Parent;
    c.job_id = job.id;
    c.guardian = guardian;
    c.deposit = deposit;
    c.opened_at = Clock::get()?.unix_timestamp;
    c.asked = job.deepest;
    c.bump = ctx.bumps.challenge;
    emit!(crate::ParentAsked { challenge_id: id, job_id: job.id, guardian, child_hash: job.deepest.hash });
    Ok(())
}

/// Answers a question: the parent is in the light client, and the block
/// asked about has the difficulty that follows from it. Anyone may call.
/// The deposit goes to the operator (D90). `prev_epoch_time` is only used
/// when the block asked about is the first of its epoch.
pub fn show_parent(ctx: Context<ShowParent>, prev_epoch_time: u32) -> Result<()> {
    let c = &ctx.accounts.challenge;
    require!(c.kind == ChallengeKind::Parent, ProtocolError::NoChallenge);
    let job = &mut ctx.accounts.job;
    let now = Clock::get()?.unix_timestamp;
    require!(status(job, now) == Status::Proven, ProtocolError::NotProven);
    require!(now < c.opened_at + RESPONSE_TIME, ProtocolError::ResponseTimeOver);

    let asked = c.asked;
    // Nobody can show a block before height 0.
    require!(asked.height > 0, ProtocolError::NoParent);
    let child = read_node(&ctx.accounts.child, &asked)?;
    let first_of_epoch = asked.height % EPOCH_BLOCKS == 0;
    let parent_ref = BlockRef {
        hash: child.prev_hash,
        height: asked.height - 1,
        epoch_time: if first_of_epoch { prev_epoch_time } else { asked.epoch_time },
    };
    let parent = read_node(&ctx.accounts.parent, &parent_ref).map_err(|_| error!(ProtocolError::NoParent))?;
    // The child has the difficulty that follows from the parent (D72).
    let pow_limit = U256::from_be_bytes(&ctx.accounts.lc_config.pow_limit);
    let expected = if first_of_epoch {
        retarget(parent.bits, prev_epoch_time, parent.time, &pow_limit).map_err(|_| error!(ProtocolError::NoParent))?
    } else {
        parent.bits
    };
    require!(expected == child.bits, ProtocolError::NoParent);
    // The first block of an epoch gives the epoch its time.
    require!(
        parent_ref.height % EPOCH_BLOCKS != 0 || parent.time == parent_ref.epoch_time,
        ProtocolError::NoParent
    );

    if job.deepest == asked {
        job.deepest = parent_ref;
        job.parents_shown += 1;
    }
    emit!(crate::ParentShown { challenge_id: c.id, parent_hash: parent_ref.hash });
    fail(job, c, &ctx.accounts.operator_credit, &ctx.accounts.funder, &ctx.accounts.system_program)
}

/// D81, D90: the deposit of a challenge that failed goes to the operator.
fn fail<'info>(
    job: &mut Job,
    c: &Challenge,
    operator_credit: &UncheckedAccount<'info>,
    funder: &Signer<'info>,
    system: &Program<'info, System>,
) -> Result<()> {
    job.open_challenges -= 1;
    emit!(crate::ChallengeFailed { challenge_id: c.id, guardian: c.guardian });
    credit_to(
        &operator_credit.to_account_info(),
        &job.operator,
        c.deposit,
        &funder.to_account_info(),
        &system.to_account_info(),
    )
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy)]
pub struct ForkArgs {
    /// A block of the operator's branch, from the anchor to the proof's block.
    pub operator_block: BlockRef,
    /// Another block with the same parent.
    pub guardian_block: BlockRef,
    /// The best block on top of `guardian_block`.
    pub guardian_tip: BlockRef,
}

/// A guardian shows a competing branch: a block that names the same parent
/// as a block of the operator's branch, with more mining work on top of it
/// than the operator's branch has from there (D81). The branches may part
/// at the anchor or after it. The work comes from three finished walks: the
/// proof's block down to `operator_block` (which also shows that block is
/// on the operator's branch), the tip down to the proof's block, and the
/// guardian's tip down to `guardian_block`.
pub fn challenge_fork(ctx: Context<ChallengeFork>, note: [u8; 32], salt: [u8; 32], paid: u64, args: ForkArgs) -> Result<()> {
    let job = &mut ctx.accounts.job;
    let guardian = ctx.accounts.guardian.key();
    require_note(&ctx.accounts.note_record, &note, &guardian, job.id, &fork_evidence(&args.guardian_block.hash), &salt)?;
    let deposit = job.commitment_fee;
    let id = open(job, &mut ctx.accounts.protocol, deposit, paid)?;
    pay_in(
        &ctx.accounts.guardian.to_account_info(),
        &ctx.accounts.vault.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
        paid,
    )?;

    // D81: at the anchor or after it, on the operator's branch.
    require!(args.operator_block.height >= job.anchor.height, ProtocolError::NotOnOperatorBranch);
    let to_op = read_walk(&ctx.accounts.walk_proof_to_operator, &job.proof_block, &args.operator_block)
        .map_err(|_| error!(ProtocolError::NotOnOperatorBranch))?;
    let to_tip = read_walk(&ctx.accounts.walk_tip_to_proof, &job.tip, &job.proof_block)?;
    let guardian_walk = read_walk(&ctx.accounts.walk_guardian, &args.guardian_tip, &args.guardian_block)?;

    // Two different blocks that name the same parent.
    let op_node = read_node(&ctx.accounts.operator_node, &args.operator_block)?;
    let g_node = read_node(&ctx.accounts.guardian_node, &args.guardian_block)?;
    require!(op_node.hash != g_node.hash && op_node.prev_hash == g_node.prev_hash, ProtocolError::NotCompeting);

    let add = |a: U256, b: U256| a.checked_add(&b).ok_or(error!(ProtocolError::Overflow));
    let operator_work = add(
        add(block_work(op_node.bits)?, U256::from_be_bytes(&to_op.work))?,
        U256::from_be_bytes(&to_tip.work),
    )?;
    let guardian_work = add(block_work(g_node.bits)?, U256::from_be_bytes(&guardian_walk.work))?;
    require!(guardian_work.gt(&operator_work), ProtocolError::NotHeavier);

    let c = &mut ctx.accounts.challenge;
    c.id = id;
    c.kind = ChallengeKind::Fork;
    c.job_id = job.id;
    c.guardian = guardian;
    c.deposit = deposit;
    c.opened_at = Clock::get()?.unix_timestamp;
    c.operator_work = operator_work.to_be_bytes();
    c.guardian_work = guardian_work.to_be_bytes();
    c.bump = ctx.bumps.challenge;
    ctx.accounts.operator_checkpoint.work = operator_work.to_be_bytes();
    ctx.accounts.operator_checkpoint.bump = ctx.bumps.operator_checkpoint;
    ctx.accounts.guardian_checkpoint.work = guardian_work.to_be_bytes();
    ctx.accounts.guardian_checkpoint.bump = ctx.bumps.guardian_checkpoint;
    // As on Ethereum: each side can also grow from its first shown block, in
    // case the tip shown with it ends on a branch Bitcoin abandoned.
    let to_proof = add(block_work(op_node.bits)?, U256::from_be_bytes(&to_op.work))?;
    ctx.accounts.proof_checkpoint.work = to_proof.to_be_bytes();
    ctx.accounts.proof_checkpoint.bump = ctx.bumps.proof_checkpoint;
    ctx.accounts.guardian_first_checkpoint.work = block_work(g_node.bits)?.to_be_bytes();
    ctx.accounts.guardian_first_checkpoint.bump = ctx.bumps.guardian_first_checkpoint;
    emit!(crate::ForkChallenged { challenge_id: id, job_id: job.id, guardian });
    Ok(())
}

/// Adds blocks to one side of an open competing-branch challenge, on top of
/// any block shown before on that side. Anyone may call, until the lock ends
/// (D81, D99). `walk` is a finished walk from `new_tip` down to `from`.
pub fn extend_branch(ctx: Context<ExtendBranch>, guardian_side: bool, from: BlockRef, new_tip: BlockRef) -> Result<()> {
    let c = &mut ctx.accounts.challenge;
    require!(c.kind == ChallengeKind::Fork, ProtocolError::NoChallenge);
    let job = &ctx.accounts.job;
    let now = Clock::get()?.unix_timestamp;
    require!(status(job, now) == Status::Proven, ProtocolError::NotProven);
    require!(now < job.lock_end, ProtocolError::LockEnded);

    let walk = read_walk(&ctx.accounts.walk, &new_tip, &from)?;
    let total = U256::from_be_bytes(&ctx.accounts.from_checkpoint.work)
        .checked_add(&U256::from_be_bytes(&walk.work))
        .ok_or(ProtocolError::Overflow)?;
    let cp = &mut ctx.accounts.new_checkpoint;
    if total.gt(&U256::from_be_bytes(&cp.work)) {
        cp.work = total.to_be_bytes();
    }
    cp.bump = ctx.bumps.new_checkpoint;
    let side = if guardian_side { &mut c.guardian_work } else { &mut c.operator_work };
    if total.gt(&U256::from_be_bytes(side)) {
        *side = total.to_be_bytes();
    }
    emit!(crate::BranchExtended { challenge_id: c.id, guardian_side, tip_hash: new_tip.hash });
    Ok(())
}

/// Ends an open challenge. Anyone may call. A question not answered in 12
/// hours: the proof is false. A competing branch: the side with more work
/// when the lock has ended wins, the operator's side when equal. When
/// another challenge already slashed the job, the deposit goes back (D88).
/// Whoever first resolves a winning challenge gets the 20% (D98).
pub fn resolve(ctx: Context<ResolveChallenge>) -> Result<()> {
    let c = &ctx.accounts.challenge;
    let job = &mut ctx.accounts.job;
    let now = Clock::get()?.unix_timestamp;
    let funder = ctx.accounts.funder.to_account_info();
    let system = ctx.accounts.system_program.to_account_info();

    if job.slashed {
        job.open_challenges -= 1;
        emit!(crate::ChallengeRefunded { challenge_id: c.id, guardian: c.guardian });
        return credit_to(&ctx.accounts.guardian_credit.to_account_info(), &c.guardian, c.deposit, &funder, &system);
    }

    let proof_is_false = match c.kind {
        ChallengeKind::Parent => {
            require!(now >= c.opened_at + RESPONSE_TIME, ProtocolError::ResponseTimeNotOver);
            true
        }
        ChallengeKind::Fork => {
            require!(now >= job.lock_end, ProtocolError::LockNotEnded);
            U256::from_be_bytes(&c.guardian_work).gt(&U256::from_be_bytes(&c.operator_work))
        }
    };
    if !proof_is_false {
        return fail(job, c, &ctx.accounts.operator_credit, &ctx.accounts.funder, &ctx.accounts.system_program);
    }

    job.open_challenges -= 1;
    emit!(crate::ChallengeWon { challenge_id: c.id, guardian: c.guardian });
    let attester = ctx.accounts.attester.as_mut().map(|a| &mut **a);
    slash(
        job,
        &c.guardian,
        SlashAccounts {
            operator: &mut ctx.accounts.operator,
            attester,
            application_credit: &ctx.accounts.application_credit.to_account_info(),
            guardian_credit: &ctx.accounts.guardian_credit.to_account_info(),
            payer_credit: &ctx.accounts.payer_credit.to_account_info(),
            funder: &funder,
            system_program: &system,
        },
    )?;
    credit_to(&ctx.accounts.guardian_credit.to_account_info(), &c.guardian, c.deposit, &funder, &system)
}

#[derive(Accounts)]
#[instruction(note: [u8; 32])]
pub struct AskParent<'info> {
    #[account(mut, seeds = [PROTOCOL_SEED], bump = protocol.bump)]
    pub protocol: Account<'info, Protocol>,
    #[account(mut, seeds = [JOB_SEED, &job.id.to_le_bytes()], bump = job.bump)]
    pub job: Account<'info, Job>,
    #[account(
        init,
        payer = guardian,
        space = 8 + Challenge::INIT_SPACE,
        seeds = [CHALLENGE_SEED, &(protocol.challenge_count + 1).to_le_bytes()],
        bump
    )]
    pub challenge: Account<'info, Challenge>,
    #[account(mut, close = guardian, seeds = [NOTE_SEED, &note], bump = note_record.bump)]
    pub note_record: Account<'info, Note>,
    /// CHECK: the protocol's vault.
    #[account(mut, seeds = [VAULT_SEED], bump = protocol.vault_bump)]
    pub vault: UncheckedAccount<'info>,
    #[account(mut)]
    pub guardian: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ShowParent<'info> {
    #[account(mut, close = guardian, seeds = [CHALLENGE_SEED, &challenge.id.to_le_bytes()], bump = challenge.bump)]
    pub challenge: Account<'info, Challenge>,
    #[account(mut, seeds = [JOB_SEED, &challenge.job_id.to_le_bytes()], bump = job.bump)]
    pub job: Account<'info, Job>,
    /// CHECK: the light client's block asked about; checked in `read_node`.
    pub child: UncheckedAccount<'info>,
    /// CHECK: the light client's block shown as its parent.
    pub parent: UncheckedAccount<'info>,
    #[account(seeds = [b"config"], bump = lc_config.bump, seeds::program = ipow_light_client::ID)]
    pub lc_config: Account<'info, LcConfig>,
    /// CHECK: the operator's credit account; checked in `credit_to`.
    #[account(mut)]
    pub operator_credit: UncheckedAccount<'info>,
    /// CHECK: receives the challenge account's rent.
    #[account(mut, address = challenge.guardian)]
    pub guardian: UncheckedAccount<'info>,
    #[account(mut)]
    pub funder: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(note: [u8; 32], salt: [u8; 32], paid: u64, args: ForkArgs)]
pub struct ChallengeFork<'info> {
    #[account(mut, seeds = [PROTOCOL_SEED], bump = protocol.bump)]
    pub protocol: Box<Account<'info, Protocol>>,
    #[account(mut, seeds = [JOB_SEED, &job.id.to_le_bytes()], bump = job.bump)]
    pub job: Box<Account<'info, Job>>,
    #[account(
        init,
        payer = guardian,
        space = 8 + Challenge::INIT_SPACE,
        seeds = [CHALLENGE_SEED, &(protocol.challenge_count + 1).to_le_bytes()],
        bump
    )]
    pub challenge: Box<Account<'info, Challenge>>,
    #[account(mut, close = guardian, seeds = [NOTE_SEED, &note], bump = note_record.bump)]
    pub note_record: Box<Account<'info, Note>>,
    /// CHECK: the light client's block of the operator's branch.
    pub operator_node: UncheckedAccount<'info>,
    /// CHECK: the light client's block of the guardian's branch.
    pub guardian_node: UncheckedAccount<'info>,
    /// CHECK: finished walks of the light client, checked in `read_walk`.
    pub walk_proof_to_operator: UncheckedAccount<'info>,
    /// CHECK: see above.
    pub walk_tip_to_proof: UncheckedAccount<'info>,
    /// CHECK: see above.
    pub walk_guardian: UncheckedAccount<'info>,
    #[account(
        init,
        payer = guardian,
        space = 8 + Checkpoint::INIT_SPACE,
        seeds = [
            CHECKPOINT_SEED,
            &(protocol.challenge_count + 1).to_le_bytes(),
            &[0],
            &job.tip.hash,
            &job.tip.height.to_le_bytes(),
            &job.tip.epoch_time.to_le_bytes()
        ],
        bump
    )]
    pub operator_checkpoint: Box<Account<'info, Checkpoint>>,
    #[account(
        init,
        payer = guardian,
        space = 8 + Checkpoint::INIT_SPACE,
        seeds = [
            CHECKPOINT_SEED,
            &(protocol.challenge_count + 1).to_le_bytes(),
            &[1],
            &args.guardian_tip.hash,
            &args.guardian_tip.height.to_le_bytes(),
            &args.guardian_tip.epoch_time.to_le_bytes()
        ],
        bump
    )]
    pub guardian_checkpoint: Box<Account<'info, Checkpoint>>,
    #[account(
        init,
        payer = guardian,
        space = 8 + Checkpoint::INIT_SPACE,
        seeds = [
            CHECKPOINT_SEED,
            &(protocol.challenge_count + 1).to_le_bytes(),
            &[0],
            &job.proof_block.hash,
            &job.proof_block.height.to_le_bytes(),
            &job.proof_block.epoch_time.to_le_bytes()
        ],
        bump
    )]
    pub proof_checkpoint: Box<Account<'info, Checkpoint>>,
    #[account(
        init,
        payer = guardian,
        space = 8 + Checkpoint::INIT_SPACE,
        seeds = [
            CHECKPOINT_SEED,
            &(protocol.challenge_count + 1).to_le_bytes(),
            &[1],
            &args.guardian_block.hash,
            &args.guardian_block.height.to_le_bytes(),
            &args.guardian_block.epoch_time.to_le_bytes()
        ],
        bump
    )]
    pub guardian_first_checkpoint: Box<Account<'info, Checkpoint>>,
    /// CHECK: the protocol's vault.
    #[account(mut, seeds = [VAULT_SEED], bump = protocol.vault_bump)]
    pub vault: UncheckedAccount<'info>,
    #[account(mut)]
    pub guardian: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(guardian_side: bool, from: BlockRef, new_tip: BlockRef)]
pub struct ExtendBranch<'info> {
    #[account(mut, seeds = [CHALLENGE_SEED, &challenge.id.to_le_bytes()], bump = challenge.bump)]
    pub challenge: Account<'info, Challenge>,
    #[account(seeds = [JOB_SEED, &challenge.job_id.to_le_bytes()], bump = job.bump)]
    pub job: Account<'info, Job>,
    #[account(
        seeds = [
            CHECKPOINT_SEED,
            &challenge.id.to_le_bytes(),
            &[guardian_side as u8],
            &from.hash,
            &from.height.to_le_bytes(),
            &from.epoch_time.to_le_bytes()
        ],
        bump = from_checkpoint.bump
    )]
    pub from_checkpoint: Account<'info, Checkpoint>,
    #[account(
        init_if_needed,
        payer = funder,
        space = 8 + Checkpoint::INIT_SPACE,
        seeds = [
            CHECKPOINT_SEED,
            &challenge.id.to_le_bytes(),
            &[guardian_side as u8],
            &new_tip.hash,
            &new_tip.height.to_le_bytes(),
            &new_tip.epoch_time.to_le_bytes()
        ],
        bump
    )]
    pub new_checkpoint: Account<'info, Checkpoint>,
    /// CHECK: a finished light client walk from `new_tip` down to `from`.
    pub walk: UncheckedAccount<'info>,
    #[account(mut)]
    pub funder: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ResolveChallenge<'info> {
    #[account(mut, close = guardian, seeds = [CHALLENGE_SEED, &challenge.id.to_le_bytes()], bump = challenge.bump)]
    pub challenge: Account<'info, Challenge>,
    #[account(mut, seeds = [JOB_SEED, &challenge.job_id.to_le_bytes()], bump = job.bump)]
    pub job: Account<'info, Job>,
    #[account(mut, seeds = [OPERATOR_SEED, job.operator.as_ref()], bump = operator.bump)]
    pub operator: Account<'info, Operator>,
    /// The attester's record, when the job has one.
    #[account(mut)]
    pub attester: Option<Account<'info, Operator>>,
    /// CHECK: credit accounts, checked and created in `credit_to`.
    #[account(mut)]
    pub operator_credit: UncheckedAccount<'info>,
    /// CHECK: see above.
    #[account(mut)]
    pub application_credit: UncheckedAccount<'info>,
    /// CHECK: see above.
    #[account(mut)]
    pub guardian_credit: UncheckedAccount<'info>,
    /// CHECK: see above.
    #[account(mut)]
    pub payer_credit: UncheckedAccount<'info>,
    /// CHECK: receives the challenge account's rent.
    #[account(mut, address = challenge.guardian)]
    pub guardian: UncheckedAccount<'info>,
    #[account(mut)]
    pub funder: Signer<'info>,
    pub system_program: Program<'info, System>,
}
