//! The Conversion application on Solana (`programs/conversion`), for the
//! node's operator.

use std::sync::Arc;

use anchor_lang::prelude::Pubkey;
use anchor_lang::solana_program::instruction::Instruction;
use anchor_lang::{InstructionData, ToAccountMetas};
use anchor_spl::associated_token::get_associated_token_address_with_program_id as ata;
use async_trait::async_trait;
use ipow_protocol_core::conversion::{ConversionApp, Side, Swap, SwapState};
use ipow_protocol_core::types::{Amount, BlockRef};
use sha2::{Digest, Sha256};

use crate::network::SvmNetwork;
use crate::programs::conversion as cv;

const SYSTEM: Pubkey = anchor_lang::solana_program::system_program::ID;
/// The confirmations of the jobs this node opens (the protocol's default).
const CONFIRMATIONS: u16 = 6;

/// What a job opened by this node pays: the commitment fee (fixed by the
/// confirmations on Solana: the protocol's `commitment_fee_for`), the escrow
/// fee on the lowest escrow, and half the commitment fee again, which goes
/// to the job's operator (D79).
fn paid(confirmations: u16) -> u64 {
    let window = 25 + confirmations as u64 - 1;
    let fee = (window + 20) * 5_000 * 3 / 2;
    let escrow_fee = fee * 5 * 50 / 10_000;
    fee + escrow_fee + fee / 2
}

pub struct SvmConversion {
    net: Arc<SvmNetwork>,
}

fn cv_pda(seeds: &[&[u8]]) -> Pubkey {
    Pubkey::find_program_address(seeds, &cv::ID).0
}

fn swap_pda(id: u64) -> Pubkey {
    cv_pda(&[b"swap", &id.to_le_bytes()])
}

/// The token accounts of a token swap: the mint, its program, the swap's
/// escrow, and `owner`'s account.
struct TokenAccounts {
    mint: Pubkey,
    program: Pubkey,
    escrow: Pubkey,
    other: Pubkey,
}

impl SvmConversion {
    pub fn new(net: Arc<SvmNetwork>, program: &str) -> anyhow::Result<Self> {
        anyhow::ensure!(program == cv::ID.to_string(), "Conversion is {program}, this node is built for {}", cv::ID);
        Ok(SvmConversion { net })
    }

    fn ix(accounts: impl ToAccountMetas, data: impl InstructionData) -> Instruction {
        Instruction { program_id: cv::ID, accounts: accounts.to_account_metas(None), data: data.data() }
    }

    async fn record(&self, id: u64) -> anyhow::Result<cv::accounts::Swap> {
        self.net.read_account(&swap_pda(id)).await?.ok_or_else(|| anyhow::anyhow!("swap {id} does not exist"))
    }

    /// The token accounts for a swap's mint, with `owner`'s account created
    /// first when it does not exist (paid by this node).
    async fn tokens(&self, swap: &cv::accounts::Swap, owner: &Pubkey) -> anyhow::Result<Option<TokenAccounts>> {
        if swap.mint == Pubkey::default() {
            return Ok(None);
        }
        let mint_owner = self.mint_program(&swap.mint).await?;
        let other = ata(owner, &swap.mint, &mint_owner);
        if self.net.account_data(&other).await?.is_none() {
            let create = anchor_spl::associated_token::spl_associated_token_account::instruction::create_associated_token_account_idempotent(
                &self.net.me_key(),
                owner,
                &swap.mint,
                &mint_owner,
            );
            self.net.send_ix(create).await?;
        }
        Ok(Some(TokenAccounts { mint: swap.mint, program: mint_owner, escrow: ata(&swap_pda(swap.id), &swap.mint, &mint_owner), other }))
    }

    /// The token program of a mint: the program that owns it.
    async fn mint_program(&self, mint: &Pubkey) -> anyhow::Result<Pubkey> {
        let (owner, _) = self.net.owner_and_lamports(mint).await?.ok_or_else(|| anyhow::anyhow!("mint {mint} does not exist"))?;
        anyhow::ensure!(owner == anchor_spl::token::ID || owner == anchor_spl::token_2022::ID, "{mint} is not a token mint");
        Ok(owner)
    }
}

#[async_trait]
impl ConversionApp for SvmConversion {
    fn application(&self) -> String {
        cv_pda(&[b"config"]).to_string()
    }

    async fn swaps_after(&self, after: u64, limit: usize) -> anyhow::Result<Vec<Swap>> {
        let config: cv::accounts::Config =
            self.net.read_account(&cv_pda(&[b"config"])).await?.ok_or_else(|| anyhow::anyhow!("Conversion is not initialized"))?;
        let mut out = vec![];
        for id in (after + 1)..=config.swap_count {
            if out.len() >= limit {
                break;
            }
            out.push(self.swap(id).await?);
        }
        Ok(out)
    }

    async fn swap(&self, id: u64) -> anyhow::Result<Swap> {
        let s = self.record(id).await?;
        Ok(Swap {
            id,
            side: match s.side {
                cv::types::Side::Sell => Side::Sell,
                cv::types::Side::Buy => Side::Buy,
            },
            state: match s.state {
                cv::types::SwapState::Open => SwapState::Open,
                cv::types::SwapState::Funded => SwapState::Funded,
                cv::types::SwapState::Done => SwapState::Done,
                cv::types::SwapState::Refunded => SwapState::Refunded,
                cv::types::SwapState::Cancelled => SwapState::Cancelled,
                cv::types::SwapState::Reclaimed => SwapState::Reclaimed,
            },
            user: s.user.to_string(),
            token: (s.mint != Pubkey::default()).then(|| s.mint.to_string()),
            amount: s.amount as Amount,
            sats: s.sats,
            job_id: s.job_id,
            script: s.script,
            pay_window: (s.pay_from != 0).then_some((s.pay_from, s.pay_to)),
            // The program keeps no memo yet; its buys are not open to users.
            memo: vec![],
        })
    }

    async fn my_balance(&self, token: Option<&str>) -> anyhow::Result<Amount> {
        let me = self.net.me_key();
        match token {
            None => Ok(self.net.owner_and_lamports(&me).await?.map_or(0, |(_, l)| l) as Amount),
            Some(t) => {
                let mint: Pubkey = t.parse()?;
                let program = self.mint_program(&mint).await?;
                match self.net.account_data(&ata(&me, &mint, &program)).await? {
                    // The amount sits at bytes 64..72 of a token account.
                    Some(d) if d.len() >= 72 => Ok(u64::from_le_bytes(d[64..72].try_into().unwrap()) as Amount),
                    _ => Ok(0),
                }
            }
        }
    }

    async fn buy_fees(&self) -> anyhow::Result<Amount> {
        Ok(paid(CONFIRMATIONS) as Amount)
    }

    async fn fund(&self, swap_id: u64, script: &[u8]) -> anyhow::Result<()> {
        let s = self.record(swap_id).await?;
        let job = self.net.job_record(s.job_id).await?;
        let anchor = BlockRef { hash: job.anchor.hash, height: job.anchor.height, epoch_time: job.anchor.epoch_time };
        let me = self.net.me_key();
        let t = self.tokens(&s, &me).await?;
        let key: [u8; 32] = Sha256::digest(script).into();
        self.net
            .send_ix(Self::ix(
                cv::client::accounts::Fund {
                    swap: swap_pda(swap_id),
                    job: SvmNetwork::job_pda(s.job_id),
                    script_record: cv_pda(&[b"script", &key]),
                    anchor_node: SvmNetwork::node_pda(&anchor),
                    operator: me,
                    system_program: SYSTEM,
                    mint: t.as_ref().map(|t| t.mint),
                    escrow: t.as_ref().map(|t| t.escrow),
                    from: t.as_ref().map(|t| t.other),
                    token_program: t.as_ref().map(|t| t.program),
                    associated_token_program: t.as_ref().map(|_| anchor_spl::associated_token::ID),
                },
                cv::client::args::Fund { script: script.to_vec() },
            ))
            .await
    }

    async fn complete_sell(&self, swap_id: u64, raw_tx: &[u8]) -> anyhow::Result<()> {
        let s = self.record(swap_id).await?;
        let job = self.net.job_record(s.job_id).await?;
        let t = self.tokens(&s, &job.operator).await?;
        self.net
            .send_ix(Self::ix(
                cv::client::accounts::CompleteSell {
                    swap: swap_pda(swap_id),
                    job: SvmNetwork::job_pda(s.job_id),
                    operator: job.operator,
                    mint: t.as_ref().map(|t| t.mint),
                    escrow: t.as_ref().map(|t| t.escrow),
                    to: t.as_ref().map(|t| t.other),
                    token_program: t.as_ref().map(|t| t.program),
                },
                cv::client::args::CompleteSell { raw_tx: raw_tx.to_vec() },
            ))
            .await
    }

    async fn complete_buy(&self, swap_id: u64, receipt_raw: &[u8], payment_raw: &[u8], vout: u32) -> anyhow::Result<()> {
        let s = self.record(swap_id).await?;
        let t = self.tokens(&s, &s.user).await?;
        self.net
            .send_ix(Self::ix(
                cv::client::accounts::CompleteBuy {
                    swap: swap_pda(swap_id),
                    job: SvmNetwork::job_pda(s.job_id),
                    user: s.user,
                    mint: t.as_ref().map(|t| t.mint),
                    escrow: t.as_ref().map(|t| t.escrow),
                    to: t.as_ref().map(|t| t.other),
                    token_program: t.as_ref().map(|t| t.program),
                },
                cv::client::args::CompleteBuy { receipt_raw: receipt_raw.to_vec(), payment_raw: payment_raw.to_vec(), vout },
            ))
            .await
    }

    async fn reclaim(&self, swap_id: u64) -> anyhow::Result<()> {
        let s = self.record(swap_id).await?;
        let job = self.net.job_record(s.job_id).await?;
        let t = self.tokens(&s, &job.operator).await?;
        self.net
            .send_ix(Self::ix(
                cv::client::accounts::Reclaim {
                    swap: swap_pda(swap_id),
                    job: SvmNetwork::job_pda(s.job_id),
                    operator: job.operator,
                    mint: t.as_ref().map(|t| t.mint),
                    escrow: t.as_ref().map(|t| t.escrow),
                    to: t.as_ref().map(|t| t.other),
                    token_program: t.as_ref().map(|t| t.program),
                },
                cv::client::args::Reclaim {},
            ))
            .await
    }
}
