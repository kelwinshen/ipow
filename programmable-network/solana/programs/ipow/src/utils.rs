use anchor_lang::prelude::*;

pub fn transfer_from_escrow<'info>(
    amount: u64,
    to_acc: &AccountInfo<'info>,
    escrow_acc: &AccountInfo<'info>,
    sys_prog: &AccountInfo<'info>,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    if amount > 0 {
        anchor_lang::solana_program::program::invoke_signed(
            &anchor_lang::solana_program::system_instruction::transfer(
                &escrow_acc.key(),
                &to_acc.key(),
                amount,
            ),
            &[escrow_acc.clone(), to_acc.clone(), sys_prog.clone()],
            signer_seeds,
        )?;
    }
    Ok(())
}
