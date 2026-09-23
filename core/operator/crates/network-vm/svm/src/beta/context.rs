use crate::beta::config::BetaSvmConfig;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Keypair;
use std::sync::Arc;

#[derive(Clone)]
pub struct BetaSvmContext {
    pub rpc_client: Arc<RpcClient>,
    pub operator: Arc<Keypair>,
    pub hub_program_id: Pubkey,
    pub ipow_program_id: Pubkey,
    pub party_id: [u8; 32],

    // --- beta_factory PDAs, fixed for the life of the deployment ---
    pub config_pda: Pubkey,
    pub bond_escrow_pda: Pubkey,
    pub insurance_pda: Pubkey,
    pub reward_pool_pda: Pubkey,
    pub fees_pda: Pubkey,
}

impl BetaSvmContext {
    pub fn init(cfg: BetaSvmConfig) -> anyhow::Result<Self> {
        let rpc_client = Arc::new(RpcClient::new(cfg.rpc_url.clone()));
        let operator =
            Arc::new(Keypair::from_base58_string(&cfg.operator_private_key));
        let hub_program_id: Pubkey = cfg.hub_program_id.parse()?;
        let ipow_program_id: Pubkey = cfg.ipow_program_id.parse()?;

        let (config_pda, _) =
            Pubkey::find_program_address(&[b"config"], &hub_program_id);
        let (bond_escrow_pda, _) =
            Pubkey::find_program_address(&[b"bond_escrow"], &hub_program_id);
        let (insurance_pda, _) =
            Pubkey::find_program_address(&[b"insurance"], &hub_program_id);
        let (reward_pool_pda, _) =
            Pubkey::find_program_address(&[b"rewards"], &hub_program_id);
        let (fees_pda, _) =
            Pubkey::find_program_address(&[b"fees"], &hub_program_id);

        Ok(Self {
            rpc_client,
            operator,
            hub_program_id,
            ipow_program_id,
            party_id: cfg.party_id,
            config_pda,
            bond_escrow_pda,
            insurance_pda,
            reward_pool_pda,
            fees_pda,
        })
    }

    pub fn party_pda(&self) -> Pubkey {
        Pubkey::find_program_address(
            &[b"party", &self.party_id],
            &self.hub_program_id,
        )
        .0
    }

    pub fn pending_pda(&self, user: &Pubkey, nonce: u64) -> Pubkey {
        Pubkey::find_program_address(
            &[b"pending", user.as_ref(), &nonce.to_le_bytes()],
            &self.hub_program_id,
        )
        .0
    }

    pub fn composition_pda(&self, id: u64) -> Pubkey {
        Pubkey::find_program_address(
            &[b"composition", &id.to_le_bytes()],
            &self.hub_program_id,
        )
        .0
    }

    pub fn processed_anchor_pda(&self, txid_le: &[u8; 32]) -> Pubkey {
        Pubkey::find_program_address(&[b"anchor", txid_le], &self.hub_program_id).0
    }

    pub fn header_pda(&self, height: u64) -> Pubkey {
        Pubkey::find_program_address(
            &[b"header", &height.to_le_bytes()],
            &self.ipow_program_id,
        )
        .0
    }
}
