//! Tests of BETA baskets (docs/specs/ipow-beta-app.md): parts standing for
//! wrapped SOL and vETH, and parts whose issuer controls them (E3): one it
//! can freeze, one it can move out of any account (Token-2022's permanent
//! delegate). A creator fee on mint and on burn, rounding in the basket's
//! favour, and the checks on baskets and accounts.

use anchor_lang;
use anchor_lang::solana_program::instruction::{AccountMeta, Instruction};
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
/// Token Metadata, as the program names it (its id and seed are not in the IDL).
const METADATA_PROGRAM: Pubkey = Pubkey::from_str_const("metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s");
const METADATA_SEED: &[u8] = b"metadata";
const CAN_FREEZE: u8 = 1;
const CAN_MOVE: u8 = 4;

fn bk(seeds: &[&[u8]]) -> Pubkey {
    Pubkey::find_program_address(seeds, &beta_basket::ID).0
}
fn basket_pda(creator: &Pubkey, id: u64) -> Pubkey {
    bk(&[b"basket", creator.as_ref(), &id.to_le_bytes()])
}
fn metadata_pda(mint: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[METADATA_SEED, METADATA_PROGRAM.as_ref(), mint.as_ref()], &METADATA_PROGRAM).0
}

fn beta_pda(basket: &Pubkey) -> Pubkey {
    bk(&[b"beta", basket.as_ref()])
}
fn owed_pda(basket: &Pubkey, user: &Pubkey, i: u8) -> Pubkey {
    bk(&[b"owed", basket.as_ref(), user.as_ref(), &[i]])
}
fn fees_pda(basket: &Pubkey, receiver: &Pubkey) -> Pubkey {
    bk(&[b"fees", basket.as_ref(), receiver.as_ref()])
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
        // Token Metadata, as deployed (fixtures/mpl_token_metadata.so, dumped
        // from mainnet on 2026-10-05): a basket's token is named through it.
        let mut ctx = AnchorLiteSVM::build_with_programs(&[
            (beta_basket::ID, include_bytes!("../../../../target/deploy/beta_basket.so")),
            (METADATA_PROGRAM, include_bytes!("fixtures/mpl_token_metadata.so")),
        ]);
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

    fn send_all(&mut self, ixs: Vec<Instruction>, signers: &[&Keypair]) -> Result<(), String> {
        self.ctx.svm.expire_blockhash();
        let r = self.ctx.execute_instructions(ixs, signers).unwrap();
        if r.is_success() { Ok(()) } else { Err(r.logs().join("\n")) }
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
        self.create_named(id, "Test Basket", "TBSK", "data:application/json,{}", parts, mint_fee_bps, burn_fee_bps)
    }

    fn create_named(&mut self, id: u64, name: &str, symbol: &str, uri: &str, parts: Vec<(Pubkey, u64)>, mint_fee_bps: u16, burn_fee_bps: u16) -> Result<Pubkey, String> {
        let creator = self.creator.insecure_clone();
        let basket = basket_pda(&creator.pubkey(), id);
        let mints: Vec<Pubkey> = parts.iter().map(|(m, _)| *m).collect();
        let mut ix = self.ix(
            beta_basket::client::accounts::CreateBasket {
                basket,
                beta: beta_pda(&basket),
                fees: fees_pda(&basket, &creator.pubkey()),
                metadata: metadata_pda(&beta_pda(&basket)),
                creator: creator.pubkey(),
                token_program: TOKEN,
                system_program: SYSTEM,
                metadata_program: METADATA_PROGRAM,
            },
            beta_basket::client::args::CreateBasket {
                id,
                name: name.to_string(),
                symbol: symbol.to_string(),
                uri: uri.to_string(),
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

    /// The four accounts of each part, as the user sees them.
    fn part_metas(&self, basket: &Pubkey) -> Vec<AccountMeta> {
        let b = self.basket(basket);
        let user = self.user.pubkey();
        let mut out = vec![];
        for p in &b.parts {
            // A pending part (E5) has no program recorded: the client knows it.
            let program = if p.token_program == Pubkey::default() { self.programs[&p.mint] } else { p.token_program };
            out.push(AccountMeta::new_readonly(p.mint, false));
            out.push(AccountMeta::new(ata(basket, &p.mint, &program), false));
            out.push(AccountMeta::new(ata(&user, &p.mint, &program), false));
            out.push(AccountMeta::new_readonly(program, false));
        }
        out
    }

    /// The fee receiver's record of fees; none once the fee goes to the
    /// basket itself.
    fn fees_of(&self, basket: &Pubkey) -> Option<Pubkey> {
        let to = self.basket(basket).fee_to;
        (to != *basket).then(|| fees_pda(basket, &to))
    }

    /// The fees `receiver` has set aside in a basket, by part.
    fn fees(&self, basket: &Pubkey, receiver: &Pubkey) -> [u64; 8] {
        let f: beta_basket::accounts::Fees = self.ctx.get_account(&fees_pda(basket, receiver)).unwrap();
        f.amounts
    }

    /// What the basket holds of a part for its holders: its account, less
    /// what is set aside.
    fn for_holders(&self, basket: &Pubkey, i: usize) -> u64 {
        let p = self.basket(basket).parts[i];
        self.tokens(&ata(basket, &p.mint, &p.token_program)) - p.owed
    }

    fn set_fee_to(&mut self, basket: &Pubkey, from: &Keypair, to: Pubkey) -> Result<(), String> {
        let new_fees = (to != *basket).then(|| fees_pda(basket, &to));
        let ix = self.ix(
            beta_basket::client::accounts::SetFeeTo { basket: *basket, new_fees, fee_to: from.pubkey(), system_program: SYSTEM },
            beta_basket::client::args::SetFeeTo { fee_to: to },
        );
        self.send(ix, &[from])
    }

    /// `receiver` collects its fees of part `index` into token account `to`.
    fn collect_fees(&mut self, basket: &Pubkey, receiver: &Keypair, index: u8, to: Pubkey) -> Result<(), String> {
        let mut ix = self.ix(
            beta_basket::client::accounts::CollectFees { basket: *basket, fees: fees_pda(basket, &receiver.pubkey()), receiver: receiver.pubkey() },
            beta_basket::client::args::CollectFees { index },
        );
        let mut metas = self.part_metas(basket)[4 * index as usize..4 * index as usize + 4].to_vec();
        metas[2] = AccountMeta::new(to, false);
        ix.accounts.extend(metas);
        self.send(ix, &[receiver])
    }

    fn mint(&mut self, basket: &Pubkey, amount: u64) -> Result<(), String> {
        let user = self.user.insecure_clone();
        let user_beta = self.user_beta(basket);
        let mut ix = self.ix(
            beta_basket::client::accounts::Mint {
                basket: *basket,
                fees: self.fees_of(basket),
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
                fees: self.fees_of(basket),
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
            beta_basket::client::accounts::CollectOwed { basket: *basket, owed: owed_pda(basket, &user.pubkey(), index), fees: self.fees_of(basket), user: user.pubkey() },
            beta_basket::client::args::CollectOwed { index },
        );
        let metas = self.part_metas(basket);
        ix.accounts.extend(metas[4 * index as usize..4 * index as usize + 4].iter().cloned());
        self.send(ix, &[&user])
    }
}

#[test]
fn mints_and_burns_a_basket_of_two_parts_with_the_creators_fee() {
    let mut w = World::new();
    // 1 BETA = 1 SOL + 0.5 vETH; 0.3% on mint, 0.2% on burn.
    let basket = w.create(1, vec![(w.wsol, ONE), (w.veth, ONE / 2)], 30, 20).unwrap();
    let creator = w.creator.pubkey();
    let wsol = w.wsol;

    w.mint(&basket, 2 * ONE).unwrap();
    assert_eq!(w.tokens(&ata(&w.user.pubkey(), &beta_pda(&basket), &TOKEN)), 2 * ONE);
    // The holders have 2 SOL and 1 vETH; the fee is 0.3% of each on top,
    // set aside in the basket for the creator.
    assert_eq!(w.for_holders(&basket, 0), 2 * ONE);
    assert_eq!(w.for_holders(&basket, 1), ONE);
    assert_eq!(w.held(&basket, &wsol), 2 * ONE + 2 * ONE * 3 / 1000);
    assert_eq!(w.fees(&basket, &creator)[..2], [2 * ONE * 3 / 1000, ONE * 3 / 1000]);
    assert_eq!(w.user_has(&wsol), 10 * ONE - 2 * ONE - 2 * ONE * 3 / 1000);

    // Burning 1 BETA gives half of what the holders have, less 0.2%.
    w.burn(&basket, ONE, 0).unwrap();
    assert_eq!(w.for_holders(&basket, 0), ONE);
    assert_eq!(w.for_holders(&basket, 1), ONE / 2);
    assert_eq!(w.fees(&basket, &creator)[0], 2 * ONE * 3 / 1000 + ONE * 2 / 1000);
    // The creator collects its fees.
    let c = w.creator.insecure_clone();
    let before = w.tokens(&ata(&creator, &wsol, &TOKEN));
    w.collect_fees(&basket, &c, 0, ata(&creator, &wsol, &TOKEN)).unwrap();
    assert_eq!(w.tokens(&ata(&creator, &wsol, &TOKEN)), before + 2 * ONE * 3 / 1000 + ONE * 2 / 1000);
    expect_err(w.collect_fees(&basket, &c, 0, ata(&creator, &wsol, &TOKEN)), "NothingOwed");
    assert_eq!(w.for_holders(&basket, 0), ONE);
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
    // An account that holds data but is no mint (a token account) is
    // refused; one that does not exist is a receipt not made yet (E5).
    let not_a_mint = ata(&w.user.pubkey(), &wsol, &TOKEN);
    expect_err(w.create(1, vec![(not_a_mint, ONE)], 0, 0), "BadMint");
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
    w.collect_owed(&basket, 1).unwrap();
    // The burn fee is taken when collected: deferring never avoids it.
    assert_eq!(w.user_has(&frz), before + 1_000_000 - 1_000);
    assert_eq!(w.fees(&basket, &creator.pubkey())[1], 1_000);
    assert_eq!(w.basket(&basket).parts[1].owed, 1_000);
    // The record is closed: its rent went back.
    assert!(w.ctx.svm.get_account(&owed_pda(&basket, &w.user.pubkey(), 1)).is_none_or(|a| a.lamports == 0));
    assert!(w.collect_owed(&basket, 1).is_err());
    // What was owed was set aside: the last BETA still has its full share.
    let before = w.user_has(&frz);
    w.burn(&basket, ONE, 0).unwrap();
    assert_eq!(w.user_has(&frz) - before, 1_000_000 - 1_000);
    assert_eq!(w.for_holders(&basket, 1), 0);
    assert_eq!(w.held(&basket, &frz), 2_000);
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
    assert!(w.set_fee_to(&basket, &other, other.pubkey()).is_err());
    let creator = w.creator.insecure_clone();
    w.mint(&basket, ONE).unwrap();
    w.set_fee_to(&basket, &creator, other.pubkey()).unwrap();
    assert_eq!(w.basket(&basket).fee_to, other.pubkey());
    w.mint(&basket, ONE).unwrap();
    // Each receiver keeps what it earned.
    assert_eq!(w.fees(&basket, &creator.pubkey())[0], ONE / 1000);
    assert_eq!(w.fees(&basket, &other.pubkey())[0], ONE / 1000);
    let ix = anchor_spl::associated_token::spl_associated_token_account::instruction::create_associated_token_account_idempotent(&creator.pubkey(), &other.pubkey(), &wsol, &TOKEN);
    w.send(ix, &[&creator]).unwrap();
    w.collect_fees(&basket, &other, 0, ata(&other.pubkey(), &wsol, &TOKEN)).unwrap();
    assert_eq!(w.tokens(&ata(&other.pubkey(), &wsol, &TOKEN)), ONE / 1000);
    w.collect_fees(&basket, &creator, 0, ata(&creator.pubkey(), &wsol, &TOKEN)).unwrap();
    // Handed to the basket itself: later fees back BETA, and nobody can hand
    // it on again.
    w.set_fee_to(&basket, &other, basket).unwrap();
    w.mint(&basket, ONE).unwrap();
    assert_eq!(w.for_holders(&basket, 0), 3 * ONE + ONE / 1000);
    assert!(w.set_fee_to(&basket, &other, other.pubkey()).is_err());
}

/// The issuer freezes the fee receiver's account: mints, burns and
/// collections of what is owed go on, the fees set aside; the receiver
/// collects into another account.
#[test]
fn a_frozen_fee_account_stops_no_holder() {
    let mut w = World::new();
    let creator = w.creator.insecure_clone();
    let frz = litesvm_token::CreateMint::new(&mut w.ctx.svm, &creator).decimals(6).freeze_authority(&creator.pubkey()).send().unwrap();
    w.programs.insert(frz, TOKEN);
    w.fund(&frz, 10_000_000);
    let basket = w.create(1, vec![(frz, 1_000_000)], 100, 100).unwrap();
    let mine = ata(&creator.pubkey(), &frz, &TOKEN);
    let freeze = t22::instruction::freeze_account(&TOKEN, &mine, &frz, &creator.pubkey(), &[]).unwrap();
    w.send(freeze, &[&creator]).unwrap();
    w.mint(&basket, 3 * ONE).unwrap();
    w.burn(&basket, ONE, 0).unwrap();
    w.burn(&basket, ONE, 0b1).unwrap();
    w.collect_owed(&basket, 0).unwrap();
    // 1% of 3, of 1 and of the 1 collected.
    assert_eq!(w.fees(&basket, &creator.pubkey())[0], 30_000 + 10_000 + 10_000);
    // Not into its frozen account; into another of its own.
    assert!(w.collect_fees(&basket, &creator, 0, mine).is_err());
    let user = w.user.pubkey();
    w.collect_fees(&basket, &creator, 0, ata(&user, &frz, &TOKEN)).unwrap();
    assert_eq!(w.for_holders(&basket, 0), 1_000_000);
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
    assert_eq!(w.fees(&basket, &w.creator.pubkey())[0], 1);
}

/// With no fee, the fee receiver's record is not needed; with one, it is.
#[test]
fn needs_the_fees_record_only_with_a_fee() {
    let mut w = World::new();
    let wsol = w.wsol;
    let user = w.user.insecure_clone();
    for (id, bps) in [(1u64, 0u16), (2, 10)] {
        let basket = w.create(id, vec![(wsol, ONE)], bps, 0).unwrap();
        let user_beta = w.user_beta(&basket);
        let mut ix = w.ix(
            beta_basket::client::accounts::Mint {
                basket,
                fees: None,
                beta: beta_pda(&basket),
                user_beta,
                user: user.pubkey(),
                token_program: TOKEN,
                associated_token_program: ATA,
                system_program: SYSTEM,
            },
            beta_basket::client::args::Mint { amount: ONE },
        );
        ix.accounts.extend(w.part_metas(&basket));
        let r = w.send(ix, &[&user]);
        if bps == 0 {
            r.unwrap();
            assert_eq!(w.held(&basket, &wsol), ONE);
        } else {
            expect_err(r, "NoFeesRecord");
        }
    }
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
        beta_basket::client::accounts::Burn { basket, fees: w.fees_of(&basket), beta: beta_pda(&basket), user_beta, user: user.pubkey(), token_program: TOKEN, system_program: SYSTEM },
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
            fees: w.fees_of(&basket),
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

/// A fee always reaches the current receiver's record: a mint, a burn or a
/// collection cannot name another receiver's record, nor skip it.
#[test]
fn takes_the_fee_only_into_the_current_receivers_record() {
    let mut w = World::new();
    let wsol = w.wsol;
    let creator = w.creator.insecure_clone();
    let basket = w.create(1, vec![(wsol, ONE)], 10, 10).unwrap();
    let other = Keypair::new();
    w.ctx.svm.airdrop(&other.pubkey(), SOL).unwrap();
    w.mint(&basket, 2 * ONE).unwrap();
    w.burn(&basket, ONE / 2, 0b1).unwrap();
    w.set_fee_to(&basket, &creator, other.pubkey()).unwrap();
    let user = w.user.insecure_clone();
    let user_beta = w.user_beta(&basket);
    let old = Some(fees_pda(&basket, &creator.pubkey()));
    for fees in [old, None] {
        let mut ix = w.ix(
            beta_basket::client::accounts::Mint {
                basket,
                fees,
                beta: beta_pda(&basket),
                user_beta,
                user: user.pubkey(),
                token_program: TOKEN,
                associated_token_program: ATA,
                system_program: SYSTEM,
            },
            beta_basket::client::args::Mint { amount: ONE },
        );
        ix.accounts.extend(w.part_metas(&basket));
        assert!(w.send(ix, &[&user]).is_err());
        let mut ix = w.ix(
            beta_basket::client::accounts::Burn { basket, fees, beta: beta_pda(&basket), user_beta, user: user.pubkey(), token_program: TOKEN, system_program: SYSTEM },
            beta_basket::client::args::Burn { amount: ONE / 2, defer: 0 },
        );
        ix.accounts.extend(w.part_metas(&basket));
        assert!(w.send(ix, &[&user]).is_err());
        let mut ix = w.ix(
            beta_basket::client::accounts::CollectOwed { basket, owed: owed_pda(&basket, &user.pubkey(), 0), fees, user: user.pubkey() },
            beta_basket::client::args::CollectOwed { index: 0 },
        );
        ix.accounts.extend(w.part_metas(&basket));
        assert!(w.send(ix, &[&user]).is_err());
    }
    // With the right record, all three go through, into the new receiver's.
    let before = w.fees(&basket, &creator.pubkey());
    w.mint(&basket, ONE).unwrap();
    w.burn(&basket, ONE / 2, 0).unwrap();
    w.collect_owed(&basket, 0).unwrap();
    assert_eq!(w.fees(&basket, &creator.pubkey()), before);
    assert!(w.fees(&basket, &other.pubkey())[0] > 0);
}

/// Handing the fee on: never to nobody or the program; back to an earlier
/// receiver, whose record keeps what it earned; to an address someone sent
/// lamports to first.
#[test]
fn hands_the_fee_on_to_any_receiver_that_can_collect() {
    let mut w = World::new();
    let wsol = w.wsol;
    let creator = w.creator.insecure_clone();
    let basket = w.create(1, vec![(wsol, ONE)], 10, 0).unwrap();
    w.mint(&basket, ONE).unwrap();
    expect_err(w.set_fee_to(&basket, &creator, Pubkey::default()), "WrongAccount");
    expect_err(w.set_fee_to(&basket, &creator, beta_basket::ID), "WrongAccount");
    let other = Keypair::new();
    w.ctx.svm.airdrop(&other.pubkey(), SOL).unwrap();
    // Someone sends lamports to the new record's address first.
    w.ctx.svm.airdrop(&fees_pda(&basket, &other.pubkey()), 1_000_000).unwrap();
    w.set_fee_to(&basket, &creator, other.pubkey()).unwrap();
    w.mint(&basket, ONE).unwrap();
    // To itself, and back to the creator: each record keeps its fees.
    w.set_fee_to(&basket, &other, other.pubkey()).unwrap();
    w.set_fee_to(&basket, &other, creator.pubkey()).unwrap();
    w.mint(&basket, ONE).unwrap();
    assert_eq!(w.fees(&basket, &creator.pubkey())[0], 2 * ONE / 1000);
    assert_eq!(w.fees(&basket, &other.pubkey())[0], ONE / 1000);
    // A collection names the basket's own account of the part.
    let mut ix = w.ix(
        beta_basket::client::accounts::CollectFees { basket, fees: fees_pda(&basket, &creator.pubkey()), receiver: creator.pubkey() },
        beta_basket::client::args::CollectFees { index: 0 },
    );
    ix.accounts.extend([
        AccountMeta::new_readonly(wsol, false),
        AccountMeta::new(ata(&creator.pubkey(), &wsol, &TOKEN), false),
        AccountMeta::new(ata(&creator.pubkey(), &wsol, &TOKEN), false),
        AccountMeta::new_readonly(TOKEN, false),
    ]);
    expect_err(w.send(ix, &[&creator]), "WrongAccount");
    // Into an account of another token: refused.
    let veth = w.veth;
    expect_err(w.collect_fees(&basket, &creator, 0, ata(&creator.pubkey(), &veth, &TOKEN)), "WrongAccount");
    w.collect_fees(&basket, &creator, 0, ata(&creator.pubkey(), &wsol, &TOKEN)).unwrap();
}

/// The name, symbol and URI a Token Metadata account holds (fixed-width, NUL-padded).
fn read_metadata(data: &[u8]) -> (String, String, String) {
    let mut at = 1 + 32 + 32;
    let mut str = |width: usize| {
        let len = u32::from_le_bytes(data[at..at + 4].try_into().unwrap()) as usize;
        let v = String::from_utf8(data[at + 4..at + 4 + len].to_vec()).unwrap().trim_end_matches('\0').to_string();
        at += 4 + width;
        v
    };
    (str(32), str(10), str(200))
}

#[test]
fn names_the_token_as_its_creator_does_and_refuses_bad_names() {
    let mut w = World::new();
    let basket = w.create_named(9, "Gold & Treasuries", "GTB", "data:application/json,{\"description\":\"x\"}", vec![(w.wsol, ONE)], 25, 50).unwrap();
    let meta = w.ctx.svm.get_account(&metadata_pda(&beta_pda(&basket))).expect("a metadata account");
    assert_eq!(meta.owner, METADATA_PROGRAM);
    assert_eq!(read_metadata(&meta.data), ("Gold & Treasuries".into(), "GTB".into(), "data:application/json,{\"description\":\"x\"}".into()));
    // Immutable, with the basket as its update authority: Metadata V1 is
    // key (1), update authority (32), mint (32), the three strings (36, 14,
    // 204), the seller fee (2), creators None (1), primary sale (1), mutable (1).
    assert_eq!(meta.data[0], 4, "a MetadataV1 account");
    assert_eq!(Pubkey::new_from_array(meta.data[1..33].try_into().unwrap()), basket);
    assert_eq!(meta.data[323], 0, "immutable");
    // The program's own refusals, before Token Metadata sees anything.
    expect_err(w.create_named(10, "", "X", "", vec![(w.wsol, ONE)], 0, 0), "BadMetadata");
    expect_err(w.create_named(10, "N", "", "", vec![(w.wsol, ONE)], 0, 0), "BadMetadata");
    expect_err(w.create_named(10, &"n".repeat(33), "X", "", vec![(w.wsol, ONE)], 0, 0), "BadMetadata");
    expect_err(w.create_named(10, "N", &"s".repeat(11), "", vec![(w.wsol, ONE)], 0, 0), "BadMetadata");
    expect_err(w.create_named(10, "N", "X", &"u".repeat(201), vec![(w.wsol, ONE)], 0, 0), "BadMetadata");
    w.create_named(10, &"n".repeat(32), &"s".repeat(10), &"u".repeat(200), vec![(w.wsol, ONE)], 0, 0).unwrap();
}

#[test]
fn takes_a_part_whose_mint_is_not_made_yet_and_mints_once_it_is() {
    let mut w = World::new();
    // A receipt the vault has not made: its address is known, its account empty.
    let future = Keypair::new();
    let basket = w.create(11, vec![(w.wsol, ONE), (future.pubkey(), ONE)], 0, 0).unwrap();
    let b = w.basket(&basket);
    assert_eq!(b.parts[1].token_program, Pubkey::default(), "pending");
    assert_eq!(b.parts[1].decimals, 0);
    // Nothing can be minted while it is not made.
    w.programs.insert(future.pubkey(), TOKEN);
    expect_err(w.mint(&basket, ONE), "PartNotMadeYet");
    // The vault makes it (here, the test): a classic mint of 9 decimals, the user funded.
    let creator = w.creator.insecure_clone();
    let rent = w.ctx.svm.minimum_balance_for_rent_exemption(82);
    let create = anchor_lang::solana_program::system_instruction::create_account(&creator.pubkey(), &future.pubkey(), rent, 82, &TOKEN);
    let mut init = vec![20u8, 9]; // InitializeMint2, 9 decimals, the creator its authority, no freeze authority
    init.extend_from_slice(creator.pubkey().as_ref());
    init.push(0);
    let init = Instruction { program_id: TOKEN, accounts: vec![AccountMeta::new(future.pubkey(), false)], data: init };
    w.send_all(vec![create, init], &[&creator, &future]).unwrap();
    w.fund(&future.pubkey(), 10 * ONE);
    w.mint(&basket, ONE).unwrap();
    let b = w.basket(&basket);
    assert_eq!(b.parts[1].token_program, TOKEN, "read when first minted");
    assert_eq!(b.parts[1].decimals, 9);
    assert_eq!(w.user_has(&future.pubkey()), 9 * ONE);
}
