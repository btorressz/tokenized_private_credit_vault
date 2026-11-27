//! Financial accounting engine for interest accrual, repayment tracking, missed payments, and PIK mechanics.

use anchor_lang::prelude::*;
use crate::constants::{MISSED_PAYMENT_DEFAULT_THRESHOLD, THIRTY_DAYS_SECONDS};
use crate::errors::PrivateCreditError;
use crate::events::write_event_log;
use crate::math::{calculate_interest_amount, checked_add, checked_sub};
use crate::state::{
    EventLog, EventType, Facility, FacilityStatus, PaymentStatus, RepaymentSchedule,
};
use crate::validation::{require_not_defaulted_or_closed, require_operator};

pub fn process_accrue_interest(
    facility: &mut Account<Facility>,
    event_log: &mut Account<EventLog>,
    operator_key: Pubkey,
    event_index: u64,
    event_bump: u8,
    now: i64,
) -> Result<()> {
    require_operator(facility, operator_key)?;
    require_not_defaulted_or_closed(facility)?;

    let elapsed_seconds = now
        .checked_sub(facility.last_interest_accrual_timestamp)
        .ok_or_else(|| error!(PrivateCreditError::MathOverflow))?;
    let interest_amount = calculate_interest_amount(
        facility.outstanding_principal,
        facility.interest_rate_bps,
        elapsed_seconds,
    )?;

    facility.accrued_interest = checked_add(facility.accrued_interest, interest_amount)?;
    facility.last_interest_accrual_timestamp = now;

    write_event_log(
        event_log,
        facility,
        event_index,
        EventType::InterestAccrued,
        operator_key,
        interest_amount,
        0,
        event_bump,
        now,
    )?;
    Ok(())
}

pub fn process_make_repayment(
    facility: &mut Account<Facility>,
    repayment_schedule: &mut Account<RepaymentSchedule>,
    event_log: &mut Account<EventLog>,
    borrower_key: Pubkey,
    amount: u64,
    principal_portion: u64,
    interest_portion: u64,
    scheduled_payment_satisfied: bool,
    event_index: u64,
    event_bump: u8,
    now: i64,
) -> Result<()> {
    require_keys_eq!(borrower_key, facility.borrower, PrivateCreditError::Unauthorized);
    require!(amount > 0, PrivateCreditError::AmountMustBeGreaterThanZero);
    require!(
        checked_add(principal_portion, interest_portion)? == amount,
        PrivateCreditError::InvalidRepaymentAmount
    );
    require!(
        principal_portion <= facility.outstanding_principal,
        PrivateCreditError::InvalidRepaymentAmount
    );
    require!(
        facility.status != FacilityStatus::Closed,
        PrivateCreditError::FacilityClosed
    );

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

    repayment_schedule.last_payment_timestamp = now;
    repayment_schedule.last_repayment_amount = amount;
    repayment_schedule.last_principal_portion = principal_portion;
    repayment_schedule.last_interest_portion = interest_portion;
    repayment_schedule.payments_made = repayment_schedule
        .payments_made
        .checked_add(1)
        .ok_or_else(|| error!(PrivateCreditError::MathOverflow))?;

    if scheduled_payment_satisfied {
        repayment_schedule.scheduled_payments_satisfied = repayment_schedule
            .scheduled_payments_satisfied
            .checked_add(1)
            .ok_or_else(|| error!(PrivateCreditError::MathOverflow))?;
        repayment_schedule.status = facility.payment_status;
    }
    repayment_schedule.updated_at = now;

    write_event_log(
        event_log,
        facility,
        event_index,
        EventType::RepaymentMade,
        borrower_key,
        amount,
        0,
        event_bump,
        now,
    )?;
    Ok(())
}

pub fn process_record_missed_payment(
    facility: &mut Account<Facility>,
    repayment_schedule: &mut Account<RepaymentSchedule>,
    event_log: &mut Account<EventLog>,
    operator_key: Pubkey,
    event_index: u64,
    event_bump: u8,
    now: i64,
) -> Result<()> {
    require_operator(facility, operator_key)?;
    require_not_defaulted_or_closed(facility)?;

    facility.missed_payment_count = facility
        .missed_payment_count
        .checked_add(1)
        .ok_or_else(|| error!(PrivateCreditError::MathOverflow))?;
    facility.payment_status = PaymentStatus::Missed;

    repayment_schedule.status = PaymentStatus::Missed;
    repayment_schedule.updated_at = now;

    if facility.missed_payment_count >= MISSED_PAYMENT_DEFAULT_THRESHOLD {
        facility.status = FacilityStatus::Defaulted;
        facility.payment_status = PaymentStatus::Defaulted;
        repayment_schedule.status = PaymentStatus::Defaulted;
    }

    let missed_count = facility.missed_payment_count;
    write_event_log(
        event_log,
        facility,
        event_index,
        EventType::PaymentMissed,
        operator_key,
        0,
        missed_count,
        event_bump,
        now,
    )?;
    Ok(())
}

pub fn process_toggle_pik(
    facility: &mut Account<Facility>,
    event_log: &mut Account<EventLog>,
    operator_key: Pubkey,
    enabled: bool,
    pik_accrual_amount: u64,
    event_index: u64,
    event_bump: u8,
    now: i64,
) -> Result<()> {
    require_operator(facility, operator_key)?;
    require_not_defaulted_or_closed(facility)?;

    facility.pik_enabled = enabled;
    facility.total_pik_accrued = checked_add(facility.total_pik_accrued, pik_accrual_amount)?;
    facility.last_pik_timestamp = now;

    write_event_log(
        event_log,
        facility,
        event_index,
        EventType::PikToggled,
        operator_key,
        pik_accrual_amount,
        if enabled { 1 } else { 0 },
        event_bump,
        now,
    )?;
    Ok(())
}

pub fn process_capitalize_pik(
    facility: &mut Account<Facility>,
    event_log: &mut Account<EventLog>,
    operator_key: Pubkey,
    event_index: u64,
    event_bump: u8,
    now: i64,
) -> Result<()> {
    require_operator(facility, operator_key)?;
    require_not_defaulted_or_closed(facility)?;
    require!(
        facility.total_pik_accrued > 0,
        PrivateCreditError::AmountMustBeGreaterThanZero
    );

    let capitalized_amount = facility.total_pik_accrued;
    facility.outstanding_principal = checked_add(facility.outstanding_principal, capitalized_amount)?;
    facility.total_pik_accrued = 0;
    facility.last_pik_timestamp = now;

    if facility.status == FacilityStatus::Active || facility.status == FacilityStatus::Drawn {
        facility.status = FacilityStatus::Performing;
    }

    write_event_log(
        event_log,
        facility,
        event_index,
        EventType::PikCapitalized,
        operator_key,
        capitalized_amount,
        0,
        event_bump,
        now,
    )?;
    Ok(())
}
