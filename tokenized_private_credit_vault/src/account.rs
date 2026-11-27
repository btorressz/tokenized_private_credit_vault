//! Anchor account validation contexts for instruction handlers.

use anchor_lang::prelude::*;
use crate::state::{
    CovenantState, EventLog, Facility, InvestorEligibility, RepaymentSchedule, VaultExtension,
};

#[derive(Accounts)]
#[instruction(facility_id: String, event_index: u64)]
pub struct InitializeFacility<'info> {
    #[account(mut)]
    pub operator: Signer<'info>,
    /// CHECK: Borrower identity key.
    pub borrower: UncheckedAccount<'info>,
    #[account(
        init,
        payer = operator,
        seeds = [b"facility", facility_id.as_bytes()],
        bump,
        space = 8 + Facility::INIT_SPACE
    )]
    pub facility: Account<'info, Facility>,

    #[account(
        init,
        payer = operator,
        seeds = [b"repayment", facility.key().as_ref()],
        bump,
        space = 8 + RepaymentSchedule::INIT_SPACE
    )]
    pub repayment_schedule: Account<'info, RepaymentSchedule>,

    #[account(
        init,
        payer = operator,
        seeds = [b"covenant", facility.key().as_ref()],
        bump,
        space = 8 + CovenantState::INIT_SPACE
    )]
    pub covenant_state: Account<'info, CovenantState>,

    #[account(
        init,
        payer = operator,
        seeds = [b"event", facility.key().as_ref(), event_index.to_le_bytes().as_ref()],
        bump,
        space = 8 + EventLog::INIT_SPACE
    )]
    pub event_log: Account<'info, EventLog>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(event_index: u64)]
pub struct ActivateFacility<'info> {
    #[account(mut)]
    pub operator: Signer<'info>,
    #[account(
        mut,
        seeds = [b"facility", facility.facility_id.as_bytes()],
        bump = facility.bump
    )]
    pub facility: Account<'info, Facility>,

    #[account(
        init,
        payer = operator,
        seeds = [b"event", facility.key().as_ref(), event_index.to_le_bytes().as_ref()],
        bump,
        space = 8 + EventLog::INIT_SPACE
    )]
    pub event_log: Account<'info, EventLog>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(_amount: u64, event_index: u64)]
pub struct DepositCapital<'info> {
    #[account(mut)]
    pub investor: Signer<'info>,
    #[account(
        mut,
        seeds = [b"facility", facility.facility_id.as_bytes()],
        bump = facility.bump
    )]
    pub facility: Account<'info, Facility>,

    /// CHECK: PDA constrained; manually deserialized, owner checked and saved.
    #[account(
        mut,
        seeds = [b"investor", facility.key().as_ref(), investor.key().as_ref()],
        bump
    )]
    pub investor_position: UncheckedAccount<'info>,

    #[account(
        init,
        payer = investor,
        seeds = [b"event", facility.key().as_ref(), event_index.to_le_bytes().as_ref()],
        bump,
        space = 8 + EventLog::INIT_SPACE
    )]
    pub event_log: Account<'info, EventLog>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(_amount: u64, event_index: u64)]
pub struct BorrowerDrawdown<'info> {
    #[account(mut)]
    pub actor: Signer<'info>,
    #[account(
        mut,
        seeds = [b"facility", facility.facility_id.as_bytes()],
        bump = facility.bump
    )]
    pub facility: Account<'info, Facility>,

    #[account(
        mut,
        seeds = [b"repayment", facility.key().as_ref()],
        bump = repayment_schedule.bump
    )]
    pub repayment_schedule: Account<'info, RepaymentSchedule>,

    #[account(
        init,
        payer = actor,
        seeds = [b"event", facility.key().as_ref(), event_index.to_le_bytes().as_ref()],
        bump,
        space = 8 + EventLog::INIT_SPACE
    )]
    pub event_log: Account<'info, EventLog>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(event_index: u64)]
pub struct AccrueInterest<'info> {
    #[account(mut)]
    pub operator: Signer<'info>,
    #[account(
        mut,
        seeds = [b"facility", facility.facility_id.as_bytes()],
        bump = facility.bump
    )]
    pub facility: Account<'info, Facility>,

    #[account(
        init,
        payer = operator,
        seeds = [b"event", facility.key().as_ref(), event_index.to_le_bytes().as_ref()],
        bump,
        space = 8 + EventLog::INIT_SPACE
    )]
    pub event_log: Account<'info, EventLog>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(_amount: u64, _principal_portion: u64, _interest_portion: u64, _scheduled_payment_satisfied: bool, event_index: u64)]
pub struct MakeRepayment<'info> {
    #[account(mut)]
    pub borrower: Signer<'info>,
    #[account(
        mut,
        seeds = [b"facility", facility.facility_id.as_bytes()],
        bump = facility.bump
    )]
    pub facility: Account<'info, Facility>,

    #[account(
        mut,
        seeds = [b"repayment", facility.key().as_ref()],
        bump = repayment_schedule.bump
    )]
    pub repayment_schedule: Account<'info, RepaymentSchedule>,

    #[account(
        init,
        payer = borrower,
        seeds = [b"event", facility.key().as_ref(), event_index.to_le_bytes().as_ref()],
        bump,
        space = 8 + EventLog::INIT_SPACE
    )]
    pub event_log: Account<'info, EventLog>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(event_index: u64)]
pub struct RecordMissedPayment<'info> {
    #[account(mut)]
    pub operator: Signer<'info>,
    #[account(
        mut,
        seeds = [b"facility", facility.facility_id.as_bytes()],
        bump = facility.bump
    )]
    pub facility: Account<'info, Facility>,

    #[account(
        mut,
        seeds = [b"repayment", facility.key().as_ref()],
        bump = repayment_schedule.bump
    )]
    pub repayment_schedule: Account<'info, RepaymentSchedule>,

    #[account(
        init,
        payer = operator,
        seeds = [b"event", facility.key().as_ref(), event_index.to_le_bytes().as_ref()],
        bump,
        space = 8 + EventLog::INIT_SPACE
    )]
    pub event_log: Account<'info, EventLog>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(_ds_bps: u64, _ltv_bps: u64, _liq_bps: u64, _current: bool, _breached: bool, _reason: String, event_index: u64)]
pub struct UpdateCovenantState<'info> {
    #[account(mut)]
    pub operator: Signer<'info>,
    #[account(
        mut,
        seeds = [b"facility", facility.facility_id.as_bytes()],
        bump = facility.bump
    )]
    pub facility: Account<'info, Facility>,

    #[account(
        mut,
        seeds = [b"covenant", facility.key().as_ref()],
        bump = covenant_state.bump
    )]
    pub covenant_state: Account<'info, CovenantState>,

    #[account(
        init,
        payer = operator,
        seeds = [b"event", facility.key().as_ref(), event_index.to_le_bytes().as_ref()],
        bump,
        space = 8 + EventLog::INIT_SPACE
    )]
    pub event_log: Account<'info, EventLog>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(_enabled: bool, _pik_amount: u64, event_index: u64)]
pub struct TogglePik<'info> {
    #[account(mut)]
    pub operator: Signer<'info>,
    #[account(
        mut,
        seeds = [b"facility", facility.facility_id.as_bytes()],
        bump = facility.bump
    )]
    pub facility: Account<'info, Facility>,

    #[account(
        init,
        payer = operator,
        seeds = [b"event", facility.key().as_ref(), event_index.to_le_bytes().as_ref()],
        bump,
        space = 8 + EventLog::INIT_SPACE
    )]
    pub event_log: Account<'info, EventLog>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(event_index: u64)]
pub struct CapitalizePik<'info> {
    #[account(mut)]
    pub operator: Signer<'info>,
    #[account(
        mut,
        seeds = [b"facility", facility.facility_id.as_bytes()],
        bump = facility.bump
    )]
    pub facility: Account<'info, Facility>,

    #[account(
        init,
        payer = operator,
        seeds = [b"event", facility.key().as_ref(), event_index.to_le_bytes().as_ref()],
        bump,
        space = 8 + EventLog::INIT_SPACE
    )]
    pub event_log: Account<'info, EventLog>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(_val: u64, _status: crate::state::CollateralStatus, _impaired: bool, event_index: u64)]
pub struct UpdateCollateralStatus<'info> {
    #[account(mut)]
    pub operator: Signer<'info>,
    #[account(
        mut,
        seeds = [b"facility", facility.facility_id.as_bytes()],
        bump = facility.bump
    )]
    pub facility: Account<'info, Facility>,

    #[account(
        mut,
        seeds = [b"covenant", facility.key().as_ref()],
        bump = covenant_state.bump
    )]
    pub covenant_state: Account<'info, CovenantState>,

    #[account(
        init,
        payer = operator,
        seeds = [b"event", facility.key().as_ref(), event_index.to_le_bytes().as_ref()],
        bump,
        space = 8 + EventLog::INIT_SPACE
    )]
    pub event_log: Account<'info, EventLog>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(_amount: u64, event_index: u64)]
pub struct RequestRedemption<'info> {
    #[account(mut)]
    pub investor: Signer<'info>,
    #[account(
        mut,
        seeds = [b"facility", facility.facility_id.as_bytes()],
        bump = facility.bump
    )]
    pub facility: Account<'info, Facility>,

    /// CHECK: PDA constrained; manually deserialized, owner checked and saved.
    #[account(
        mut,
        seeds = [b"investor", facility.key().as_ref(), investor.key().as_ref()],
        bump
    )]
    pub investor_position: UncheckedAccount<'info>,

    /// CHECK: PDA constrained; manually deserialized, owner checked and saved.
    #[account(
        mut,
        seeds = [b"redemption", facility.key().as_ref(), investor.key().as_ref()],
        bump
    )]
    pub redemption_request: UncheckedAccount<'info>,

    #[account(
        init,
        payer = investor,
        seeds = [b"event", facility.key().as_ref(), event_index.to_le_bytes().as_ref()],
        bump,
        space = 8 + EventLog::INIT_SPACE
    )]
    pub event_log: Account<'info, EventLog>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(event_index: u64)]
pub struct CancelRedemption<'info> {
    #[account(mut)]
    pub investor: Signer<'info>,
    #[account(
        mut,
        seeds = [b"facility", facility.facility_id.as_bytes()],
        bump = facility.bump
    )]
    pub facility: Account<'info, Facility>,

    /// CHECK: PDA constrained; manually deserialized, owner checked and saved.
    #[account(
        mut,
        seeds = [b"investor", facility.key().as_ref(), investor.key().as_ref()],
        bump
    )]
    pub investor_position: UncheckedAccount<'info>,

    /// CHECK: PDA constrained; manually deserialized, owner checked and saved.
    #[account(
        mut,
        seeds = [b"redemption", facility.key().as_ref(), investor.key().as_ref()],
        bump
    )]
    pub redemption_request: UncheckedAccount<'info>,

    #[account(
        init,
        payer = investor,
        seeds = [b"event", facility.key().as_ref(), event_index.to_le_bytes().as_ref()],
        bump,
        space = 8 + EventLog::INIT_SPACE
    )]
    pub event_log: Account<'info, EventLog>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(_amount: u64, event_index: u64)]
pub struct ProcessRedemption<'info> {
    #[account(mut)]
    pub operator: Signer<'info>,
    /// CHECK: Investor identity used for PDA derivation.
    pub investor: UncheckedAccount<'info>,
    #[account(
        mut,
        seeds = [b"facility", facility.facility_id.as_bytes()],
        bump = facility.bump
    )]
    pub facility: Account<'info, Facility>,

    /// CHECK: PDA constrained; manually deserialized, owner checked and saved.
    #[account(
        mut,
        seeds = [b"investor", facility.key().as_ref(), investor.key().as_ref()],
        bump
    )]
    pub investor_position: UncheckedAccount<'info>,

    /// CHECK: PDA constrained; manually deserialized, owner checked and saved.
    #[account(
        mut,
        seeds = [b"redemption", facility.key().as_ref(), investor.key().as_ref()],
        bump
    )]
    pub redemption_request: UncheckedAccount<'info>,

    #[account(
        init,
        payer = operator,
        seeds = [b"event", facility.key().as_ref(), event_index.to_le_bytes().as_ref()],
        bump,
        space = 8 + EventLog::INIT_SPACE
    )]
    pub event_log: Account<'info, EventLog>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(event_index: u64)]
pub struct FreezeInvestorPosition<'info> {
    #[account(mut)]
    pub operator: Signer<'info>,
    /// CHECK: Investor identity used for PDA derivation.
    pub investor: UncheckedAccount<'info>,
    #[account(
        mut,
        seeds = [b"facility", facility.facility_id.as_bytes()],
        bump = facility.bump
    )]
    pub facility: Account<'info, Facility>,

    /// CHECK: PDA constrained; manually deserialized, owner checked and saved.
    #[account(
        mut,
        seeds = [b"investor", facility.key().as_ref(), investor.key().as_ref()],
        bump
    )]
    pub investor_position: UncheckedAccount<'info>,

    /// CHECK: PDA constrained; manually deserialized, owner checked and saved.
    #[account(
        mut,
        seeds = [b"redemption", facility.key().as_ref(), investor.key().as_ref()],
        bump
    )]
    pub redemption_request: UncheckedAccount<'info>,

    #[account(
        init,
        payer = operator,
        seeds = [b"event", facility.key().as_ref(), event_index.to_le_bytes().as_ref()],
        bump,
        space = 8 + EventLog::INIT_SPACE
    )]
    pub event_log: Account<'info, EventLog>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(event_index: u64)]
pub struct MarkMatured<'info> {
    #[account(mut)]
    pub operator: Signer<'info>,
    #[account(
        mut,
        seeds = [b"facility", facility.facility_id.as_bytes()],
        bump = facility.bump
    )]
    pub facility: Account<'info, Facility>,

    #[account(
        mut,
        seeds = [b"repayment", facility.key().as_ref()],
        bump = repayment_schedule.bump
    )]
    pub repayment_schedule: Account<'info, RepaymentSchedule>,

    #[account(
        init,
        payer = operator,
        seeds = [b"event", facility.key().as_ref(), event_index.to_le_bytes().as_ref()],
        bump,
        space = 8 + EventLog::INIT_SPACE
    )]
    pub event_log: Account<'info, EventLog>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(_reason: u16, event_index: u64)]
pub struct RaiseFacilityDefault<'info> {
    #[account(mut)]
    pub operator: Signer<'info>,
    #[account(
        mut,
        seeds = [b"facility", facility.facility_id.as_bytes()],
        bump = facility.bump
    )]
    pub facility: Account<'info, Facility>,

    #[account(
        mut,
        seeds = [b"repayment", facility.key().as_ref()],
        bump = repayment_schedule.bump
    )]
    pub repayment_schedule: Account<'info, RepaymentSchedule>,

    #[account(
        init,
        payer = operator,
        seeds = [b"event", facility.key().as_ref(), event_index.to_le_bytes().as_ref()],
        bump,
        space = 8 + EventLog::INIT_SPACE
    )]
    pub event_log: Account<'info, EventLog>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(
    _hash: [u8; 32],
    _mat: Option<i64>,
    _draw: Option<u64>,
    _borr: Option<Pubkey>,
    _cov: Option<u64>,
    _res: Option<u64>,
    event_index: u64
)]
pub struct UpdateFacilityMetadata<'info> {
    #[account(mut)]
    pub operator: Signer<'info>,
    #[account(
        mut,
        seeds = [b"facility", facility.facility_id.as_bytes()],
        bump = facility.bump
    )]
    pub facility: Account<'info, Facility>,

    #[account(
        init,
        payer = operator,
        seeds = [b"event", facility.key().as_ref(), event_index.to_le_bytes().as_ref()],
        bump,
        space = 8 + EventLog::INIT_SPACE
    )]
    pub event_log: Account<'info, EventLog>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(_resolved: bool, event_index: u64)]
pub struct CloseFacility<'info> {
    #[account(mut)]
    pub operator: Signer<'info>,
    #[account(
        mut,
        seeds = [b"facility", facility.facility_id.as_bytes()],
        bump = facility.bump
    )]
    pub facility: Account<'info, Facility>,

    #[account(
        init,
        payer = operator,
        seeds = [b"event", facility.key().as_ref(), event_index.to_le_bytes().as_ref()],
        bump,
        space = 8 + EventLog::INIT_SPACE
    )]
    pub event_log: Account<'info, EventLog>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(event_index: u64)]
pub struct AppendEventLog<'info> {
    #[account(mut)]
    pub operator: Signer<'info>,
    #[account(
        mut,
        seeds = [b"facility", facility.facility_id.as_bytes()],
        bump = facility.bump
    )]
    pub facility: Account<'info, Facility>,

    #[account(
        init,
        payer = operator,
        seeds = [b"event", facility.key().as_ref(), event_index.to_le_bytes().as_ref()],
        bump,
        space = 8 + EventLog::INIT_SPACE
    )]
    pub event_log: Account<'info, EventLog>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct InitializeExtension<'info> {
    #[account(mut)]
    pub operator: Signer<'info>,
    #[account(
        seeds = [b"facility", facility.facility_id.as_bytes()],
        bump = facility.bump
    )]
    pub facility: Account<'info, Facility>,
    #[account(
        init,
        payer = operator,
        seeds = [b"extension", facility.key().as_ref()],
        bump,
        space = 8 + VaultExtension::INIT_SPACE
    )]
    pub extension: Account<'info, VaultExtension>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct InitializeEligibility<'info> {
    #[account(mut)]
    pub operator: Signer<'info>,
    /// CHECK: Target investor key.
    pub investor: UncheckedAccount<'info>,
    #[account(
        seeds = [b"facility", facility.facility_id.as_bytes()],
        bump = facility.bump
    )]
    pub facility: Account<'info, Facility>,
    #[account(
        init,
        payer = operator,
        seeds = [b"eligibility", facility.key().as_ref(), investor.key().as_ref()],
        bump,
        space = 8 + InvestorEligibility::INIT_SPACE
    )]
    pub eligibility: Account<'info, InvestorEligibility>,
    pub system_program: Program<'info, System>,
}
