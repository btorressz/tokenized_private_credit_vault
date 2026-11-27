//! Covenant evaluation and monitoring engine.

use anchor_lang::prelude::*;
use crate::constants::MAX_COVENANT_REASON_LEN;
use crate::errors::PrivateCreditError;
use crate::events::write_event_log;
use crate::state::{CovenantState, EventLog, EventType, Facility, FacilityStatus};
use crate::validation::require_operator;

pub fn process_update_covenant_state(
    facility: &mut Account<Facility>,
    covenant: &mut Account<CovenantState>,
    event_log: &mut Account<EventLog>,
    operator_key: Pubkey,
    debt_service_coverage_ratio_bps: u64,
    loan_to_value_bps: u64,
    minimum_liquidity_bps: u64,
    reporting_current: bool,
    breached: bool,
    breach_reason: String,
    event_index: u64,
    event_bump: u8,
    now: i64,
) -> Result<()> {
    require_operator(facility, operator_key)?;
    require!(
        breach_reason.as_bytes().len() <= MAX_COVENANT_REASON_LEN,
        PrivateCreditError::StringTooLong
    );
    require!(
        facility.status != FacilityStatus::Closed,
        PrivateCreditError::FacilityClosed
    );

    covenant.debt_service_coverage_ratio_bps = debt_service_coverage_ratio_bps;
    covenant.loan_to_value_bps = loan_to_value_bps;
    covenant.minimum_liquidity_bps = minimum_liquidity_bps;
    covenant.reporting_current = reporting_current;
    covenant.last_report_timestamp = now;
    let breached = breached || covenant.collateral_impaired;
    covenant.breached = breached;
    covenant.updated_by = operator_key;
    covenant.updated_at = now;

    let event_type = if breached {
        covenant.breach_reason = breach_reason;
        if !matches!(facility.status, FacilityStatus::Defaulted | FacilityStatus::Matured | FacilityStatus::Initialized) {
            facility.status = FacilityStatus::Breached;
        }
        EventType::CovenantBreached
    } else {
        covenant.breach_reason = String::new();
        if facility.status == FacilityStatus::Breached && now < facility.maturity_timestamp {
            facility.status = if facility.outstanding_principal > 0 {
                FacilityStatus::Performing
            } else {
                FacilityStatus::Active
            };
        }
        EventType::CovenantUpdated
    };

    write_event_log(
        event_log,
        facility,
        event_index,
        event_type,
        operator_key,
        0,
        0,
        event_bump,
        now,
    )?;
    Ok(())
}
