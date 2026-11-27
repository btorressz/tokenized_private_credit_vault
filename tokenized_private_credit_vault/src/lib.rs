use anchor_lang::prelude::*;

// MVP only: simulated stablecoin accounting; no real asset transfers, custody,
// verified KYC, enforceable liens, production lending, or audited security.
// GitHub export had an empty ID. Replace this public placeholder with the
// actual Playground program public key before deployment.
declare_id!("9HGqntBBGRjhw3FtZh9wBMyPT9hk51rkhbx4cQ8vLCDY");

pub mod account;
pub mod state;
pub mod accounting;
pub mod facility;
pub mod redemption;
pub mod risk;
pub mod valuation;
pub mod waterfall;
pub mod recovery;
pub mod covenant;
pub mod collateral;
pub mod pda;
pub mod math;
pub mod events;
pub mod validation;
pub mod errors;
pub mod constants;
pub mod compliance;
pub mod token_vault;

pub use account::*;
pub use state::*;
use constants::*;
use errors::PrivateCreditError;
use events::write_event_log;
use math::{checked_add, checked_sub};
use pda::{load_or_init_investor_position, save_investor_position};
use validation::*;

#[program]
pub mod tokenized_private_credit_vault {
    use super::*;

    pub fn initialize_facility(
        ctx: Context<InitializeFacility>,
        facility_id: String,
        event_index: u64,
        stablecoin_mint: Pubkey,
        facility_size: u64,
        interest_rate_bps: u16,
        maturity_timestamp: i64,
        max_drawdown_amount: u64,
        collateral_description_hash: [u8; 32],
        minimum_collateral_coverage_bps: u64,
        minimum_liquidity_reserve: u64,
    ) -> Result<()> {
        let facility_key = ctx.accounts.facility.key();
        facility::process_initialize_facility(
            &mut ctx.accounts.facility,
            &mut ctx.accounts.repayment_schedule,
            &mut ctx.accounts.covenant_state,
            &mut ctx.accounts.event_log,
            ctx.accounts.operator.key(),
            ctx.accounts.borrower.key(),
            facility_key,
            facility_id,
            event_index,
            stablecoin_mint,
            facility_size,
            interest_rate_bps,
            maturity_timestamp,
            max_drawdown_amount,
            collateral_description_hash,
            minimum_collateral_coverage_bps,
            minimum_liquidity_reserve,
            ctx.bumps.facility,
            ctx.bumps.repayment_schedule,
            ctx.bumps.covenant_state,
            ctx.bumps.event_log,
            Clock::get()?.unix_timestamp,
        )
    }

    pub fn activate_facility(
        ctx: Context<ActivateFacility>,
        event_index: u64,
    ) -> Result<()> {
        facility::process_activate_facility(
            &mut ctx.accounts.facility,
            &mut ctx.accounts.event_log,
            ctx.accounts.operator.key(),
            event_index,
            ctx.bumps.event_log,
            Clock::get()?.unix_timestamp,
        )
    }

    pub fn deposit_capital(
        ctx: Context<DepositCapital>,
        amount: u64,
        event_index: u64,
    ) -> Result<()> {
        let now = Clock::get()?.unix_timestamp;
        require!(amount > 0, PrivateCreditError::AmountMustBeGreaterThanZero);
        require_deposits_allowed(&ctx.accounts.facility)?;
        let facility_key = ctx.accounts.facility.key();
        let investor_key = ctx.accounts.investor.key();
        let mut position = load_or_init_investor_position(
            &ctx.accounts.investor_position.to_account_info(),
            &ctx.accounts.investor.to_account_info(),
            &ctx.accounts.system_program.to_account_info(),
            &facility_key, &investor_key, ctx.bumps.investor_position, now,
        )?;
        require!(position.status != InvestorPositionStatus::Frozen, PrivateCreditError::InvestorPositionFrozen);
        position.principal = checked_add(position.principal, amount)?;
        position.share_balance = checked_add(position.share_balance, amount)?;
        position.deposited_amount = checked_add(position.deposited_amount, amount)?;
        position.status = if position.pending_redemption_amount > 0 { InvestorPositionStatus::RedemptionPending } else { InvestorPositionStatus::Active };
        position.updated_at = now;
        save_investor_position(&ctx.accounts.investor_position.to_account_info(), &position)?;
        let facility = &mut ctx.accounts.facility;
        let new_total_deposited = checked_add(facility.total_deposited, amount)?;
        require!(new_total_deposited <= facility.facility_size, PrivateCreditError::ExceedsFacilitySize);
        facility.total_deposited = new_total_deposited;
        facility.available_liquidity = token_vault::cash_after(facility.available_liquidity, amount, token_vault::SettlementKind::CapitalDeposit, 0)?;
        // Simulated deposit; production requires SPL Token CPI and eligibility checks.
        write_event_log(
            &mut ctx.accounts.event_log,
            facility,
            event_index,
            EventType::CapitalDeposited,
            investor_key,
            amount,
            0,
            ctx.bumps.event_log,
            now,
        )?;
        Ok(())
    }

    pub fn borrower_drawdown(
        ctx: Context<BorrowerDrawdown>,
        amount: u64,
        event_index: u64,
    ) -> Result<()> {
        let now = Clock::get()?.unix_timestamp;
        let actor = ctx.accounts.actor.key();
        require_borrower_or_operator(&ctx.accounts.facility, actor)?;
        require!(amount > 0, PrivateCreditError::AmountMustBeGreaterThanZero);
        require_drawdowns_allowed(&ctx.accounts.facility)?;
        require!(now < ctx.accounts.facility.maturity_timestamp, PrivateCreditError::InvalidMaturity);
        let facility = &mut ctx.accounts.facility;
        require!(amount <= facility.available_liquidity, PrivateCreditError::ExceedsAvailableLiquidity);
        let new_total_drawn = checked_add(facility.total_drawn, amount)?;
        require!(new_total_drawn <= facility.max_drawdown_amount, PrivateCreditError::ExceedsFacilitySize);
        require!(new_total_drawn <= facility.facility_size, PrivateCreditError::ExceedsFacilitySize);
        accounting::accrue_to(facility, now)?;
        facility.total_drawn = new_total_drawn;
        facility.outstanding_principal = checked_add(facility.outstanding_principal, amount)?;
        facility.available_liquidity = token_vault::cash_after(facility.available_liquidity, amount, token_vault::SettlementKind::BorrowerDrawdown, facility.minimum_liquidity_reserve)?;
        facility.status = FacilityStatus::Drawn;
        facility.payment_status = PaymentStatus::Due;
        let schedule = &mut ctx.accounts.repayment_schedule;
        schedule.status = PaymentStatus::Due;
        schedule.next_due_timestamp = now.checked_add(THIRTY_DAYS_SECONDS).ok_or(error!(PrivateCreditError::MathOverflow))?;
        schedule.updated_at = now;
        // Simulated drawdown; no stablecoins transferred.
        write_event_log(
            &mut ctx.accounts.event_log,
            facility,
            event_index,
            EventType::BorrowerDrawdown,
            actor,
            amount,
            0,
            ctx.bumps.event_log,
            now,
        )?;
        Ok(())
    }

    pub fn accrue_interest(
        ctx: Context<AccrueInterest>,
        event_index: u64,
    ) -> Result<()> {
        accounting::process_accrue_interest(
            &mut ctx.accounts.facility,
            &mut ctx.accounts.event_log,
            ctx.accounts.operator.key(),
            event_index,
            ctx.bumps.event_log,
            Clock::get()?.unix_timestamp,
        )
    }

    pub fn make_repayment(
        ctx: Context<MakeRepayment>,
        amount: u64,
        principal_portion: u64,
        interest_portion: u64,
        scheduled_payment_satisfied: bool,
        event_index: u64,
    ) -> Result<()> {
        accounting::process_make_repayment(
            &mut ctx.accounts.facility,
            &mut ctx.accounts.repayment_schedule,
            &mut ctx.accounts.event_log,
            ctx.accounts.borrower.key(),
            amount,
            principal_portion,
            interest_portion,
            scheduled_payment_satisfied,
            event_index,
            ctx.bumps.event_log,
            Clock::get()?.unix_timestamp,
        )
    }

    pub fn record_missed_payment(
        ctx: Context<RecordMissedPayment>,
        event_index: u64,
    ) -> Result<()> {
        accounting::process_record_missed_payment(
            &mut ctx.accounts.facility,
            &mut ctx.accounts.repayment_schedule,
            &mut ctx.accounts.event_log,
            ctx.accounts.operator.key(),
            event_index,
            ctx.bumps.event_log,
            Clock::get()?.unix_timestamp,
        )
    }

    pub fn update_covenant_state(
        ctx: Context<UpdateCovenantState>,
        debt_service_coverage_ratio_bps: u64,
        loan_to_value_bps: u64,
        minimum_liquidity_bps: u64,
        reporting_current: bool,
        breached: bool,
        breach_reason: String,
        event_index: u64,
    ) -> Result<()> {
        covenant::process_update_covenant_state(
            &mut ctx.accounts.facility,
            &mut ctx.accounts.covenant_state,
            &mut ctx.accounts.event_log,
            ctx.accounts.operator.key(),
            debt_service_coverage_ratio_bps,
            loan_to_value_bps,
            minimum_liquidity_bps,
            reporting_current,
            breached,
            breach_reason,
            event_index,
            ctx.bumps.event_log,
            Clock::get()?.unix_timestamp,
        )
    }

    pub fn toggle_pik(
        ctx: Context<TogglePik>,
        enabled: bool,
        pik_accrual_amount: u64,
        event_index: u64,
    ) -> Result<()> {
        accounting::process_toggle_pik(
            &mut ctx.accounts.facility,
            &mut ctx.accounts.event_log,
            ctx.accounts.operator.key(),
            enabled,
            pik_accrual_amount,
            event_index,
            ctx.bumps.event_log,
            Clock::get()?.unix_timestamp,
        )
    }

    pub fn capitalize_pik(
        ctx: Context<CapitalizePik>,
        event_index: u64,
    ) -> Result<()> {
        accounting::process_capitalize_pik(
            &mut ctx.accounts.facility,
            &mut ctx.accounts.event_log,
            ctx.accounts.operator.key(),
            event_index,
            ctx.bumps.event_log,
            Clock::get()?.unix_timestamp,
        )
    }

    pub fn update_collateral_status(
        ctx: Context<UpdateCollateralStatus>,
        collateral_value: u64,
        collateral_status: CollateralStatus,
        collateral_impaired: bool,
        event_index: u64,
    ) -> Result<()> {
        collateral::process_update_collateral_status(
            &mut ctx.accounts.facility,
            &mut ctx.accounts.covenant_state,
            &mut ctx.accounts.event_log,
            ctx.accounts.operator.key(),
            collateral_value,
            collateral_status,
            collateral_impaired,
            event_index,
            ctx.bumps.event_log,
            Clock::get()?.unix_timestamp,
        )
    }

    pub fn request_redemption(
        ctx: Context<RequestRedemption>,
        amount: u64,
        event_index: u64,
    ) -> Result<()> {
        redemption::process_request_redemption(
            &mut ctx.accounts.facility,
            &ctx.accounts.investor_position.to_account_info(),
            &ctx.accounts.redemption_request.to_account_info(),
            &ctx.accounts.investor.to_account_info(),
            &ctx.accounts.system_program.to_account_info(),
            &mut ctx.accounts.event_log,
            amount,
            event_index,
            ctx.bumps.redemption_request,
            ctx.bumps.event_log,
            Clock::get()?.unix_timestamp,
        )
    }

    pub fn cancel_redemption(
        ctx: Context<CancelRedemption>,
        event_index: u64,
    ) -> Result<()> {
        redemption::process_cancel_redemption(
            &mut ctx.accounts.facility,
            &ctx.accounts.investor_position.to_account_info(),
            &ctx.accounts.redemption_request.to_account_info(),
            ctx.accounts.investor.key(),
            &mut ctx.accounts.event_log,
            event_index,
            ctx.bumps.event_log,
            Clock::get()?.unix_timestamp,
        )
    }

    pub fn process_redemption(
        ctx: Context<ProcessRedemption>,
        amount: u64,
        event_index: u64,
    ) -> Result<()> {
        redemption::process_process_redemption(
            &mut ctx.accounts.facility,
            &ctx.accounts.investor_position.to_account_info(),
            &ctx.accounts.redemption_request.to_account_info(),
            ctx.accounts.operator.key(),
            ctx.accounts.investor.key(),
            &mut ctx.accounts.event_log,
            amount,
            event_index,
            ctx.bumps.event_log,
            Clock::get()?.unix_timestamp,
        )
    }

    pub fn freeze_investor_position(
        ctx: Context<FreezeInvestorPosition>,
        event_index: u64,
    ) -> Result<()> {
        redemption::process_freeze_investor_position(
            &mut ctx.accounts.facility,
            &ctx.accounts.investor_position.to_account_info(),
            &ctx.accounts.redemption_request.to_account_info(),
            ctx.accounts.operator.key(),
            ctx.accounts.investor.key(),
            &mut ctx.accounts.event_log,
            event_index,
            ctx.bumps.event_log,
            Clock::get()?.unix_timestamp,
        )
    }

    pub fn mark_matured(
        ctx: Context<MarkMatured>,
        event_index: u64,
    ) -> Result<()> {
        facility::process_mark_matured(
            &mut ctx.accounts.facility,
            &mut ctx.accounts.repayment_schedule,
            &mut ctx.accounts.event_log,
            ctx.accounts.operator.key(),
            event_index,
            ctx.bumps.event_log,
            Clock::get()?.unix_timestamp,
        )
    }

    pub fn raise_facility_default(
        ctx: Context<RaiseFacilityDefault>,
        default_reason_code: u16,
        event_index: u64,
    ) -> Result<()> {
        facility::process_raise_facility_default(
            &mut ctx.accounts.facility,
            &mut ctx.accounts.repayment_schedule,
            &mut ctx.accounts.event_log,
            ctx.accounts.operator.key(),
            default_reason_code,
            event_index,
            ctx.bumps.event_log,
            Clock::get()?.unix_timestamp,
        )
    }

    pub fn update_facility_metadata(
        ctx: Context<UpdateFacilityMetadata>,
        new_collateral_description_hash: [u8; 32],
        new_maturity_timestamp: Option<i64>,
        new_max_drawdown_amount: Option<u64>,
        new_borrower: Option<Pubkey>,
        new_minimum_collateral_coverage_bps: Option<u64>,
        new_minimum_liquidity_reserve: Option<u64>,
        event_index: u64,
    ) -> Result<()> {
        facility::process_update_facility_metadata(
            &mut ctx.accounts.facility,
            &mut ctx.accounts.event_log,
            ctx.accounts.operator.key(),
            new_collateral_description_hash,
            new_maturity_timestamp,
            new_max_drawdown_amount,
            new_borrower,
            new_minimum_collateral_coverage_bps,
            new_minimum_liquidity_reserve,
            event_index,
            ctx.bumps.event_log,
            Clock::get()?.unix_timestamp,
        )
    }

    pub fn close_facility(
        ctx: Context<CloseFacility>,
        fully_resolved: bool,
        event_index: u64,
    ) -> Result<()> {
        facility::process_close_facility(
            &mut ctx.accounts.facility,
            &mut ctx.accounts.event_log,
            ctx.accounts.operator.key(),
            fully_resolved,
            event_index,
            ctx.bumps.event_log,
            Clock::get()?.unix_timestamp,
        )
    }

    pub fn append_event_log(
        ctx: Context<AppendEventLog>,
        event_index: u64,
        event_type: EventType,
        amount: u64,
        note_code: u16,
    ) -> Result<()> {
        let now = Clock::get()?.unix_timestamp;
        require_operator(&ctx.accounts.facility, ctx.accounts.operator.key())?;
        require!(event_type == EventType::AdministrativeNote, PrivateCreditError::InvalidEventTypeForAdminLog);
        let facility = &mut ctx.accounts.facility;
        write_event_log(
            &mut ctx.accounts.event_log,
            facility,
            event_index,
            event_type,
            ctx.accounts.operator.key(),
            amount,
            note_code,
            ctx.bumps.event_log,
            now,
        )?;
        Ok(())
    }
}
