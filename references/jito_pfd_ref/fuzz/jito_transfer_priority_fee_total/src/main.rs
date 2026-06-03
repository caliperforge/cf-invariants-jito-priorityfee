// invariant_transfer_priority_fee_tips_increments_total
//
// cf-invariants-jito-priorityfee fixture — transfer_increments_total_state_update class.
// Target: Crucible v0.2.0 (asymmetric-research/crucible).
//
// Structural invariant under test:
//
//     After every successful call to `transfer_priority_fee_tips(lamports)`,
//     the on-chain `PriorityFeeDistributionAccount.total_lamports_transferred`
//     field MUST advance by exactly `lamports` (no more, no less, never
//     unchanged). I.e. the program's structural-state commit of the
//     transfer total must actually happen, not just appear to succeed.
//
// Why this invariant, not lamport conservation:
//
//     The Solana SVM runtime enforces total-lamport conservation across
//     every instruction natively (any program that fails to balance
//     debits and credits has its tx rejected by the runtime). So a
//     lamport-conservation invariant cannot meaningfully observe a
//     violation — the runtime rejects the offending tx before the
//     fixture's `read_account` ever sees a discrepancy. A user-
//     meaningful structural invariant has to live in the post-
//     instruction state shape that the runtime does NOT police, e.g.
//     whether the on-chain `total_lamports_transferred` field tracks
//     the caller's sum.
//
//     This invariant is *priority-fee-distribution specific* (the
//     `total_lamports_transferred` field exists only on this program,
//     not on tip-distribution or tip-payment), and tests the program's
//     own value-add over its sibling tip-distribution.
//
// Setup (pre-bake — sidesteps the multi-step
// initialize/initialize_priority_fee_distribution_account flow that
// requires a real vote account):
//
//   1. Derive Config PDA from `[CONFIG_ACCOUNT]` seed; pre-bake at
//      go_live_epoch = 0 (always live, so transfer_priority_fee_tips
//      runs the system-program transfer branch, not the no-op log
//      branch).
//   2. Derive a PriorityFeeDistributionAccount PDA from
//      `[PF_DISTRIBUTION_ACCOUNT, vote_pk, epoch_created_at_le]` seeds
//      with epoch_created_at = 0 (matches the LiteSVM default epoch).
//      Pre-bake the account state with total_lamports_transferred = 0,
//      merkle_root = None, expires_at = 10.
//   3. Pre-fund `from` (the validator-signer paying the priority fee)
//      with enough lamports to cover repeated transfers + tx fees.
//
// Action surface:
//   - action_transfer_priority_fee_tips_small — call
//     transfer_priority_fee_tips with a small fixed lamport amount.
//     The fixture tracks `expected_total` (sum of successful transfers'
//     `lamports` arg).
//
// Invariant assertion:
//   on-chain PriorityFeeDistributionAccount.total_lamports_transferred
//   == fixture.expected_total. Clean reference: holds (program calls
//   increment_total_lamports_transferred on every successful transfer).
//   Planted twin (the increment call is dropped): first successful
//   transfer → on-chain stays at 0 while expected becomes lamports →
//   VIOLATION.

#![allow(unused_imports)]

use crucible_fuzzer::anchor_lang::system_program;
use crucible_fuzzer::*;
use ::jito_priority_fee_distribution::*;
use ::jito_priority_fee_distribution::state::{
    Config as PfdConfig, PriorityFeeDistributionAccount,
};
use solana_keypair::Keypair;
use solana_pubkey::Pubkey;
use solana_signer::Signer;
use std::rc::Rc;

/// Starting lamports for the validator-signer; large enough for many
/// transfers + tx fees but small enough that pre-baked rent doesn't go
/// wild.
const FROM_INITIAL_BALANCE: u64 = 10_000_000_000;
/// Lamports transferred on each call. Picked small enough to survive
/// the action-budget without overflowing FROM_INITIAL_BALANCE; the
/// invariant fires on call #1 in the planted variant, so the absolute
/// value doesn't matter for the planted-side detection.
const TRANSFER_LAMPORTS: u64 = 1_000_000;

#[derive(Clone)]
struct JitoTransferPriorityFeeTotalFixture {
    ctx: TestContext,
    program_id: Pubkey,
    /// The validator-signer paying the priority fee. Sole signer on the
    /// transfer_priority_fee_tips call (Anchor's `from: Signer<'info>`).
    from: Rc<Keypair>,
    config_pda: Pubkey,
    pfd_pda: Pubkey,
    /// Fixture-side oracle: the sum of `lamports` arguments across all
    /// successful transfer_priority_fee_tips calls so far. Starts at 0
    /// (matches the pre-baked PriorityFeeDistributionAccount's
    /// `total_lamports_transferred = 0`).
    expected_total: u64,
}

#[fuzz_fixture]
impl JitoTransferPriorityFeeTotalFixture {
    pub fn setup() -> Self {
        let mut ctx = TestContext::new();
        let program_id = Pubkey::new_from_array(ID.to_bytes());
        ctx.add_program(
            &program_id,
            "../../target/deploy/jito_priority_fee_distribution.so",
        )
        .unwrap();

        let from = Rc::new(Keypair::new());

        ctx.create_account()
            .pubkey(from.pubkey())
            .lamports(FROM_INITIAL_BALANCE)
            .owner(system_program::ID)
            .create()
            .unwrap();

        // Pre-bake Config:
        //   - go_live_epoch = 0 → transfer runs the system-program
        //     transfer branch (not the no-op log branch).
        //   - authority / expired_funds_account: arbitrary non-default
        //     keys (only authority/expired-side validation matters; this
        //     fixture never calls update_config or close_*).
        //   - num_epochs_valid: 10 (max allowed by Config::validate).
        //   - max_validator_commission_bps: 10000.
        let (config_pda, config_bump) =
            Pubkey::find_program_address(&[PfdConfig::SEED], &program_id);
        let authority_key = Keypair::new().pubkey();
        let expired_funds_key = Keypair::new().pubkey();
        let cfg = PfdConfig {
            authority: authority_key,
            expired_funds_account: expired_funds_key,
            num_epochs_valid: 10,
            max_validator_commission_bps: 10_000,
            go_live_epoch: 0,
            bump: config_bump,
        };

        use crucible_fuzzer::anchor_lang::prelude::Rent;
        let rent = Rent::default();
        let rent_min_for_config = rent.minimum_balance(PfdConfig::SIZE);
        ctx.create_account()
            .pubkey(config_pda)
            .lamports(rent_min_for_config)
            .owner(program_id)
            .size(PfdConfig::SIZE)
            .create()
            .unwrap();
        ctx.write_anchor_account(&config_pda, &cfg).unwrap();

        // Pre-bake the PriorityFeeDistributionAccount.
        //
        // Seeds:
        //   [PF_DISTRIBUTION_ACCOUNT, validator_vote_account, epoch_created_at.to_le_bytes()]
        //
        // We can't call initialize_priority_fee_distribution_account
        // because it deserializes a vote account via VoteState (real
        // account owned by the vote program). Instead we pre-bake the
        // account directly. The vote-account-key we put in the seeds is
        // arbitrary; the program never re-derives the PDA in
        // transfer_priority_fee_tips (the `mut` `Account<...>` is just
        // an Anchor account constraint without seeds re-check), so any
        // self-consistent key works.
        let vote_account_key = Keypair::new().pubkey();
        let epoch_created_at: u64 = 0;
        let (pfd_pda, pfd_bump) = Pubkey::find_program_address(
            &[
                PriorityFeeDistributionAccount::SEED,
                vote_account_key.as_ref(),
                &epoch_created_at.to_le_bytes(),
            ],
            &program_id,
        );
        let pfd_state = PriorityFeeDistributionAccount {
            validator_vote_account: vote_account_key,
            merkle_root_upload_authority: Keypair::new().pubkey(),
            merkle_root: None,
            epoch_created_at,
            validator_commission_bps: 0,
            expires_at: 10,
            total_lamports_transferred: 0,
            bump: pfd_bump,
        };
        let rent_min_for_pfd = rent.minimum_balance(PriorityFeeDistributionAccount::SIZE);
        ctx.create_account()
            .pubkey(pfd_pda)
            .lamports(rent_min_for_pfd)
            .owner(program_id)
            .size(PriorityFeeDistributionAccount::SIZE)
            .create()
            .unwrap();
        ctx.write_anchor_account(&pfd_pda, &pfd_state).unwrap();

        Self {
            ctx,
            program_id,
            from,
            config_pda,
            pfd_pda,
            expected_total: 0,
        }
    }

    /// Call transfer_priority_fee_tips with a small fixed lamport amount.
    pub fn action_transfer_priority_fee_tips_small(&mut self) -> bool {
        let result = self
            .ctx
            .program(self.program_id)
            .call(instruction::TransferPriorityFeeTips {
                lamports: TRANSFER_LAMPORTS,
            })
            .accounts(accounts::TransferPriorityFeeTips {
                config: self.config_pda,
                priority_fee_distribution_account: self.pfd_pda,
                from: self.from.pubkey(),
                system_program: system_program::ID,
            })
            .signers(&[&*self.from])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false);

        if result {
            // From the caller's perspective the transfer committed.
            // Advance the fixture oracle.
            self.expected_total = self
                .expected_total
                .saturating_add(TRANSFER_LAMPORTS);
        }
        result
    }
}

// transfer_priority_fee_tips_increments_total invariant.
//
// On-chain PriorityFeeDistributionAccount.total_lamports_transferred must
// equal the fixture's expected_total after every action. Clean: holds.
// Planted (the increment call is missing in the program): on-chain stays
// at 0 while expected advances → VIOLATION.
#[invariant_test]
fn invariant_transfer_priority_fee_tips_increments_total(
    fixture: &mut JitoTransferPriorityFeeTotalFixture,
) {
    // 8-byte Anchor discriminator (the PriorityFeeDistributionAccount
    // has the standard `#[account]` derive → 8-byte
    // sha256("account:PriorityFeeDistributionAccount")[..8] prefix).
    let pfd_state = fixture
        .ctx
        .read_account_with_discriminator::<PriorityFeeDistributionAccount>(&fixture.pfd_pda, 8)
        .expect("priority-fee-distribution account exists (pre-baked) and deserializes");

    fuzz_assert_eq!(
        pfd_state.total_lamports_transferred,
        fixture.expected_total,
        "PriorityFeeDistributionAccount.total_lamports_transferred drift: on-chain={} expected={} (sum of caller's transfer lamports)",
        pfd_state.total_lamports_transferred,
        fixture.expected_total
    );
}
