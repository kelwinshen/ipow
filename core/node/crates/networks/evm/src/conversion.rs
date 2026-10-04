//! The Conversion application on an EVM network
//! (`contracts/apps/Conversion.sol`), for the node's operator.

use std::sync::Arc;

use alloy::eips::BlockId;
use alloy::primitives::{Address, Bytes, U256};
use alloy::providers::Provider;
use async_trait::async_trait;
use ipow_protocol_core::conversion::{ConversionApp, Side, Swap, SwapState};
use ipow_protocol_core::types::Amount;

use crate::contracts::{Conversion, IERC20};
use crate::network::{EvmNetwork, rejection, send, with_margin};

/// The confirmations of the jobs this node opens (the protocol's default).
const CONFIRMATIONS: u16 = 6;

pub struct EvmConversion {
    net: Arc<EvmNetwork>,
    address: Address,
}

impl EvmConversion {
    pub fn new(net: Arc<EvmNetwork>, address: &str) -> anyhow::Result<Self> {
        Ok(EvmConversion { net, address: address.parse().map_err(|_| anyhow::anyhow!("the Conversion address is not valid"))? })
    }

    fn contract(&self) -> Conversion::ConversionInstance<&alloy::providers::DynProvider> {
        Conversion::new(self.address, self.net.provider())
    }
}

impl EvmConversion {
    /// What a job this node opens pays in fees (D138): the commitment fee
    /// at the network's price of work now, twice (the margin goes to the
    /// job's operator, D79, which is this node when it takes the job),
    /// plus the escrow fee. Read with `commitmentFeeAt` and a price this
    /// node supplies: a call to `feesFor` runs at a price of zero on many
    /// nodes, and the protocol then refuses the transaction (FeesNotPaid).
    /// The price is the larger of the base fee and the gas price, so it
    /// covers the networks that price work either way (D139).
    async fn fees_now(&self) -> anyhow::Result<U256> {
        let provider = self.net.provider();
        let block = provider.get_block_by_number(alloy::eips::BlockNumberOrTag::Latest).await?;
        let base_fee = block.and_then(|b| b.header.base_fee_per_gas).unwrap_or(0) as u128;
        let gas_price = provider.get_gas_price().await.unwrap_or(0);
        let price = U256::from(base_fee.max(gas_price));
        let protocol = self.net.protocol();
        let c = self.contract();
        let fee = protocol.commitmentFeeAt(CONFIRMATIONS, price).call().await?;
        let (multiple, bps, escrow_bps) = (protocol.MIN_ESCROW_MULTIPLE().call().await?, protocol.BPS().call().await?, c.ESCROW_FEE_BPS().call().await?);
        let escrow = (multiple * fee).max(U256::from(1));
        let escrow_fee = escrow * U256::from(escrow_bps) / U256::from(bps);
        Ok(fee * U256::from(2) + escrow_fee)
    }
}

fn to_amount(v: U256) -> anyhow::Result<Amount> {
    u128::try_from(v).map_err(|_| anyhow::anyhow!("the amount {v} does not fit 128 bits"))
}

#[async_trait]
impl ConversionApp for EvmConversion {
    fn application(&self) -> String {
        self.address.to_string()
    }

    async fn swaps_after(&self, after: u64, limit: usize) -> anyhow::Result<Vec<Swap>> {
        let count = self.contract().swapCount().call().await?.to::<u64>();
        let mut out = vec![];
        for id in (after + 1)..=count {
            if out.len() >= limit {
                break;
            }
            out.push(self.swap(id).await?);
        }
        Ok(out)
    }

    async fn swap(&self, id: u64) -> anyhow::Result<Swap> {
        let s = self.contract().getSwap(U256::from(id)).call().await?;
        let side = match s.side {
            1 => Side::Sell,
            2 => Side::Buy,
            other => anyhow::bail!("swap {id} does not exist (side {other})"),
        };
        let state = match s.state {
            1 => SwapState::Open,
            2 => SwapState::Funded,
            3 => SwapState::Done,
            4 => SwapState::Refunded,
            5 => SwapState::Cancelled,
            6 => SwapState::Reclaimed,
            other => anyhow::bail!("swap {id} has an unknown state {other}"),
        };
        Ok(Swap {
            id,
            side,
            state,
            user: s.user.to_string(),
            token: (s.token != Address::ZERO).then(|| s.token.to_string()),
            amount: to_amount(s.amount)?,
            sats: s.sats,
            job_id: s.jobId.to::<u64>(),
            script: s.script.to_vec(),
            pay_window: (s.payFrom != 0).then_some((s.payFrom, s.payTo)),
        })
    }

    async fn my_balance(&self, token: Option<&str>) -> anyhow::Result<Amount> {
        let me = self.net.address();
        match token {
            None => to_amount(self.net.provider().get_balance(me).await?),
            Some(t) => to_amount(IERC20::new(t.parse()?, self.net.provider()).balanceOf(me).call().await?),
        }
    }

    async fn buy_fees(&self) -> anyhow::Result<Amount> {
        to_amount(self.fees_now().await?)
    }

    /// An EIP-191 personal signature (`personal_sign`), hex, 65 bytes.
    fn signed_by(&self, user: &str, message: &str, signature: &str) -> bool {
        let Ok(user) = user.parse::<Address>() else { return false };
        let Ok(sig) = signature.parse::<alloy::primitives::Signature>() else { return false };
        sig.recover_address_from_msg(message.as_bytes()).is_ok_and(|a| a == user)
    }

    async fn fund(&self, swap_id: u64, script: &[u8]) -> anyhow::Result<()> {
        let s = self.swap(swap_id).await?;
        let c = self.contract();
        match &s.token {
            None => {
                send!(self.net, c.fund(U256::from(swap_id), Bytes::copy_from_slice(script)).value(U256::from(s.amount)));
            }
            Some(t) => {
                let token = IERC20::new(t.parse()?, self.net.provider());
                // Some tokens refuse to change a non-zero approval to another
                // non-zero one: a left-over approval is set to zero first.
                let left = token.allowance(self.net.address(), self.address).call().await?;
                if left != U256::from(s.amount) {
                    if !left.is_zero() {
                        send!(self.net, token.approve(self.address, U256::ZERO));
                    }
                    send!(self.net, token.approve(self.address, U256::from(s.amount)));
                }
                send!(self.net, c.fund(U256::from(swap_id), Bytes::copy_from_slice(script)));
            }
        }
        Ok(())
    }

    async fn complete_sell(&self, swap_id: u64, raw_tx: &[u8]) -> anyhow::Result<()> {
        send!(self.net, self.contract().completeSell(U256::from(swap_id), Bytes::copy_from_slice(raw_tx)));
        Ok(())
    }

    async fn complete_buy(&self, swap_id: u64, receipt_raw: &[u8], payment_raw: &[u8], vout: u32) -> anyhow::Result<()> {
        send!(
            self.net,
            self.contract().completeBuy(U256::from(swap_id), Bytes::copy_from_slice(receipt_raw), Bytes::copy_from_slice(payment_raw), vout)
        );
        Ok(())
    }

    async fn reclaim(&self, swap_id: u64) -> anyhow::Result<()> {
        send!(self.net, self.contract().reclaim(U256::from(swap_id)));
        Ok(())
    }
}
