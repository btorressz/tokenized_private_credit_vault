//! Credit facility lifecycle operations and management.

use anchor_lang::prelude::*;
use crate::constants::MAX_FACILITY_ID_LEN;
use crate::errors::PrivateCreditError;
use crate::events::write_event_log;
use crate::state::{
    CovenantState, EventLog, EventType, Facility, FacilityStatus, PaymentStatus, CollateralStatus,
    RepaymentSchedule,
};
use crate::validation::require_operator;

pub fn process_initialize_facility(
    facility: &mut Account<Facility>,
    repayment_schedule: &mut Account<RepaymentSchedule>,
    covenant_state: &mut Account<CovenantState>,
    event_log: &mut Account<EventLog>,
    operator_key: Pubkey,
    borrower_key: Pubkey,
    facility_key: Pubkey,
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
    facility_bump: u8,
    repayment_bump: u8,
    covenant_bump: u8,
    event_bump: u8,
    now: i64,
) -> Result<()> {
    require!(
        facility_id.as_bytes().len() <= MAX_FACILITY_ID_LEN,
        PrivateCreditError::StringTooLong
    );
    require!(event_index == 0, PrivateCreditError::InvalidEventIndex);
    require!(
        facility_size > 0 && max_drawdown_amount > 0,
        PrivateCreditError::AmountMustBeGreaterThanZero
    );
    require!(
        max_drawdown_amount <= facility_size,
        PrivateCreditError::ExceedsFacilitySize
    );
    require!(
        minimum_liquidity_reserve <= facility_size,
        PrivateCreditError::ExceedsFacilitySize
    );
    require!(maturity_timestamp > now, PrivateCreditError::InvalidMaturity);

    facility.facility_id = facility_id;
    facility.operator = operator_key;
    facility.borrower = borrower_key;
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
    facility.bump = facility_bump;

    repayment_schedule.facility = facility_key;
    repayment_schedule.expected_total_repayment = 0;
    repayment_schedule.next_due_timestamp = maturity_timestamp;
    repayment_schedule.scheduled_payment_amount = 0;
    repayment_schedule.last_payment_timestamp = 0;
    repayment_schedule.last_repayment_amount = 0;
    repayment_schedule.last_principal_portion = 0;
    repayment_schedule.last_interest_portion = 0;
    repayment_schedule.payments_made = 0;
    repayment_schedule.scheduled_payments_satisfied = 0;
    repayment_schedule.status = PaymentStatus::Current;
    repayment_schedule.created_at = now;
    repayment_schedule.updated_at = now;
    repayment_schedule.bump = repayment_bump;

    covenant_state.facility = facility_key;
    covenant_state.debt_service_coverage_ratio_bps = 0;
    covenant_state.loan_to_value_bps = 0;
    covenant_state.minimum_liquidity_bps = 0;
    covenant_state.reporting_current = false;
    covenant_state.last_report_timestamp = 0;
    covenant_state.breached = false;
    covenant_state.breach_reason = String::new();
    covenant_state.collateral_value = 0;
    covenant_state.collateral_status = CollateralStatus::Unknown;
    covenant_state.last_valuation_timestamp = 0;
    covenant_state.collateral_impaired = false;
    covenant_state.updated_by = operator_key;
    covenant_state.created_at = now;
    covenant_state.updated_at = now;
    covenant_state.bump = covenant_bump;

    write_event_log(
        event_log,
        facility,
        event_index,
        EventType::FacilityInitialized,
        operator_key,
        0,
        0,
        event_bump,
        now,
    )?;
    Ok(())
}

pub fn process_activate_facility(
    facility: &mut Account<Facility>,
    event_log: &mut Account<EventLog>,
    operator_key: Pubkey,
    event_index: u64,
    event_bump: u8,
    now: i64,
) -> Result<()> {
    require_operator(facility, operator_key)?;
    require!(
        facility.status == FacilityStatus::Initialized,
        PrivateCreditError::InvalidFacilityStatus
    );
    require!(
        now < facility.maturity_timestamp,
        PrivateCreditError::InvalidMaturity
    );

    facility.status = FacilityStatus::Active;
    write_event_log(
        event_log,
        facility,
        event_index,
        EventType::FacilityActivated,
        operator_key,
        0,
        0,
        event_bump,
        now,
    )?;
    Ok(())
}

pub fn process_mark_matured(
    facility: &mut Account<Facility>,
    repayment_schedule: &mut Account<RepaymentSchedule>,
    event_log: &mut Account<EventLog>,
    operator_key: Pubkey,
    event_index: u64,
    event_bump: u8,
    now: i64,
) -> Result<()> {
    require_operator(facility, operator_key)?;
    require!(
        facility.status != FacilityStatus::Defaulted && facility.status != FacilityStatus::Closed,
        PrivateCreditError::FacilityClosed
    );
    require!(
        now >= facility.maturity_timestamp || facility.outstanding_principal == 0,
        PrivateCreditError::InvalidMaturity
    );

    facility.status = FacilityStatus::Matured;
    if facility.outstanding_principal == 0 && facility.accrued_interest == 0 {
        facility.payment_status = PaymentStatus::Repaid;
    } else {
        facility.payment_status = PaymentStatus::Due;
    }

    repayment_schedule.status = facility.payment_status;
    repayment_schedule.updated_at = now;

    let outstanding = facility.outstanding_principal;
    write_event_log(
        event_log,
        facility,
        event_index,
        EventType::FacilityMatured,
        operator_key,
        outstanding,
        0,
        event_bump,
        now,
    )?;
    Ok(())
}

pub fn process_raise_facility_default(
    facility: &mut Account<Facility>,
    repayment_schedule: &mut Account<RepaymentSchedule>,
    event_log: &mut Account<EventLog>,
    operator_key: Pubkey,
    default_reason_code: u16,
    event_index: u64,
    event_bump: u8,
    now: i64,
) -> Result<()> {
    require_operator(facility, operator_key)?;
    require!(
        facility.status != FacilityStatus::Closed,
        PrivateCreditError::FacilityClosed
    );

    facility.status = FacilityStatus::Defaulted;
    facility.payment_status = PaymentStatus::Defaulted;
    facility.default_reason_code = default_reason_code;

    repayment_schedule.status = PaymentStatus::Defaulted;
    repayment_schedule.updated_at = now;

    write_event_log(
        event_log,
        facility,
        event_index,
        EventType::FacilityDefaulted,
        operator_key,
        0,
        default_reason_code,
        event_bump,
        now,
    )?;
    Ok(())
}

pub fn process_update_facility_metadata(
    facility: &mut Account<Facility>,
    event_log: &mut Account<EventLog>,
    operator_key: Pubkey,
    new_collateral_description_hash: [u8; 32],
    new_maturity_timestamp: Option<i64>,
    new_max_drawdown_amount: Option<u64>,
    new_borrower: Option<Pubkey>,
    new_minimum_collateral_coverage_bps: Option<u64>,
    new_minimum_liquidity_reserve: Option<u64>,
    event_index: u64,
    event_bump: u8,
    now: i64,
) -> Result<()> {
    require_operator(facility, operator_key)?;
    require!(
        facility.status != FacilityStatus::Closed,
        PrivateCreditError::FacilityClosed
    );

    facility.collateral_description_hash = new_collateral_description_hash;

    if let Some(new_maturity) = new_maturity_timestamp {
        require!(
            facility.total_drawn == 0,
            PrivateCreditError::TermsCannotBeUpdated
        );
        require!(new_maturity > now, PrivateCreditError::InvalidMaturity);
        facility.maturity_timestamp = new_maturity;
    }

    if let Some(new_max_drawdown) = new_max_drawdown_amount {
        require!(
            facility.total_drawn == 0,
            PrivateCreditError::TermsCannotBeUpdated
        );
        require!(
            new_max_drawdown > 0,
            PrivateCreditError::AmountMustBeGreaterThanZero
        );
        require!(
            new_max_drawdown <= facility.facility_size,
            PrivateCreditError::ExceedsFacilitySize
        );
        facility.max_drawdown_amount = new_max_drawdown;
    }

    if let Some(new_borrower_key) = new_borrower {
        require!(
            facility.status == FacilityStatus::Initialized,
            PrivateCreditError::TermsCannotBeUpdated
        );
        facility.borrower = new_borrower_key;
    }

    if let Some(new_minimum_coverage) = new_minimum_collateral_coverage_bps {
        facility.minimum_collateral_coverage_bps = new_minimum_coverage;
    }

    if let Some(new_reserve) = new_minimum_liquidity_reserve {
        require!(
            new_reserve <= facility.facility_size,
            PrivateCreditError::ExceedsFacilitySize
        );
        facility.minimum_liquidity_reserve = new_reserve;
    }

    write_event_log(
        event_log,
        facility,
        event_index,
        EventType::TermsUpdated,
        operator_key,
        0,
        0,
        event_bump,
        now,
    )?;
    Ok(())
}

pub fn process_close_facility(
    facility: &mut Account<Facility>,
    event_log: &mut Account<EventLog>,
    operator_key: Pubkey,
    fully_resolved: bool,
    event_index: u64,
    event_bump: u8,
    now: i64,
) -> Result<()> {
    require_operator(facility, operator_key)?;
    require!(
        facility.status != FacilityStatus::Closed,
        PrivateCreditError::FacilityClosed
    );

    let can_close = facility.outstanding_principal == 0 || fully_resolved;
    require!(can_close, PrivateCreditError::CannotCloseFacility);

    if !fully_resolved {
        require!(
            facility.total_pending_redemptions == 0,
            PrivateCreditError::CannotCloseFacility
        );
    }

    facility.status = FacilityStatus::Closed;
    if facility.outstanding_principal == 0 && facility.accrued_interest == 0 {
        facility.payment_status = PaymentStatus::Repaid;
    }

    write_event_log(
        event_log,
        facility,
        event_index,
        EventType::FacilityClosed,
        operator_key,
        0,
        0,
        event_bump,
        now,
    )?;
    Ok(())
}
