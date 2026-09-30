//! Tests of BETA baskets (docs/drafts/ipow-beta-app.md): parts standing for
//! wrapped SOL and vETH, and parts whose issuer controls them (E3): one it
//! can freeze, one it can move out of any account (Token-2022's permanent
//! delegate). A creator fee on mint and on burn, rounding in the basket's
//! favour, and the checks on baskets and accounts.

use anchor_lang;
use anchor_lang::solana_program::instruction::{AccountMeta, Instruction};
use anchor_lang::AccountDeserialize;
use anchor_litesvm::{AnchorContext, AnchorLiteSVM, TestHelpers};
use anchor_spl::token_2022::spl_token_2022 as t22;
use solana_keypair::Keypair;
use solana_program::pubkey::Pubkey;
use solana_signer::Signer;

anchor_lang::declare_program!(beta_basket);

const SOL: u64 = 1_000_000_000;
const ONE: u64 = 1_000_000_000;
const SYSTEM: Pubkey = anchor_lang::solana_program::system_program::ID;
const TOKEN: Pubkey = anchor_spl::token::ID;
const TOKEN22: Pubkey = anchor_spl::token_2022::ID;
const ATA: Pubkey = anchor_spl::associated_token::ID;
const CAN_FREEZE: u8 = 1;
const CAN_MOVE: u8 = 4;

fn bk(seeds: &[&[u8]]) -> Pubkey {
    Pubkey::find_program_address(seeds, &beta_basket::ID).0
}
fn basket_pda(creator: &Pubkey, id: u64) -> Pubkey {
    bk(&[b"basket", creator.as_ref(), &id.to_le_bytes()])
}
fn beta_pda(basket: &Pubkey) -> Pubkey {
    bk(&[b"beta", basket.as_ref()])
}
fn owed_pda(basket: &Pubkey, user: &Pubkey, i: u8) -> Pubkey {
    bk(&[b"owed", basket.as_ref(), user.as_ref(), &[i]])
}
fn ata(owner: &Pubkey, mint: &Pubkey, program: &Pubkey) -> Pubkey {
    anchor_spl::associated_token::get_associated_token_address_with_program_id(owner, mint, program)
}

fn expect_err<T: std::fmt::Debug>(r: Result<T, String>, name: &str) {
    match r {
        Ok(v) => panic!("expected {name}, got {v:?}"),
        Err(logs) => assert!(logs.contains(&format!("Error Code: {name}")), "expected {name}, got:\n{logs}"),
    }
}

struct World {
    ctx: AnchorContext,
    creator: Keypair,
    user: Keypair,
    /// Wrapped SOL and vETH, as two plain tokens of 9 decimals.
    wsol: Pubkey,
    veth: Pubkey,
    /// Each mint's token program.
    programs: std::collections::HashMap<Pubkey, Pubkey>,
}

impl World {
    fn new() -> Self {
        let mut ctx = AnchorLiteSVM::build_with_programs(&[(beta_basket::ID, include_bytes!("../../../target/deploy/beta_basket.so"))]);
        let creator = ctx.svm.create_funded_account(100 * SOL).unwrap();
        let user = ctx.svm.create_funded_account(100 * SOL).unwrap();
        let wsol = litesvm_token::CreateMint::new(&mut ctx.svm, &creator).decimals(9).send().unwrap();
        let veth = litesvm_token::CreateMint::new(&mut ctx.svm, &creator).decimals(9).send().unwrap();
        let mut w = World { ctx, creator, user, wsol, veth, programs: Default::default() };
        w.programs.insert(wsol, TOKEN);
        w.programs.insert(veth, TOKEN);
        for mint in [wsol, veth] {
            w.fund(&mint, 10 * ONE);
        }
        w
    }

    /// Accounts of `mint` for the user (holding `amount`) and the creator.
    fn fund(&mut self, mint: &Pubkey, amount: u64) {
        let program = self.programs[mint];
        let creator = self.creator.insecure_clone();
        for owner in [self.user.pubkey(), creator.pubkey()] {
            let ix = anchor_spl::associated_token::spl_associated_token_account::instruction::create_associated_token_account_idempotent(
                &creator.pubkey(),
                &owner,
                mint,
                &program,
            );
            self.send(ix, &[&creator]).unwrap();
        }
        let ix = t22::instruction::mint_to(&program, mint, &ata(&self.user.pubkey(), mint, &program), &creator.pubkey(), &[], amount).unwrap();
        self.send(ix, &[&creator]).unwrap();
    }

    /// A Token-2022 mint of 9 decimals, with a permanent delegate (the
    /// creator) when `delegate`, and a transfer fee when `fee`.
    fn mint22(&mut self, delegate: bool, fee: bool) -> Pubkey {
        use t22::extension::ExtensionType;
        let creator = self.creator.insecure_clone();
        let mint = Keypair::new();
        let mut exts = vec![];
        if delegate {
            exts.push(ExtensionType::PermanentDelegate);
        }
        if fee {
            exts.push(ExtensionType::TransferFeeConfig);
        }
        let space = ExtensionType::try_calculate_account_len::<t22::state::Mint>(&exts).unwrap();
        let rent = self.ctx.svm.minimum_balance_for_rent_exemption(space);
        let mut ixs = vec![anchor_lang::solana_program::system_instruction::create_account(&creator.pubkey(), &mint.pubkey(), rent, space as u64, &TOKEN22)];
        if delegate {
            ixs.push(t22::instruction::initialize_permanent_delegate(&TOKEN22, &mint.pubkey(), &creator.pubkey()).unwrap());
        }
        if fee {
            ixs.push(t22::extension::transfer_fee::instruction::initialize_transfer_fee_config(&TOKEN22, &mint.pubkey(), None, None, 10, 1_000).unwrap());
        }
        ixs.push(t22::instruction::initialize_mint2(&TOKEN22, &mint.pubkey(), &creator.pubkey(), None, 9).unwrap());
        self.ctx.svm.expire_blockhash();
        let r = self.ctx.execute_instructions(ixs, &[&creator, &mint]).unwrap();
        assert!(r.is_success(), "{}", r.logs().join("\n"));
        self.programs.insert(mint.pubkey(), TOKEN22);
        mint.pubkey()
    }

    fn send(&mut self, ix: Instruction, signers: &[&Keypair]) -> Result<(), String> {
        self.ctx.svm.expire_blockhash();
        // A basket of many parts needs more than Solana's default 200,000
        // compute units: a client asks for more, as for any large call.
        let mut data = vec![2u8];
        data.extend_from_slice(&1_400_000u32.to_le_bytes());
        let budget = Instruction { program_id: "ComputeBudget111111111111111111111111111111".parse().unwrap(), accounts: vec![], data };
        let r = self.ctx.execute_instructions(vec![budget, ix], signers).unwrap();
        if r.is_success() { Ok(()) } else { Err(r.logs().join("\n")) }
    }

    fn ix<A: anchor_lang::ToAccountMetas, D: anchor_lang::InstructionData>(&self, a: A, d: D) -> Instruction {
        Instruction { program_id: beta_basket::ID, accounts: a.to_account_metas(None), data: d.data() }
    }

    fn tokens(&self, account: &Pubkey) -> u64 {
        match self.ctx.svm.get_account(account) {
            Some(a) if !a.data.is_empty() => t22::extension::StateWithExtensions::<t22::state::Account>::unpack(&a.data).unwrap().base.amount,
            _ => 0,
        }
    }

    fn held(&self, basket: &Pubkey, mint: &Pubkey) -> u64 {
        self.tokens(&ata(basket, mint, &self.programs[mint]))
    }

    fn user_has(&self, mint: &Pubkey) -> u64 {
        self.tokens(&ata(&self.user.pubkey(), mint, &self.programs[mint]))
    }

    fn create(&mut self, id: u64, parts: Vec<(Pubkey, u64)>, mint_fee_bps: u16, burn_fee_bps: u16) -> Result<Pubkey, String> {
        let creator = self.creator.insecure_clone();
        let basket = basket_pda(&creator.pubkey(), id);
        let mints: Vec<Pubkey> = parts.iter().map(|(m, _)| *m).collect();
        let mut ix = self.ix(
            beta_basket::client::accounts::CreateBasket {
                basket,
                beta: beta_pda(&basket),
                creator: creator.pubkey(),
                token_program: TOKEN,
                system_program: SYSTEM,
            },
            beta_basket::client::args::CreateBasket {
                id,
                parts: parts
                    .into_iter()
                    .map(|(mint, amount)| beta_basket::types::Part { mint, amount, token_program: Pubkey::default(), decimals: 0, powers: 0, owed: 0 })
                    .collect(),
                mint_fee_bps,
                burn_fee_bps,
            },
        );
        for m in mints {
            ix.accounts.push(AccountMeta::new_readonly(m, false));
        }
        self.send(ix, &[&creator])?;
        Ok(basket)
    }

    fn basket(&self, key: &Pubkey) -> beta_basket::accounts::Basket {
        self.ctx.get_account(key).unwrap()
    }

    /// The user's BETA account, created if needed.
    fn user_beta(&mut self, basket: &Pubkey) -> Pubkey {
        let beta = beta_pda(basket);
        let a = ata(&self.user.pubkey(), &beta, &TOKEN);
        if self.ctx.svm.get_account(&a).is_none() {
            let payer = self.user.insecure_clone();
            litesvm_token::CreateAssociatedTokenAccount::new(&mut self.ctx.svm, &payer, &beta).owner(&payer.pubkey()).send().unwrap();
        }
        a
    }

    /// The five accounts of each part, as the user sees them.
    fn part_metas(&self, basket: &Pubkey) -> Vec<AccountMeta> {
        let b = self.basket(basket);
        let user = self.user.pubkey();
        let mut out = vec![];
        for p in &b.parts {
            out.push(AccountMeta::new_readonly(p.mint, false));
            out.push(AccountMeta::new(ata(basket, &p.mint, &p.token_program), false));
            out.push(AccountMeta::new(ata(&user, &p.mint, &p.token_program), false));
            out.push(AccountMeta::new(ata(&b.fee_to, &p.mint, &p.token_program), false));
            out.push(AccountMeta::new_readonly(p.token_program, false));
        }
        out
    }

    fn mint(&mut self, basket: &Pubkey, amount: u64) -> Result<(), String> {
        let user = self.user.insecure_clone();
        let user_beta = self.user_beta(basket);
        let mut ix = self.ix(
            beta_basket::client::accounts::Mint {
                basket: *basket,
                beta: beta_pda(basket),
                user_beta,
                user: user.pubkey(),
                token_program: TOKEN,
                associated_token_program: ATA,
                system_program: SYSTEM,
            },
            beta_basket::client::args::Mint { amount },
        );
        ix.accounts.extend(self.part_metas(basket));
        self.send(ix, &[&user])
    }

    /// Burns `amount` BETA, deferring the parts whose bits are in `defer`.
    fn burn(&mut self, basket: &Pubkey, amount: u64, defer: u8) -> Result<(), String> {
        let user = self.user.insecure_clone();
        let user_beta = self.user_beta(basket);
        let mut ix = self.ix(
            beta_basket::client::accounts::Burn {
                basket: *basket,
                beta: beta_pda(basket),
                user_beta,
                user: user.pubkey(),
                token_program: TOKEN,
                system_program: SYSTEM,
            },
            beta_basket::client::args::Burn { amount, defer },
        );
        ix.accounts.extend(self.part_metas(basket));
        for i in 0..8u8 {
            if defer & (1 << i) != 0 {
                ix.accounts.push(AccountMeta::new(owed_pda(basket, &user.pubkey(), i), false));
            }
        }
        self.send(ix, &[&user])
    }

    fn collect_owed(&mut self, basket: &Pubkey, index: u8) -> Result<(), String> {
        let user = self.user.insecure_clone();
        let mut ix = self.ix(
            beta_basket::client::accounts::CollectOwed { basket: *basket, owed: owed_pda(basket, &user.pubkey(), index), user: user.pubkey() },
            beta_basket::client::args::CollectOwed { index },
        );
        let metas = self.part_metas(basket);
        ix.accounts.extend(metas[5 * index as usize..5 * index as usize + 5].iter().cloned());
        self.send(ix, &[&user])
    }
}

#[test]
fn mints_and_burns_a_basket_of_two_parts_with_the_creators_fee() {
    let mut w = World::new();
    // 1 BETA = 1 SOL + 0.5 vETH; 0.3% on mint, 0.2% on burn.
    let basket = w.create(1, vec![(w.wsol, ONE), (w.veth, ONE / 2)], 30, 20).unwrap();
    let creator = w.creator.pubkey();
    let (wsol, veth) = (w.wsol, w.veth);

    w.mint(&basket, 2 * ONE).unwrap();
    assert_eq!(w.tokens(&ata(&w.user.pubkey(), &beta_pda(&basket), &TOKEN)), 2 * ONE);
    // The basket holds 2 SOL and 1 vETH; the fee is 0.3% of each on top.
    assert_eq!(w.held(&basket, &wsol), 2 * ONE);
    assert_eq!(w.held(&basket, &veth), ONE);
    assert_eq!(w.tokens(&ata(&creator, &wsol, &TOKEN)), 2 * ONE * 3 / 1000);
    assert_eq!(w.tokens(&ata(&creator, &veth, &TOKEN)), ONE * 3 / 1000);
    assert_eq!(w.user_has(&wsol), 10 * ONE - 2 * ONE - 2 * ONE * 3 / 1000);

    // Burning 1 BETA gives half of what the basket holds, less 0.2%.
    w.burn(&basket, ONE, 0).unwrap();
    assert_eq!(w.held(&basket, &wsol), ONE);
    assert_eq!(w.held(&basket, &veth), ONE / 2);
    assert_eq!(w.tokens(&ata(&creator, &wsol, &TOKEN)), 2 * ONE * 3 / 1000 + ONE * 2 / 1000);
    // Plain tokens: no issuer powers recorded.
    assert!(w.basket(&basket).parts.iter().all(|p| p.powers == 0 && p.token_program == TOKEN));
}

#[test]
fn rounds_in_the_baskets_favour() {
    let mut w = World::new();
    // 1 BETA holds 3 of the smallest vETH unit.
    let veth = w.veth;
    let basket = w.create(1, vec![(veth, 3)], 0, 0).unwrap();
    // The first mint is whole BETA: 1 BETA holds exactly 3.
    expect_err(w.mint(&basket, 1), "FirstMintNotWhole");
    w.mint(&basket, ONE).unwrap();
    assert_eq!(w.held(&basket, &veth), 3);
    // One smallest unit more holds 3e-9 of a unit: paid in as 1.
    w.mint(&basket, 1).unwrap();
    assert_eq!(w.held(&basket, &veth), 4);
    // Paid out as 0: every BETA stays fully backed.
    w.burn(&basket, 1, 0).unwrap();
    assert_eq!(w.held(&basket, &veth), 4);
}

#[test]
fn refuses_bad_baskets() {
    let mut w = World::new();
    let (wsol, veth) = (w.wsol, w.veth);
    expect_err(w.create(1, vec![], 0, 0), "BadParts");
    expect_err(w.create(1, vec![(wsol, ONE), (wsol, ONE)], 0, 0), "BadParts");
    expect_err(w.create(1, vec![(wsol, 0)], 0, 0), "BadParts");
    let nine: Vec<(Pubkey, u64)> = (0..9).map(|_| (Pubkey::new_unique(), 1)).collect();
    expect_err(w.create(1, nine, 0, 0), "BadParts");
    expect_err(w.create(1, vec![(wsol, ONE)], 101, 0), "FeeTooHigh");
    expect_err(w.create(1, vec![(wsol, ONE)], 0, 101), "FeeTooHigh");
    // Not a mint at all.
    expect_err(w.create(1, vec![(Pubkey::new_unique(), ONE)], 0, 0), "BadMint");
    // A token that takes a fee on transfer breaks the accounting: refused.
    let with_fee = w.mint22(false, true);
    expect_err(w.create(1, vec![(with_fee, ONE)], 0, 0), "BadMint");
    w.create(1, vec![(wsol, ONE)], 100, 100).unwrap();
    // Each basket once.
    assert!(w.create(1, vec![(veth, ONE)], 0, 0).is_err());
}

/// The issuer freezes the basket's account of one part. A burn that defers
/// it pays the other parts now; the frozen part is owed, and collected once
/// the issuer thaws the account.
#[test]
fn defers_a_frozen_part_and_pays_the_others() {
    let mut w = World::new();
    let creator = w.creator.insecure_clone();
    let frz = litesvm_token::CreateMint::new(&mut w.ctx.svm, &creator).decimals(6).freeze_authority(&creator.pubkey()).send().unwrap();
    w.programs.insert(frz, TOKEN);
    w.fund(&frz, 10_000_000);
    let wsol = w.wsol;
    let basket = w.create(1, vec![(wsol, ONE), (frz, 1_000_000)], 0, 10).unwrap();
    assert_eq!(w.basket(&basket).parts[1].powers, CAN_FREEZE);
    w.mint(&basket, 2 * ONE).unwrap();

    let held = ata(&basket, &frz, &TOKEN);
    let freeze = t22::instruction::freeze_account(&TOKEN, &held, &frz, &creator.pubkey(), &[]).unwrap();
    w.send(freeze, &[&creator]).unwrap();
    // Moving the frozen part fails the whole burn.
    assert!(w.burn(&basket, ONE, 0).is_err());
    // Deferring it: SOL now, less 0.1%; the frozen part owed, with no fee.
    let before = w.user_has(&wsol);
    w.burn(&basket, ONE, 0b10).unwrap();
    assert_eq!(w.user_has(&wsol), before + ONE - ONE / 1000);
    assert_eq!(w.basket(&basket).parts[1].owed, 1_000_000);
    // Not collected while the account is frozen.
    assert!(w.collect_owed(&basket, 1).is_err());
    let thaw = t22::instruction::thaw_account(&TOKEN, &held, &frz, &creator.pubkey(), &[]).unwrap();
    w.send(thaw, &[&creator]).unwrap();
    let before = w.user_has(&frz);
    let fees = w.tokens(&ata(&creator.pubkey(), &frz, &TOKEN));
    w.collect_owed(&basket, 1).unwrap();
    // The burn fee is taken when collected: deferring never avoids it.
    assert_eq!(w.user_has(&frz), before + 1_000_000 - 1_000);
    assert_eq!(w.tokens(&ata(&creator.pubkey(), &frz, &TOKEN)), fees + 1_000);
    assert_eq!(w.basket(&basket).parts[1].owed, 0);
    // The record is closed: its rent went back.
    assert!(w.ctx.svm.get_account(&owed_pda(&basket, &w.user.pubkey(), 1)).is_none_or(|a| a.lamports == 0));
    assert!(w.collect_owed(&basket, 1).is_err());
    // What was owed was set aside: the last BETA still has its full share.
    let before = w.user_has(&frz);
    w.burn(&basket, ONE, 0).unwrap();
    assert_eq!(w.user_has(&frz) - before, 1_000_000 - 1_000);
    assert_eq!(w.held(&basket, &frz), 0);
}

/// The issuer moves half of the basket's tokens out (a permanent delegate).
/// Every holder loses half of that part, equally; a new BETA deposits only
/// what the basket now holds per BETA, so it does not pay for the loss.
#[test]
fn shares_a_seized_part_equally_and_new_mints_follow_the_ratio() {
    let mut w = World::new();
    let creator = w.creator.insecure_clone();
    let stock = w.mint22(true, false);
    w.fund(&stock, 10 * ONE);
    let wsol = w.wsol;
    let basket = w.create(1, vec![(wsol, ONE), (stock, ONE)], 0, 0).unwrap();
    assert_eq!(w.basket(&basket).parts[1].powers, CAN_MOVE);
    assert_eq!(w.basket(&basket).parts[1].token_program, TOKEN22);
    w.mint(&basket, 2 * ONE).unwrap();
    assert_eq!(w.held(&basket, &stock), 2 * ONE);

    // The issuer burns 1 of the basket's 2 tokens.
    let held = ata(&basket, &stock, &TOKEN22);
    let seize = t22::instruction::burn(&TOKEN22, &held, &stock, &creator.pubkey(), &[], ONE).unwrap();
    w.send(seize, &[&creator]).unwrap();

    // A new BETA deposits 0.5 of the part, as the basket holds per BETA.
    let before = w.user_has(&stock);
    w.mint(&basket, ONE).unwrap();
    assert_eq!(before - w.user_has(&stock), ONE / 2);
    // Each BETA now pays 1 SOL and 0.5 of the part.
    let before = w.user_has(&stock);
    w.burn(&basket, ONE, 0).unwrap();
    assert_eq!(w.user_has(&stock) - before, ONE / 2);
    assert_eq!(w.held(&basket, &wsol), 2 * ONE);
}

#[test]
fn only_the_fee_receiver_hands_the_fee_on() {
    let mut w = World::new();
    let wsol = w.wsol;
    let basket = w.create(1, vec![(wsol, ONE)], 10, 10).unwrap();
    let other = Keypair::new();
    w.ctx.svm.airdrop(&other.pubkey(), SOL).unwrap();
    let ix = w.ix(beta_basket::client::accounts::SetFeeTo { basket, fee_to: other.pubkey() }, beta_basket::client::args::SetFeeTo { fee_to: other.pubkey() });
    assert!(w.send(ix, &[&other]).is_err());
    let creator = w.creator.insecure_clone();
    let ix = w.ix(beta_basket::client::accounts::SetFeeTo { basket, fee_to: creator.pubkey() }, beta_basket::client::args::SetFeeTo { fee_to: other.pubkey() });
    w.send(ix, &[&creator]).unwrap();
    assert_eq!(w.basket(&basket).fee_to, other.pubkey());
    let ix = anchor_spl::associated_token::spl_associated_token_account::instruction::create_associated_token_account_idempotent(&creator.pubkey(), &other.pubkey(), &wsol, &TOKEN);
    w.send(ix, &[&creator]).unwrap();
    w.mint(&basket, ONE).unwrap();
    assert_eq!(w.tokens(&ata(&other.pubkey(), &wsol, &TOKEN)), ONE / 1000);
}

/// Eight parts, one per network, in one mint and one burn.
#[test]
fn mints_and_burns_a_basket_of_eight_parts() {
    let mut w = World::new();
    let creator = w.creator.insecure_clone();
    let user = w.user.pubkey();
    let mut parts = vec![];
    for i in 0..8u64 {
        let m = litesvm_token::CreateMint::new(&mut w.ctx.svm, &creator).decimals(9).send().unwrap();
        w.programs.insert(m, TOKEN);
        w.fund(&m, 10 * ONE);
        parts.push((m, (i + 1) * ONE / 10));
    }
    let basket = w.create(8, parts.clone(), 10, 10).unwrap();
    w.mint(&basket, ONE).unwrap();
    w.burn(&basket, ONE, 0).unwrap();
    for (m, amount) in parts {
        // All paid back but the two fees, each rounded up.
        let fee = (amount as u128 * 10).div_ceil(10_000) as u64;
        assert_eq!(w.tokens(&ata(&user, &m, &TOKEN)), 10 * ONE - 2 * fee);
    }
}

/// Splitting a mint into tiny ones does not avoid the fee: it rounds up.
#[test]
fn rounds_the_fee_up() {
    let mut w = World::new();
    let veth = w.veth;
    // 1 BETA holds 50 units; 1% of 50 is 0.5, paid as 1.
    let basket = w.create(1, vec![(veth, 50)], 100, 100).unwrap();
    w.mint(&basket, ONE).unwrap();
    assert_eq!(w.tokens(&ata(&w.creator.pubkey(), &veth, &TOKEN)), 1);
}

/// With no fee, the fee receiver's account is not needed.
#[test]
fn needs_no_fee_account_without_a_fee() {
    let mut w = World::new();
    let wsol = w.wsol;
    let basket = w.create(1, vec![(wsol, ONE)], 0, 0).unwrap();
    let user = w.user.insecure_clone();
    let user_beta = w.user_beta(&basket);
    let mut ix = w.ix(
        beta_basket::client::accounts::Mint {
            basket,
            beta: beta_pda(&basket),
            user_beta,
            user: user.pubkey(),
            token_program: TOKEN,
            associated_token_program: ATA,
            system_program: SYSTEM,
        },
        beta_basket::client::args::Mint { amount: ONE },
    );
    let mut metas = w.part_metas(&basket);
    metas[3] = AccountMeta::new(Pubkey::new_unique(), false);
    ix.accounts.extend(metas);
    w.send(ix, &[&user]).unwrap();
    assert_eq!(w.held(&basket, &wsol), ONE);
}

/// A burn needs the burner's own BETA, at most what it holds, and its own
/// part accounts; a mint needs the basket's parts.
#[test]
fn refuses_bad_accounts_and_burns() {
    let mut w = World::new();
    let (wsol, veth) = (w.wsol, w.veth);
    let basket = w.create(1, vec![(wsol, ONE)], 0, 0).unwrap();
    w.mint(&basket, ONE).unwrap();
    assert!(w.burn(&basket, 2 * ONE, 0).is_err());
    // A deferral of a part the basket does not have.
    expect_err(w.burn(&basket, ONE, 0b10), "WrongAccount");
    let user = w.user.insecure_clone();
    let user_beta = w.user_beta(&basket);
    // The creator's account of the part, for the user's.
    let mut ix = w.ix(
        beta_basket::client::accounts::Burn { basket, beta: beta_pda(&basket), user_beta, user: user.pubkey(), token_program: TOKEN, system_program: SYSTEM },
        beta_basket::client::args::Burn { amount: ONE, defer: 0 },
    );
    let mut metas = w.part_metas(&basket);
    metas[2] = AccountMeta::new(ata(&w.creator.pubkey(), &wsol, &TOKEN), false);
    ix.accounts.extend(metas);
    expect_err(w.send(ix, &[&user]), "WrongAccount");
    // The wrong token for the part.
    let mut ix = w.ix(
        beta_basket::client::accounts::Mint {
            basket,
            beta: beta_pda(&basket),
            user_beta,
            user: user.pubkey(),
            token_program: TOKEN,
            associated_token_program: ATA,
            system_program: SYSTEM,
        },
        beta_basket::client::args::Mint { amount: ONE },
    );
    ix.accounts.extend([
        AccountMeta::new_readonly(veth, false),
        AccountMeta::new(ata(&basket, &veth, &TOKEN), false),
        AccountMeta::new(ata(&user.pubkey(), &veth, &TOKEN), false),
        AccountMeta::new(ata(&w.creator.pubkey(), &veth, &TOKEN), false),
        AccountMeta::new_readonly(TOKEN, false),
    ]);
    expect_err(w.send(ix, &[&user]), "WrongAccount");
    w.burn(&basket, ONE, 0).unwrap();
}

/// The issuer takes all of a part: minting stops, since that part would be
/// free for new holders; burns go on.
#[test]
fn stops_minting_while_a_part_is_empty() {
    let mut w = World::new();
    let creator = w.creator.insecure_clone();
    let stock = w.mint22(true, false);
    w.fund(&stock, 10 * ONE);
    let wsol = w.wsol;
    let basket = w.create(1, vec![(wsol, ONE), (stock, ONE)], 0, 0).unwrap();
    w.mint(&basket, ONE).unwrap();
    let held = ata(&basket, &stock, &TOKEN22);
    let seize = t22::instruction::burn(&TOKEN22, &held, &stock, &creator.pubkey(), &[], ONE).unwrap();
    w.send(seize, &[&creator]).unwrap();
    expect_err(w.mint(&basket, ONE), "PartEmpty");
    // The holder still gets its SOL.
    let before = w.user_has(&wsol);
    w.burn(&basket, ONE, 0).unwrap();
    assert_eq!(w.user_has(&wsol), before + ONE);
}
