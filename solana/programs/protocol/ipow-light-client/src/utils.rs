use anchor_lang::prelude::*;
use anchor_lang::system_program::{allocate, assign, create_account, transfer, Allocate, Assign, CreateAccount, Transfer};

use crate::bitcoin::{target_from_bits, Header, U256};
use crate::constants::{MAX_FUTURE_TIME, NODE_SEED};
use crate::errors::LightClientError;
use crate::state::{Config, Node};

pub fn target(bits: u32) -> Result<U256> {
    target_from_bits(bits).map_err(|_| error!(LightClientError::InvalidBits))
}

/// The checks every block passes, however it arrives: enough work for its
/// own difficulty, the minimum difficulty (D38), and a time that is not in
/// the future.
pub fn check_header(header: &Header, config: &Config, now: i64) -> Result<[u8; 32]> {
    let hash = header.hash();
    let t = target(header.bits())?;
    require!(!t.gt(&U256::from_be_bytes(&config.max_target)), LightClientError::DifficultyTooLow);
    require!(!U256::from_le_bytes(&hash).gt(&t), LightClientError::InsufficientWork);
    require!((header.time() as i64) <= now + MAX_FUTURE_TIME, LightClientError::TimeTooNew);
    Ok(hash)
}

pub fn node_address(program_id: &Pubkey, hash: &[u8; 32], height: u32, epoch_time: u32) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[NODE_SEED, hash, &height.to_le_bytes(), &epoch_time.to_le_bytes()],
        program_id,
    )
}

/// A stored block, or `None` when the account holds none.
pub fn load_node(program_id: &Pubkey, account: &AccountInfo) -> Result<Option<Node>> {
    if account.owner != program_id || account.data_is_empty() {
        return Ok(None);
    }
    let data = account.try_borrow_data()?;
    let node = Node::try_deserialize(&mut &data[..])?;
    Ok(Some(node))
}

/// Writes a changed block back to its account.
pub fn save_node(account: &AccountInfo, node: &Node) -> Result<()> {
    let mut data = account.try_borrow_mut_data()?;
    node.try_serialize(&mut &mut data[..])?;
    Ok(())
}

/// Stores a block unless it is stored already (D48). The account must be
/// the one its hash, number and epoch time give. Returns the stored block.
#[allow(clippy::too_many_arguments)]
pub fn store_node<'info>(
    program_id: &Pubkey,
    account: &AccountInfo<'info>,
    payer: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
    header: &Header,
    hash: [u8; 32],
    height: u32,
    epoch_time: u32,
    now: i64,
) -> Result<Node> {
    let (address, bump) = node_address(program_id, &hash, height, epoch_time);
    require_keys_eq!(account.key(), address, LightClientError::WrongAccount);
    if let Some(existing) = load_node(program_id, account)? {
        return Ok(existing);
    }

    let space = 8 + Node::INIT_SPACE;
    let height_bytes = height.to_le_bytes();
    let epoch_bytes = epoch_time.to_le_bytes();
    let seeds: &[&[u8]] = &[NODE_SEED, &hash, &height_bytes, &epoch_bytes, &[bump]];
    create_pda(program_id, account, payer, system_program, space, seeds)?;

    let node = Node {
        hash,
        prev_hash: header.prev(),
        merkle_root: header.merkle_root(),
        bits: header.bits(),
        time: header.time(),
        height,
        epoch_time,
        stored_at: now,
        anchored_at: 0,
        payer: payer.key(),
        bump,
    };
    save_node(account, &node)?;
    emit!(crate::BlockStored { hash, height, epoch_time, prev_hash: node.prev_hash });
    Ok(node)
}

/// Whether a transaction is in a block (Merkle proof). The same checks as
/// `txInBlock` in `iPoWLightClient.sol`.
pub fn tx_in_block(node: &Node, raw_tx: &[u8], siblings: &[[u8; 32]], index: u64) -> Result<bool> {
    // A 64-byte "transaction" could be an inner node passed off as a leaf.
    require!(raw_tx.len() != 64, LightClientError::InvalidTransaction);
    require!(siblings.len() <= 32, LightClientError::InvalidIndex);
    if siblings.len() < 64 {
        require!(index >> siblings.len() == 0, LightClientError::InvalidIndex);
    }
    let txid = crate::bitcoin::sha256d(raw_tx);
    Ok(crate::bitcoin::merkle_root_from(txid, siblings, index) == node.merkle_root)
}

/// Creates a program-owned account at a program address. Works even when
/// someone has sent lamports to the address first: a plain `create_account`
/// would then fail, and anyone could block a Bitcoin block from ever being
/// stored for a fraction of a cent. The same as Anchor's own `init`.
pub fn create_pda<'info>(
    program_id: &Pubkey,
    account: &AccountInfo<'info>,
    payer: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
    space: usize,
    seeds: &[&[u8]],
) -> Result<()> {
    let rent = Rent::get()?.minimum_balance(space);
    let current = account.lamports();
    let system = system_program.key();
    if current == 0 {
        return create_account(
            CpiContext::new_with_signer(system, CreateAccount { from: payer.clone(), to: account.clone() }, &[seeds]),
            rent,
            space as u64,
            program_id,
        );
    }
    if current < rent {
        transfer(
            CpiContext::new(system, Transfer { from: payer.clone(), to: account.clone() }),
            rent - current,
        )?;
    }
    allocate(
        CpiContext::new_with_signer(system, Allocate { account_to_allocate: account.clone() }, &[seeds]),
        space as u64,
    )?;
    assign(
        CpiContext::new_with_signer(system, Assign { account_to_assign: account.clone() }, &[seeds]),
        program_id,
    )?;
    Ok(())
}
