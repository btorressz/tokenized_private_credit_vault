use anchor_lang::prelude::*;
use anchor_lang::solana_program::{program::invoke_signed, system_instruction};
use std::io::Cursor;

// Replace this example address with your existing Solana Playground program ID.
declare_id!("");

// -----------------------------------------------------------------------------
// IMPORTANT DISCLAIMER
// -----------------------------------------------------------------------------
// This is not financial advice, not a securities product, not a real lending
// protocol, and not production-ready.
// This proof-of-concept models private credit facility workflows, covenant
// monitoring, repayment tracking, PIK accounting, collateral state and
// redemption queue mechanics on Solana.
// Stablecoin accounting is simulated; no SPL token transfers occur.
// A real implementation requires securities/lending law review, KYC/AML,
// audited SPL token transfers, oracle integrations, off-chain reporting,
// custody controls, governance, risk management and security reviews.
// -----------------------------------------------------------------------------

const MAX_FACILITY_ID_LEN: usize = 32;
const MAX_COVENANT_REASON_LEN: usize = 96;
const MISSED_PAYMENT_DEFAULT_THRESHOLD: u16 = 2;
const ACCOUNT_DISCRIMINATOR_SIZE: usize = 8;
const THIRTY_DAYS_SECONDS: i64 = 2_592_000;
const YEAR_SECONDS: i64 = 31_536_000;
const BPS_DENOMINATOR: u128 = 10_000;

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
        let now = Clock::get()?.unix_timestamp;
        require!(facility_id.as_bytes().len() <= MAX_FACILITY_ID_LEN, PrivateCreditError::StringTooLong);
        require!(event_index == 0, PrivateCreditError::InvalidEventIndex);
        require!(facility_size > 0 && max_drawdown_amount > 0, PrivateCreditError::AmountMustBeGreaterThanZero);
        require!(max_drawdown_amount <= facility_size, PrivateCreditError::ExceedsFacilitySize);
        require!(minimum_liquidity_reserve <= facility_size, PrivateCreditError::ExceedsFacilitySize);
        require!(maturity_timestamp > now, PrivateCreditError::InvalidMaturity);
        let facility_key = ctx.accounts.facility.key();
        let facility = &mut ctx.accounts.facility;
        facility.facility_id = facility_id;
        facility.operator = ctx.accounts.operator.key();
        facility.borrower = ctx.accounts.borrower.key();
        facility.stablecoin_mint = stablecoin_mint;
        facility.facility_size = facility_size;
        facility.interest_rate_bps = interest_rate_bps;
        facility.maturity_timestamp = maturity_timestamp;
        facility.max_drawdown_amount = max_drawdown_amount;
        facility.minimum_collateral_coverage_bps = minimum_collateral_coverage_bps;
        facility.minimum_liquidity_reserve = minimum_liquidity_reserve;
        facility.collateral_description_hash = collateral_description_hash;
        facility.status = FacilityStatus::Initialized;
        facility.payment_status = PaymentStatus::Current;
        facility.collateral_status = CollateralStatus::Unknown;
        facility.total_deposited = 0;
        facility.total_drawn = 0;
        facility.total_repaid = 0;
        facility.available_liquidity = 0;
        facility.outstanding_principal = 0;
        facility.accrued_interest = 0;
        facility.last_interest_accrual_timestamp = now;
        facility.missed_payment_count = 0;
        facility.total_pending_redemptions = 0;
        facility.redemption_queue_index = 0;
        facility.pik_enabled = false;
        facility.total_pik_accrued = 0;
        facility.last_pik_timestamp = 0;
        facility.default_reason_code = 0;
        facility.event_index = 0;
        facility.created_at = now;
        facility.updated_at = now;
        facility.bump = ctx.bumps.facility;

        let schedule = &mut ctx.accounts.repayment_schedule;
        schedule.facility = facility_key;
        schedule.expected_total_repayment = 0;
        schedule.next_due_timestamp = maturity_timestamp;
        schedule.scheduled_payment_amount = 0;
        schedule.last_payment_timestamp = 0;
        schedule.last_repayment_amount = 0;
        schedule.last_principal_portion = 0;
        schedule.last_interest_portion = 0;
        schedule.payments_made = 0;
        schedule.scheduled_payments_satisfied = 0;
        schedule.status = PaymentStatus::Current;
        schedule.created_at = now;
        schedule.updated_at = now;
        schedule.bump = ctx.bumps.repayment_schedule;

        let covenant = &mut ctx.accounts.covenant_state;
        covenant.facility = facility_key;
        covenant.debt_service_coverage_ratio_bps = 0;
        covenant.loan_to_value_bps = 0;
        covenant.minimum_liquidity_bps = 0;
        covenant.reporting_current = false;
        covenant.last_report_timestamp = 0;
        covenant.breached = false;
        covenant.breach_reason = String::new();
        covenant.collateral_value = 0;
        covenant.collateral_status = CollateralStatus::Unknown;
        covenant.last_valuation_timestamp = 0;
        covenant.collateral_impaired = false;
        covenant.updated_by = ctx.accounts.operator.key();
        covenant.created_at = now;
        covenant.updated_at = now;
        covenant.bump = ctx.bumps.covenant_state;
        write_event_log(
            &mut ctx.accounts.event_log,
            facility,
            event_index,
            EventType::FacilityInitialized,
            ctx.accounts.operator.key(),
            0,
            0,
            ctx.bumps.event_log,
            now,
        )?;
        Ok(())
    }

    pub fn activate_facility(
        ctx: Context<ActivateFacility>,
        event_index: u64,
    ) -> Result<()> {
        let now = Clock::get()?.unix_timestamp;
        require_operator(&ctx.accounts.facility, ctx.accounts.operator.key())?;
        require!(ctx.accounts.facility.status == FacilityStatus::Initialized, PrivateCreditError::InvalidFacilityStatus);
        require!(now < ctx.accounts.facility.maturity_timestamp, PrivateCreditError::InvalidMaturity);
        let facility = &mut ctx.accounts.facility;
        facility.status = FacilityStatus::Active;
        write_event_log(
            &mut ctx.accounts.event_log,
            facility,
            event_index,
            EventType::FacilityActivated,
            ctx.accounts.operator.key(),
            0,
            0,
            ctx.bumps.event_log,
            now,
        )?;
        Ok(())
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
        position.status = InvestorPositionStatus::Active;
        position.updated_at = now;
        save_investor_position(&ctx.accounts.investor_position.to_account_info(), &position)?;
        let facility = &mut ctx.accounts.facility;
        let new_total_deposited = checked_add(facility.total_deposited, amount)?;
        require!(new_total_deposited <= facility.facility_size, PrivateCreditError::ExceedsFacilitySize);
        facility.total_deposited = new_total_deposited;
        facility.available_liquidity = checked_add(facility.available_liquidity, amount)?;
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
        facility.total_drawn = new_total_drawn;
        facility.outstanding_principal = checked_add(facility.outstanding_principal, amount)?;
        facility.available_liquidity = checked_sub(facility.available_liquidity, amount)?;
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
        let now = Clock::get()?.unix_timestamp;
        require_operator(&ctx.accounts.facility, ctx.accounts.operator.key())?;
        require_not_defaulted_or_closed(&ctx.accounts.facility)?;
        let facility = &mut ctx.accounts.facility;
        let elapsed_seconds = now.checked_sub(facility.last_interest_accrual_timestamp).ok_or(error!(PrivateCreditError::MathOverflow))?;
        let interest_amount = calculate_interest_amount(facility.outstanding_principal, facility.interest_rate_bps, elapsed_seconds)?;
        facility.accrued_interest = checked_add(facility.accrued_interest, interest_amount)?;
        facility.last_interest_accrual_timestamp = now;
        // Simplified interest accounting; production requires formal day-count and waterfall rules.
        write_event_log(
            &mut ctx.accounts.event_log,
            facility,
            event_index,
            EventType::InterestAccrued,
            ctx.accounts.operator.key(),
            interest_amount,
            0,
            ctx.bumps.event_log,
            now,
        )?;
        Ok(())
    }

    pub fn make_repayment(
        ctx: Context<MakeRepayment>,
        amount: u64,
        principal_portion: u64,
        interest_portion: u64,
        scheduled_payment_satisfied: bool,
        event_index: u64,
    ) -> Result<()> {
        let now = Clock::get()?.unix_timestamp;
        require_keys_eq!(ctx.accounts.borrower.key(), ctx.accounts.facility.borrower, PrivateCreditError::Unauthorized);
        require!(amount > 0, PrivateCreditError::AmountMustBeGreaterThanZero);
        require!(checked_add(principal_portion, interest_portion)? == amount, PrivateCreditError::InvalidRepaymentAmount);
        require!(principal_portion <= ctx.accounts.facility.outstanding_principal, PrivateCreditError::InvalidRepaymentAmount);
        require!(ctx.accounts.facility.status != FacilityStatus::Closed, PrivateCreditError::FacilityClosed);
        let facility = &mut ctx.accounts.facility;
        let interest_to_clear = interest_portion.min(facility.accrued_interest);
        facility.total_repaid = checked_add(facility.total_repaid, amount)?;
        facility.accrued_interest = checked_sub(facility.accrued_interest, interest_to_clear)?;
        facility.outstanding_principal = checked_sub(facility.outstanding_principal, principal_portion)?;
        facility.available_liquidity = checked_add(facility.available_liquidity, amount)?;
        if facility.outstanding_principal == 0 && facility.accrued_interest == 0 {
            facility.status = FacilityStatus::Matured;
            facility.payment_status = PaymentStatus::Repaid;
        } else if facility.status != FacilityStatus::Breached && facility.status != FacilityStatus::Defaulted {
            facility.status = FacilityStatus::Performing;
            facility.payment_status = PaymentStatus::Current;
        }
        let schedule = &mut ctx.accounts.repayment_schedule;
        schedule.last_payment_timestamp = now;
        schedule.last_repayment_amount = amount;
        schedule.last_principal_portion = principal_portion;
        schedule.last_interest_portion = interest_portion;
        schedule.payments_made = schedule.payments_made.checked_add(1).ok_or(error!(PrivateCreditError::MathOverflow))?;
        if scheduled_payment_satisfied {
            schedule.scheduled_payments_satisfied = schedule.scheduled_payments_satisfied.checked_add(1).ok_or(error!(PrivateCreditError::MathOverflow))?;
            schedule.status = facility.payment_status;
        }
        schedule.updated_at = now;
        // Simulated repayment; no stablecoins transferred.
        write_event_log(
            &mut ctx.accounts.event_log,
            facility,
            event_index,
            EventType::RepaymentMade,
            ctx.accounts.borrower.key(),
            amount,
            0,
            ctx.bumps.event_log,
            now,
        )?;
        Ok(())
    }

    pub fn record_missed_payment(
        ctx: Context<RecordMissedPayment>,
        event_index: u64,
    ) -> Result<()> {
        let now = Clock::get()?.unix_timestamp;
        require_operator(&ctx.accounts.facility, ctx.accounts.operator.key())?;
        require_not_defaulted_or_closed(&ctx.accounts.facility)?;
        let facility = &mut ctx.accounts.facility;
        facility.missed_payment_count = facility.missed_payment_count.checked_add(1).ok_or(error!(PrivateCreditError::MathOverflow))?;
        facility.payment_status = PaymentStatus::Missed;
        let schedule = &mut ctx.accounts.repayment_schedule;
        schedule.status = PaymentStatus::Missed;
        schedule.updated_at = now;
        if facility.missed_payment_count >= MISSED_PAYMENT_DEFAULT_THRESHOLD {
            facility.status = FacilityStatus::Defaulted;
            facility.payment_status = PaymentStatus::Defaulted;
            schedule.status = PaymentStatus::Defaulted;
        }
        let missed_payment_count = facility.missed_payment_count;
        write_event_log(
            &mut ctx.accounts.event_log,
            facility,
            event_index,
            EventType::PaymentMissed,
            ctx.accounts.operator.key(),
            0,
            missed_payment_count,
            ctx.bumps.event_log,
            now,
        )?;
        Ok(())
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
        let now = Clock::get()?.unix_timestamp;
        require_operator(&ctx.accounts.facility, ctx.accounts.operator.key())?;
        require!(breach_reason.as_bytes().len() <= MAX_COVENANT_REASON_LEN, PrivateCreditError::StringTooLong);
        require!(ctx.accounts.facility.status != FacilityStatus::Closed, PrivateCreditError::FacilityClosed);
        let facility = &mut ctx.accounts.facility;
        let covenant = &mut ctx.accounts.covenant_state;
        covenant.debt_service_coverage_ratio_bps = debt_service_coverage_ratio_bps;
        covenant.loan_to_value_bps = loan_to_value_bps;
        covenant.minimum_liquidity_bps = minimum_liquidity_bps;
        covenant.reporting_current = reporting_current;
        covenant.last_report_timestamp = now;
        covenant.breached = breached;
        covenant.updated_by = ctx.accounts.operator.key();
        covenant.updated_at = now;
        let event_type = if breached {
            covenant.breach_reason = breach_reason;
            if facility.status != FacilityStatus::Defaulted {
                facility.status = FacilityStatus::Breached;
            }
            EventType::CovenantBreached
        } else {
            covenant.breach_reason = String::new();
            if facility.status == FacilityStatus::Breached {
                facility.status = if facility.outstanding_principal > 0 {
                    FacilityStatus::Performing
                } else {
                    FacilityStatus::Active
                };
            }
            EventType::CovenantUpdated
        };
        // Demo operator-supplied data; production requires verified financial reports.
        write_event_log(
            &mut ctx.accounts.event_log,
            facility,
            event_index,
            event_type,
            ctx.accounts.operator.key(),
            0,
            0,
            ctx.bumps.event_log,
            now,
        )?;
        Ok(())
    }

    pub fn toggle_pik(
        ctx: Context<TogglePik>,
        enabled: bool,
        pik_accrual_amount: u64,
        event_index: u64,
    ) -> Result<()> {
        let now = Clock::get()?.unix_timestamp;
        require_operator(&ctx.accounts.facility, ctx.accounts.operator.key())?;
        require_not_defaulted_or_closed(&ctx.accounts.facility)?;
        let facility = &mut ctx.accounts.facility;
        facility.pik_enabled = enabled;
        facility.total_pik_accrued = checked_add(facility.total_pik_accrued, pik_accrual_amount)?;
        facility.last_pik_timestamp = now;
        // PIK (payment-in-kind) is simulated accounting only.
        write_event_log(
            &mut ctx.accounts.event_log,
            facility,
            event_index,
            EventType::PikToggled,
            ctx.accounts.operator.key(),
            pik_accrual_amount,
            if enabled { 1 } else { 0 },
            ctx.bumps.event_log,
            now,
        )?;
        Ok(())
    }

    pub fn capitalize_pik(
        ctx: Context<CapitalizePik>,
        event_index: u64,
    ) -> Result<()> {
        let now = Clock::get()?.unix_timestamp;
        require_operator(&ctx.accounts.facility, ctx.accounts.operator.key())?;
        require_not_defaulted_or_closed(&ctx.accounts.facility)?;
        require!(ctx.accounts.facility.total_pik_accrued > 0, PrivateCreditError::AmountMustBeGreaterThanZero);
        let facility = &mut ctx.accounts.facility;
        let capitalized_amount = facility.total_pik_accrued;
        facility.outstanding_principal = checked_add(facility.outstanding_principal, capitalized_amount)?;
        facility.total_pik_accrued = 0;
        facility.last_pik_timestamp = now;
        if facility.status == FacilityStatus::Active || facility.status == FacilityStatus::Drawn {
            facility.status = FacilityStatus::Performing;
        }
        write_event_log(
            &mut ctx.accounts.event_log,
            facility,
            event_index,
            EventType::PikCapitalized,
            ctx.accounts.operator.key(),
            capitalized_amount,
            0,
            ctx.bumps.event_log,
            now,
        )?;
        Ok(())
    }

    pub fn update_collateral_status(
        ctx: Context<UpdateCollateralStatus>,
        collateral_value: u64,
        collateral_status: CollateralStatus,
        collateral_impaired: bool,
        event_index: u64,
    ) -> Result<()> {
        let now = Clock::get()?.unix_timestamp;
        require_operator(&ctx.accounts.facility, ctx.accounts.operator.key())?;
        require!(ctx.accounts.facility.status != FacilityStatus::Closed, PrivateCreditError::FacilityClosed);
        let facility = &mut ctx.accounts.facility;
        let covenant = &mut ctx.accounts.covenant_state;
        let coverage_bps = collateral_coverage_bps(collateral_value, facility.outstanding_principal)?;
        let below_minimum_coverage = facility.outstanding_principal > 0
            && facility.minimum_collateral_coverage_bps > 0
            && coverage_bps < facility.minimum_collateral_coverage_bps;
        covenant.collateral_value = collateral_value;
        covenant.collateral_status = collateral_status;
        covenant.last_valuation_timestamp = now;
        covenant.collateral_impaired = collateral_impaired;
        covenant.updated_by = ctx.accounts.operator.key();
        covenant.updated_at = now;
        facility.collateral_status = collateral_status;
        let mut note_code = 0;
        if collateral_impaired || below_minimum_coverage {
            covenant.breached = true;
            if below_minimum_coverage {
                covenant.breach_reason = "Collateral coverage below threshold".to_string();
                note_code = 2;
            } else {
                covenant.breach_reason = "Collateral impaired".to_string();
                note_code = 1;
            }
            if facility.status != FacilityStatus::Defaulted {
                facility.status = FacilityStatus::Breached;
            }
        }
        // Demo valuations; production requires verified third-party evidence.
        write_event_log(
            &mut ctx.accounts.event_log,
            facility,
            event_index,
            EventType::CollateralUpdated,
            ctx.accounts.operator.key(),
            collateral_value,
            note_code,
            ctx.bumps.event_log,
            now,
        )?;
        Ok(())
    }

    pub fn request_redemption(
        ctx: Context<RequestRedemption>,
        amount: u64,
        event_index: u64,
    ) -> Result<()> {
        let now = Clock::get()?.unix_timestamp;
        require!(amount > 0, PrivateCreditError::AmountMustBeGreaterThanZero);
        require_redemptions_allowed(&ctx.accounts.facility)?;
        let facility_key = ctx.accounts.facility.key();
        let investor_key = ctx.accounts.investor.key();
        let mut position = load_existing_investor_position(&ctx.accounts.investor_position.to_account_info())?;
        require_keys_eq!(position.facility, facility_key, PrivateCreditError::Unauthorized);
        require_keys_eq!(position.investor, investor_key, PrivateCreditError::Unauthorized);
        require!(position.status != InvestorPositionStatus::Frozen, PrivateCreditError::InvestorPositionFrozen);
        let redeemable_balance = checked_sub(position.share_balance, position.pending_redemption_amount)?;
        require!(amount <= redeemable_balance, PrivateCreditError::ExceedsInvestorBalance);
        let queue_index = ctx.accounts.facility.redemption_queue_index;
        let (mut request, created) = load_or_init_redemption_request(
            &ctx.accounts.redemption_request.to_account_info(),
            &ctx.accounts.investor.to_account_info(),
            &ctx.accounts.system_program.to_account_info(),
            &facility_key, &investor_key, ctx.bumps.redemption_request, queue_index, now,
        )?;
        require!(request.status != RedemptionStatus::Frozen, PrivateCreditError::RedemptionFrozen);
        let mut should_increment_queue = created;
        if !created && matches!(request.status, RedemptionStatus::Cancelled | RedemptionStatus::Processed) {
            let remaining = checked_sub(request.requested_amount, request.processed_amount)?;
            require!(remaining == 0, PrivateCreditError::RedemptionNotCancellable);
            request.requested_amount = 0;
            request.processed_amount = 0;
            request.timestamp = now;
            request.processed_timestamp = 0;
            request.queue_index = queue_index;
            should_increment_queue = true;
        }
        request.requested_amount = checked_add(request.requested_amount, amount)?;
        request.timestamp = now;
        request.status = RedemptionStatus::Queued;
        position.pending_redemption_amount = checked_add(position.pending_redemption_amount, amount)?;
        position.status = InvestorPositionStatus::RedemptionPending;
        position.updated_at = now;
        save_investor_position(&ctx.accounts.investor_position.to_account_info(), &position)?;
        save_redemption_request(&ctx.accounts.redemption_request.to_account_info(), &request)?;
        let facility = &mut ctx.accounts.facility;
        facility.total_pending_redemptions = checked_add(facility.total_pending_redemptions, amount)?;
        if should_increment_queue {
            facility.redemption_queue_index = checked_add(facility.redemption_queue_index, 1)?;
        }
        write_event_log(
            &mut ctx.accounts.event_log,
            facility,
            event_index,
            EventType::RedemptionRequested,
            investor_key,
            amount,
            request.queue_index as u16,
            ctx.bumps.event_log,
            now,
        )?;
        Ok(())
    }

    pub fn cancel_redemption(
        ctx: Context<CancelRedemption>,
        event_index: u64,
    ) -> Result<()> {
        let now = Clock::get()?.unix_timestamp;
        require_not_defaulted_or_closed(&ctx.accounts.facility)?;
        let facility_key = ctx.accounts.facility.key();
        let investor_key = ctx.accounts.investor.key();
        let mut position = load_existing_investor_position(&ctx.accounts.investor_position.to_account_info())?;
        let mut request = load_existing_redemption_request(&ctx.accounts.redemption_request.to_account_info())?;
        require_keys_eq!(position.facility, facility_key, PrivateCreditError::Unauthorized);
        require_keys_eq!(position.investor, investor_key, PrivateCreditError::Unauthorized);
        require_keys_eq!(request.facility, facility_key, PrivateCreditError::Unauthorized);
        require_keys_eq!(request.investor, investor_key, PrivateCreditError::Unauthorized);
        require!(position.status != InvestorPositionStatus::Frozen, PrivateCreditError::InvestorPositionFrozen);
        require!(matches!(request.status, RedemptionStatus::Requested | RedemptionStatus::Queued | RedemptionStatus::PartiallyProcessed), PrivateCreditError::RedemptionNotCancellable);
        let remaining_amount = checked_sub(request.requested_amount, request.processed_amount)?;
        require!(remaining_amount > 0, PrivateCreditError::RedemptionNotCancellable);
        require!(remaining_amount <= position.pending_redemption_amount, PrivateCreditError::ExceedsInvestorBalance);
        require!(remaining_amount <= ctx.accounts.facility.total_pending_redemptions, PrivateCreditError::ExceedsInvestorBalance);
        position.pending_redemption_amount = checked_sub(position.pending_redemption_amount, remaining_amount)?;
        if position.pending_redemption_amount > 0 {
            position.status = InvestorPositionStatus::RedemptionPending;
        } else if position.share_balance == 0 {
            position.status = InvestorPositionStatus::Redeemed;
        } else if position.redeemed_amount > 0 {
            position.status = InvestorPositionStatus::PartiallyRedeemed;
        } else {
            position.status = InvestorPositionStatus::Active;
        }
        position.updated_at = now;
        request.requested_amount = request.processed_amount;
        request.status = RedemptionStatus::Cancelled;
        request.timestamp = now;
        save_investor_position(&ctx.accounts.investor_position.to_account_info(), &position)?;
        save_redemption_request(&ctx.accounts.redemption_request.to_account_info(), &request)?;
        let facility = &mut ctx.accounts.facility;
        facility.total_pending_redemptions = checked_sub(facility.total_pending_redemptions, remaining_amount)?;
        write_event_log(
            &mut ctx.accounts.event_log,
            facility,
            event_index,
            EventType::RedemptionCancelled,
            investor_key,
            remaining_amount,
            request.queue_index as u16,
            ctx.bumps.event_log,
            now,
        )?;
        Ok(())
    }

    pub fn process_redemption(
        ctx: Context<ProcessRedemption>,
        amount: u64,
        event_index: u64,
    ) -> Result<()> {
        let now = Clock::get()?.unix_timestamp;
        require_operator(&ctx.accounts.facility, ctx.accounts.operator.key())?;
        require!(amount > 0, PrivateCreditError::AmountMustBeGreaterThanZero);
        require_redemptions_allowed(&ctx.accounts.facility)?;
        let facility_key = ctx.accounts.facility.key();
        let investor_key = ctx.accounts.investor.key();
        let mut position = load_existing_investor_position(&ctx.accounts.investor_position.to_account_info())?;
        let mut request = load_existing_redemption_request(&ctx.accounts.redemption_request.to_account_info())?;
        require_keys_eq!(position.facility, facility_key, PrivateCreditError::Unauthorized);
        require_keys_eq!(position.investor, investor_key, PrivateCreditError::Unauthorized);
        require_keys_eq!(request.facility, facility_key, PrivateCreditError::Unauthorized);
        require_keys_eq!(request.investor, investor_key, PrivateCreditError::Unauthorized);
        require!(position.status != InvestorPositionStatus::Frozen, PrivateCreditError::InvestorPositionFrozen);
        require!(request.status != RedemptionStatus::Processed, PrivateCreditError::RedemptionAlreadyProcessed);
        require!(request.status != RedemptionStatus::Frozen, PrivateCreditError::RedemptionFrozen);
        require!(request.status != RedemptionStatus::Cancelled, PrivateCreditError::RedemptionFrozen);
        let remaining_request = checked_sub(request.requested_amount, request.processed_amount)?;
        require!(amount <= remaining_request, PrivateCreditError::ExceedsInvestorBalance);
        require!(amount <= ctx.accounts.facility.available_liquidity, PrivateCreditError::ExceedsAvailableLiquidity);
        let post_redemption_liquidity = checked_sub(ctx.accounts.facility.available_liquidity, amount)?;
        require!(post_redemption_liquidity >= ctx.accounts.facility.minimum_liquidity_reserve, PrivateCreditError::ExceedsAvailableLiquidity);
        require!(amount <= position.share_balance, PrivateCreditError::ExceedsInvestorBalance);
        require!(amount <= position.pending_redemption_amount, PrivateCreditError::ExceedsInvestorBalance);
        position.share_balance = checked_sub(position.share_balance, amount)?;
        position.principal = checked_sub(position.principal, amount)?;
        position.redeemed_amount = checked_add(position.redeemed_amount, amount)?;
        position.pending_redemption_amount = checked_sub(position.pending_redemption_amount, amount)?;
        position.updated_at = now;
        if position.share_balance == 0 {
            position.status = InvestorPositionStatus::Redeemed;
        } else if position.pending_redemption_amount > 0 {
            position.status = InvestorPositionStatus::RedemptionPending;
        } else {
            position.status = InvestorPositionStatus::PartiallyRedeemed;
        }
        request.processed_amount = checked_add(request.processed_amount, amount)?;
        request.processed_timestamp = now;
        if request.processed_amount == request.requested_amount {
            request.status = RedemptionStatus::Processed;
        } else {
            request.status = RedemptionStatus::PartiallyProcessed;
        }
        save_investor_position(&ctx.accounts.investor_position.to_account_info(), &position)?;
        save_redemption_request(&ctx.accounts.redemption_request.to_account_info(), &request)?;
        let facility = &mut ctx.accounts.facility;
        facility.available_liquidity = post_redemption_liquidity;
        facility.total_pending_redemptions = checked_sub(facility.total_pending_redemptions, amount)?;
        // Simulated redemption; no stablecoins transferred.
        write_event_log(
            &mut ctx.accounts.event_log,
            facility,
            event_index,
            EventType::RedemptionProcessed,
            ctx.accounts.operator.key(),
            amount,
            request.queue_index as u16,
            ctx.bumps.event_log,
            now,
        )?;
        Ok(())
    }

    pub fn freeze_investor_position(
        ctx: Context<FreezeInvestorPosition>,
        event_index: u64,
    ) -> Result<()> {
        let now = Clock::get()?.unix_timestamp;
        require_operator(&ctx.accounts.facility, ctx.accounts.operator.key())?;
        let facility_key = ctx.accounts.facility.key();
        let investor_key = ctx.accounts.investor.key();
        let mut position = load_existing_investor_position(&ctx.accounts.investor_position.to_account_info())?;
        require_keys_eq!(position.facility, facility_key, PrivateCreditError::Unauthorized);
        require_keys_eq!(position.investor, investor_key, PrivateCreditError::Unauthorized);
        position.status = InvestorPositionStatus::Frozen;
        position.updated_at = now;
        save_investor_position(&ctx.accounts.investor_position.to_account_info(), &position)?;
        if !ctx.accounts.redemption_request.to_account_info().data_is_empty() {
            let mut request = load_existing_redemption_request(&ctx.accounts.redemption_request.to_account_info())?;
            require_keys_eq!(request.facility, facility_key, PrivateCreditError::Unauthorized);
            require_keys_eq!(request.investor, investor_key, PrivateCreditError::Unauthorized);
            if matches!(request.status, RedemptionStatus::Requested | RedemptionStatus::Queued | RedemptionStatus::PartiallyProcessed) {
                request.status = RedemptionStatus::Frozen;
                save_redemption_request(&ctx.accounts.redemption_request.to_account_info(), &request)?;
            }
        }
        // Production freezes require legally appropriate compliance and governance controls.
        let facility = &mut ctx.accounts.facility;
        write_event_log(
            &mut ctx.accounts.event_log,
            facility,
            event_index,
            EventType::InvestorFrozen,
            ctx.accounts.operator.key(),
            position.pending_redemption_amount,
            0,
            ctx.bumps.event_log,
            now,
        )?;
        Ok(())
    }

    pub fn mark_matured(
        ctx: Context<MarkMatured>,
        event_index: u64,
    ) -> Result<()> {
        let now = Clock::get()?.unix_timestamp;
        require_operator(&ctx.accounts.facility, ctx.accounts.operator.key())?;
        require_not_defaulted_or_closed(&ctx.accounts.facility)?;
        require!(now >= ctx.accounts.facility.maturity_timestamp || ctx.accounts.facility.outstanding_principal == 0, PrivateCreditError::InvalidMaturity);
        let facility = &mut ctx.accounts.facility;
        facility.status = FacilityStatus::Matured;
        if facility.outstanding_principal == 0 && facility.accrued_interest == 0 {
            facility.payment_status = PaymentStatus::Repaid;
        } else {
            facility.payment_status = PaymentStatus::Due;
        }
        let schedule = &mut ctx.accounts.repayment_schedule;
        schedule.status = facility.payment_status;
        schedule.updated_at = now;
        let outstanding_principal = facility.outstanding_principal;
        write_event_log(
            &mut ctx.accounts.event_log,
            facility,
            event_index,
            EventType::FacilityMatured,
            ctx.accounts.operator.key(),
            outstanding_principal,
            0,
            ctx.bumps.event_log,
            now,
        )?;
        Ok(())
    }

    pub fn raise_facility_default(
        ctx: Context<RaiseFacilityDefault>,
        default_reason_code: u16,
        event_index: u64,
    ) -> Result<()> {
        let now = Clock::get()?.unix_timestamp;
        require_operator(&ctx.accounts.facility, ctx.accounts.operator.key())?;
        require!(ctx.accounts.facility.status != FacilityStatus::Closed, PrivateCreditError::FacilityClosed);
        let facility = &mut ctx.accounts.facility;
        facility.status = FacilityStatus::Defaulted;
        facility.payment_status = PaymentStatus::Defaulted;
        facility.default_reason_code = default_reason_code;
        let schedule = &mut ctx.accounts.repayment_schedule;
        schedule.status = PaymentStatus::Defaulted;
        schedule.updated_at = now;
        write_event_log(
            &mut ctx.accounts.event_log,
            facility,
            event_index,
            EventType::FacilityDefaulted,
            ctx.accounts.operator.key(),
            0,
            default_reason_code,
            ctx.bumps.event_log,
            now,
        )?;
        Ok(())
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
        let now = Clock::get()?.unix_timestamp;
        require_operator(&ctx.accounts.facility, ctx.accounts.operator.key())?;
        require!(ctx.accounts.facility.status != FacilityStatus::Closed, PrivateCreditError::FacilityClosed);
        let facility = &mut ctx.accounts.facility;
        facility.collateral_description_hash = new_collateral_description_hash;
        if let Some(new_maturity) = new_maturity_timestamp {
            require!(facility.total_drawn == 0, PrivateCreditError::TermsCannotBeUpdated);
            require!(new_maturity > now, PrivateCreditError::InvalidMaturity);
            facility.maturity_timestamp = new_maturity;
        }
        if let Some(new_max_drawdown) = new_max_drawdown_amount {
            require!(facility.total_drawn == 0, PrivateCreditError::TermsCannotBeUpdated);
            require!(new_max_drawdown > 0, PrivateCreditError::AmountMustBeGreaterThanZero);
            require!(new_max_drawdown <= facility.facility_size, PrivateCreditError::ExceedsFacilitySize);
            facility.max_drawdown_amount = new_max_drawdown;
        }
        if let Some(new_borrower_key) = new_borrower {
            require!(facility.status == FacilityStatus::Initialized, PrivateCreditError::TermsCannotBeUpdated);
            facility.borrower = new_borrower_key;
        }
        if let Some(new_minimum_coverage) = new_minimum_collateral_coverage_bps {
            facility.minimum_collateral_coverage_bps = new_minimum_coverage;
        }
        if let Some(new_reserve) = new_minimum_liquidity_reserve {
            require!(new_reserve <= facility.facility_size, PrivateCreditError::ExceedsFacilitySize);
            facility.minimum_liquidity_reserve = new_reserve;
        }
        // Real amendments require consent, notices and legal documentation.
        write_event_log(
            &mut ctx.accounts.event_log,
            facility,
            event_index,
            EventType::TermsUpdated,
            ctx.accounts.operator.key(),
            0,
            0,
            ctx.bumps.event_log,
            now,
        )?;
        Ok(())
    }

    pub fn close_facility(
        ctx: Context<CloseFacility>,
        fully_resolved: bool,
        event_index: u64,
    ) -> Result<()> {
        let now = Clock::get()?.unix_timestamp;
        require_operator(&ctx.accounts.facility, ctx.accounts.operator.key())?;
        require!(ctx.accounts.facility.status != FacilityStatus::Closed, PrivateCreditError::FacilityClosed);
        let can_close = ctx.accounts.facility.outstanding_principal == 0 || fully_resolved;
        require!(can_close, PrivateCreditError::CannotCloseFacility);
        if !fully_resolved {
            require!(ctx.accounts.facility.total_pending_redemptions == 0, PrivateCreditError::CannotCloseFacility);
        }
        let facility = &mut ctx.accounts.facility;
        facility.status = FacilityStatus::Closed;
        if facility.outstanding_principal == 0 && facility.accrued_interest == 0 {
            facility.payment_status = PaymentStatus::Repaid;
        }
        write_event_log(
            &mut ctx.accounts.event_log,
            facility,
            event_index,
            EventType::FacilityClosed,
            ctx.accounts.operator.key(),
            0,
            0,
            ctx.bumps.event_log,
            now,
        )?;
        Ok(())
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

#[derive(Accounts)]
#[instruction(facility_id: String, event_index: u64)]
pub struct InitializeFacility<'info> {
    #[account(mut)]
    pub operator: Signer<'info>,
    /// CHECK: Demo borrower identity; no funds are transferred.
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
#[instruction(_debt_service_coverage_ratio_bps: u64, _loan_to_value_bps: u64, _minimum_liquidity_bps: u64, _reporting_current: bool, _breached: bool, _breach_reason: String, event_index: u64)]
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
#[instruction(_enabled: bool, _pik_accrual_amount: u64, event_index: u64)]
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
#[instruction(_collateral_value: u64, _collateral_status: CollateralStatus, _collateral_impaired: bool, event_index: u64)]
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
#[instruction(_default_reason_code: u16, event_index: u64)]
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
#[instruction(_new_collateral_description_hash: [u8; 32], _new_maturity_timestamp: Option<i64>, _new_max_drawdown_amount: Option<u64>, _new_borrower: Option<Pubkey>, _new_minimum_collateral_coverage_bps: Option<u64>, _new_minimum_liquidity_reserve: Option<u64>, event_index: u64)]
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
#[instruction(_fully_resolved: bool, event_index: u64)]
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

#[account]
#[derive(InitSpace)]
pub struct Facility {
    #[max_len(32)]
    pub facility_id: String,
    pub operator: Pubkey,
    pub borrower: Pubkey,
    pub stablecoin_mint: Pubkey,
    pub facility_size: u64,
    pub interest_rate_bps: u16,
    pub maturity_timestamp: i64,
    pub max_drawdown_amount: u64,
    pub minimum_collateral_coverage_bps: u64,
    pub minimum_liquidity_reserve: u64,
    pub collateral_description_hash: [u8; 32],
    pub status: FacilityStatus,
    pub payment_status: PaymentStatus,
    pub collateral_status: CollateralStatus,
    pub total_deposited: u64,
    pub total_drawn: u64,
    pub total_repaid: u64,
    pub available_liquidity: u64,
    pub outstanding_principal: u64,
    pub accrued_interest: u64,
    pub last_interest_accrual_timestamp: i64,
    pub missed_payment_count: u16,
    pub total_pending_redemptions: u64,
    pub redemption_queue_index: u64,
    pub pik_enabled: bool,
    pub total_pik_accrued: u64,
    pub last_pik_timestamp: i64,
    pub default_reason_code: u16,
    pub event_index: u64,
    pub created_at: i64,
    pub updated_at: i64,
    pub bump: u8,
}

#[account]
#[derive(InitSpace)]
pub struct InvestorPosition {
    pub facility: Pubkey,
    pub investor: Pubkey,
    pub principal: u64,
    pub share_balance: u64,
    pub deposited_amount: u64,
    pub redeemed_amount: u64,
    pub pending_redemption_amount: u64,
    pub status: InvestorPositionStatus,
    pub created_at: i64,
    pub updated_at: i64,
    pub bump: u8,
}

#[account]
#[derive(InitSpace)]
pub struct RepaymentSchedule {
    pub facility: Pubkey,
    pub expected_total_repayment: u64,
    pub next_due_timestamp: i64,
    pub scheduled_payment_amount: u64,
    pub last_payment_timestamp: i64,
    pub last_repayment_amount: u64,
    pub last_principal_portion: u64,
    pub last_interest_portion: u64,
    pub payments_made: u32,
    pub scheduled_payments_satisfied: u32,
    pub status: PaymentStatus,
    pub created_at: i64,
    pub updated_at: i64,
    pub bump: u8,
}

#[account]
#[derive(InitSpace)]
pub struct CovenantState {
    pub facility: Pubkey,
    pub debt_service_coverage_ratio_bps: u64,
    pub loan_to_value_bps: u64,
    pub minimum_liquidity_bps: u64,
    pub reporting_current: bool,
    pub last_report_timestamp: i64,
    pub breached: bool,
    #[max_len(96)]
    pub breach_reason: String,
    pub collateral_value: u64,
    pub collateral_status: CollateralStatus,
    pub last_valuation_timestamp: i64,
    pub collateral_impaired: bool,
    pub updated_by: Pubkey,
    pub created_at: i64,
    pub updated_at: i64,
    pub bump: u8,
}

#[account]
#[derive(InitSpace)]
pub struct RedemptionRequest {
    pub facility: Pubkey,
    pub investor: Pubkey,
    pub requested_amount: u64,
    pub processed_amount: u64,
    pub timestamp: i64,
    pub processed_timestamp: i64,
    pub status: RedemptionStatus,
    pub queue_index: u64,
    pub bump: u8,
}

#[account]
#[derive(InitSpace)]
pub struct EventLog {
    pub facility: Pubkey,
    pub event_index: u64,
    pub event_type: EventType,
    pub actor: Pubkey,
    pub timestamp: i64,
    pub amount: u64,
    pub status_snapshot: FacilityStatus,
    pub payment_status_snapshot: PaymentStatus,
    pub note_code: u16,
    pub bump: u8,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, InitSpace)]
pub enum FacilityStatus {
    Initialized,
    Active,
    Drawn,
    Performing,
    Breached,
    Defaulted,
    Matured,
    Closed,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, InitSpace)]
pub enum PaymentStatus {
    Current,
    Due,
    Late,
    Missed,
    Defaulted,
    Repaid,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, InitSpace)]
pub enum InvestorPositionStatus {
    Active,
    RedemptionPending,
    PartiallyRedeemed,
    Redeemed,
    Frozen,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, InitSpace)]
pub enum RedemptionStatus {
    Requested,
    Queued,
    PartiallyProcessed,
    Processed,
    Cancelled,
    Frozen,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, InitSpace)]
pub enum CollateralStatus {
    Unknown,
    Healthy,
    Watchlist,
    Impaired,
    LiquidationPending,
    Liquidated,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, InitSpace)]
pub enum EventType {
    FacilityInitialized,
    FacilityActivated,
    CapitalDeposited,
    BorrowerDrawdown,
    InterestAccrued,
    RepaymentMade,
    PaymentMissed,
    CovenantUpdated,
    CovenantBreached,
    PikToggled,
    PikCapitalized,
    CollateralUpdated,
    RedemptionRequested,
    RedemptionCancelled,
    RedemptionProcessed,
    InvestorFrozen,
    FacilityMatured,
    FacilityDefaulted,
    TermsUpdated,
    FacilityClosed,
}

#[error_code]
pub enum PrivateCreditError {
    #[msg("Unauthorized")]
    Unauthorized,
    #[msg("Invalid facility status for this action")]
    InvalidFacilityStatus,
    #[msg("Facility is closed")]
    FacilityClosed,
    #[msg("Facility is defaulted")]
    FacilityDefaulted,
    #[msg("Amount must be greater than zero")]
    AmountMustBeGreaterThanZero,
    #[msg("Amount exceeds facility size or max drawdown")]
    ExceedsFacilitySize,
    #[msg("Amount exceeds available liquidity")]
    ExceedsAvailableLiquidity,
    #[msg("Amount exceeds investor balance")]
    ExceedsInvestorBalance,
    #[msg("Redemption has already been processed")]
    RedemptionAlreadyProcessed,
    #[msg("Redemption is frozen or unavailable")]
    RedemptionFrozen,
    #[msg("Redemption is not cancellable")]
    RedemptionNotCancellable,
    #[msg("Investor position is frozen")]
    InvestorPositionFrozen,
    #[msg("Invalid maturity timestamp")]
    InvalidMaturity,
    #[msg("Math overflow")]
    MathOverflow,
    #[msg("Covenant is already breached")]
    CovenantAlreadyBreached,
    #[msg("Cannot close facility")]
    CannotCloseFacility,
    #[msg("Invalid repayment amount")]
    InvalidRepaymentAmount,
    #[msg("Invalid event index")]
    InvalidEventIndex,
    #[msg("String too long")]
    StringTooLong,
    #[msg("Investor position was not found")]
    InvestorPositionNotFound,
    #[msg("Redemption request was not found")]
    RedemptionRequestNotFound,
    #[msg("Facility terms cannot be updated after this lifecycle point")]
    TermsCannotBeUpdated,
}

fn checked_add(a: u64, b: u64) -> Result<u64> {
    a.checked_add(b).ok_or(error!(PrivateCreditError::MathOverflow))
}

fn checked_sub(a: u64, b: u64) -> Result<u64> {
    a.checked_sub(b).ok_or(error!(PrivateCreditError::MathOverflow))
}

fn calculate_interest_amount(
    principal: u64,
    interest_rate_bps: u16,
    elapsed_seconds: i64,
) -> Result<u64> {
    if principal == 0 || interest_rate_bps == 0 || elapsed_seconds <= 0 {
        return Ok(0);
    }
    let numerator = (principal as u128)
        .checked_mul(interest_rate_bps as u128)
        .ok_or(error!(PrivateCreditError::MathOverflow))?
        .checked_mul(elapsed_seconds as u128)
        .ok_or(error!(PrivateCreditError::MathOverflow))?;
    let denominator = (YEAR_SECONDS as u128)
        .checked_mul(BPS_DENOMINATOR)
        .ok_or(error!(PrivateCreditError::MathOverflow))?;
    let interest = numerator.checked_div(denominator).ok_or(error!(PrivateCreditError::MathOverflow))?;
    require!(interest <= u64::MAX as u128, PrivateCreditError::MathOverflow);
    Ok(interest as u64)
}

fn collateral_coverage_bps(collateral_value: u64, outstanding_principal: u64) -> Result<u64> {
    if outstanding_principal == 0 {
        return Ok(u64::MAX);
    }
    let numerator = (collateral_value as u128)
        .checked_mul(BPS_DENOMINATOR)
        .ok_or(error!(PrivateCreditError::MathOverflow))?;
    let coverage = numerator.checked_div(outstanding_principal as u128).ok_or(error!(PrivateCreditError::MathOverflow))?;
    require!(coverage <= u64::MAX as u128, PrivateCreditError::MathOverflow);
    Ok(coverage as u64)
}

fn require_operator(facility: &Facility, actor: Pubkey) -> Result<()> {
    require_keys_eq!(facility.operator, actor, PrivateCreditError::Unauthorized);
    Ok(())
}

fn require_borrower_or_operator(facility: &Facility, actor: Pubkey) -> Result<()> {
    require!(actor == facility.borrower || actor == facility.operator, PrivateCreditError::Unauthorized);
    Ok(())
}

fn require_not_defaulted_or_closed(facility: &Facility) -> Result<()> {
    require!(facility.status != FacilityStatus::Defaulted, PrivateCreditError::FacilityDefaulted);
    require!(facility.status != FacilityStatus::Closed, PrivateCreditError::FacilityClosed);
    Ok(())
}

fn require_deposits_allowed(facility: &Facility) -> Result<()> {
    require_not_defaulted_or_closed(facility)?;
    require!(matches!(facility.status, FacilityStatus::Initialized | FacilityStatus::Active), PrivateCreditError::InvalidFacilityStatus);
    Ok(())
}

fn require_drawdowns_allowed(facility: &Facility) -> Result<()> {
    require_not_defaulted_or_closed(facility)?;
    require!(matches!(facility.status, FacilityStatus::Active | FacilityStatus::Drawn | FacilityStatus::Performing), PrivateCreditError::InvalidFacilityStatus);
    Ok(())
}

fn require_redemptions_allowed(facility: &Facility) -> Result<()> {
    require_not_defaulted_or_closed(facility)?;
    require!(facility.status != FacilityStatus::Matured || facility.available_liquidity > 0, PrivateCreditError::ExceedsAvailableLiquidity);
    Ok(())
}

fn write_event_log(
    event_log: &mut Account<EventLog>,
    facility: &mut Account<Facility>,
    event_index: u64,
    event_type: EventType,
    actor: Pubkey,
    amount: u64,
    note_code: u16,
    bump: u8,
    now: i64,
) -> Result<()> {
    require!(event_index == facility.event_index, PrivateCreditError::InvalidEventIndex);
    event_log.facility = facility.key();
    event_log.event_index = event_index;
    event_log.event_type = event_type;
    event_log.actor = actor;
    event_log.timestamp = now;
    event_log.amount = amount;
    event_log.status_snapshot = facility.status;
    event_log.payment_status_snapshot = facility.payment_status;
    event_log.note_code = note_code;
    event_log.bump = bump;
    facility.event_index = checked_add(facility.event_index, 1)?;
    facility.updated_at = now;
    Ok(())
}

fn create_pda_account<'info>(
    account_info: &AccountInfo<'info>,
    payer_info: &AccountInfo<'info>,
    system_program_info: &AccountInfo<'info>,
    space: usize,
    signer_seeds: &[&[u8]],
) -> Result<()> {
    if account_info.data_is_empty() {
        let rent = Rent::get()?;
        let lamports = rent.minimum_balance(space);
        let ix = system_instruction::create_account(
            payer_info.key,
            account_info.key,
            lamports,
            space as u64,
            &crate::ID,
        );
        invoke_signed(
            &ix,
            &[payer_info.clone(), account_info.clone(), system_program_info.clone()],
            &[signer_seeds],
        )?;
    }
    Ok(())
}

fn load_or_init_investor_position<'info>(
    account_info: &AccountInfo<'info>,
    payer_info: &AccountInfo<'info>,
    system_program_info: &AccountInfo<'info>,
    facility: &Pubkey,
    investor: &Pubkey,
    bump: u8,
    now: i64,
) -> Result<InvestorPosition> {
    let space = ACCOUNT_DISCRIMINATOR_SIZE + InvestorPosition::INIT_SPACE;
    let bump_seed = [bump];
    let signer_seeds: &[&[u8]] = &[
        b"investor", facility.as_ref(), investor.as_ref(), bump_seed.as_ref(),
    ];
    if account_info.data_is_empty() {
        create_pda_account(account_info, payer_info, system_program_info, space, signer_seeds)?;
        let position = InvestorPosition {
            facility: *facility,
            investor: *investor,
            principal: 0,
            share_balance: 0,
            deposited_amount: 0,
            redeemed_amount: 0,
            pending_redemption_amount: 0,
            status: InvestorPositionStatus::Active,
            created_at: now,
            updated_at: now,
            bump,
        };
        save_investor_position(account_info, &position)?;
        Ok(position)
    } else {
        load_existing_investor_position(account_info)
    }
}

fn load_existing_investor_position<'info>(account_info: &AccountInfo<'info>) -> Result<InvestorPosition> {
    require!(!account_info.data_is_empty(), PrivateCreditError::InvestorPositionNotFound);
    require_keys_eq!(*account_info.owner, crate::ID, PrivateCreditError::Unauthorized);
    let data = account_info.try_borrow_data()?;
    let mut data_slice: &[u8] = &data[..];
    InvestorPosition::try_deserialize(&mut data_slice)
}

fn save_investor_position<'info>(account_info: &AccountInfo<'info>, position: &InvestorPosition) -> Result<()> {
    require_keys_eq!(*account_info.owner, crate::ID, PrivateCreditError::Unauthorized);
    let mut data = account_info.try_borrow_mut_data()?;
    let mut cursor = Cursor::new(&mut data[..]);
    position.try_serialize(&mut cursor)?;
    Ok(())
}

fn load_or_init_redemption_request<'info>(
    account_info: &AccountInfo<'info>,
    payer_info: &AccountInfo<'info>,
    system_program_info: &AccountInfo<'info>,
    facility: &Pubkey,
    investor: &Pubkey,
    bump: u8,
    queue_index: u64,
    now: i64,
) -> Result<(RedemptionRequest, bool)> {
    let space = ACCOUNT_DISCRIMINATOR_SIZE + RedemptionRequest::INIT_SPACE;
    let bump_seed = [bump];
    let signer_seeds: &[&[u8]] = &[
        b"redemption", facility.as_ref(), investor.as_ref(), bump_seed.as_ref(),
    ];
    if account_info.data_is_empty() {
        create_pda_account(account_info, payer_info, system_program_info, space, signer_seeds)?;
        let request = RedemptionRequest {
            facility: *facility,
            investor: *investor,
            requested_amount: 0,
            processed_amount: 0,
            timestamp: now,
            processed_timestamp: 0,
            status: RedemptionStatus::Requested,
            queue_index,
            bump,
        };
        save_redemption_request(account_info, &request)?;
        Ok((request, true))
    } else {
        Ok((load_existing_redemption_request(account_info)?, false))
    }
}

fn load_existing_redemption_request<'info>(account_info: &AccountInfo<'info>) -> Result<RedemptionRequest> {
    require!(!account_info.data_is_empty(), PrivateCreditError::RedemptionRequestNotFound);
    require_keys_eq!(*account_info.owner, crate::ID, PrivateCreditError::Unauthorized);
    let data = account_info.try_borrow_data()?;
    let mut data_slice: &[u8] = &data[..];
    RedemptionRequest::try_deserialize(&mut data_slice)
}

fn save_redemption_request<'info>(account_info: &AccountInfo<'info>, request: &RedemptionRequest) -> Result<()> {
    require_keys_eq!(*account_info.owner, crate::ID, PrivateCreditError::Unauthorized);
    let mut data = account_info.try_borrow_mut_data()?;
    let mut cursor = Cursor::new(&mut data[..]);
    request.try_serialize(&mut cursor)?;
    Ok(())
}
