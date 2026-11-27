//! Collateral valuation, coverage verification, and status monitoring.

use anchor_lang::prelude::*;
use crate::errors::PrivateCreditError;
use crate::events::write_event_log;
use crate::math::collateral_coverage_bps;
use crate::state::{CollateralStatus, CovenantState, EventLog, EventType, Facility, FacilityStatus};
use crate::validation::require_operator;

pub fn process_update_collateral_status(
    facility: &mut Account<Facility>,
    covenant: &mut Account<CovenantState>,
    event_log: &mut Account<EventLog>,
    operator_key: Pubkey,
    collateral_value: u64,
    collateral_status: CollateralStatus,
    collateral_impaired: bool,
    event_index: u64,
    event_bump: u8,
    now: i64,
) -> Result<()> {
    require_operator(facility, operator_key)?;
    require!(
        facility.status != FacilityStatus::Closed,
        PrivateCreditError::FacilityClosed
    );

    let coverage_bps = collateral_coverage_bps(collateral_value, facility.outstanding_principal)?;
    let below_minimum_coverage = facility.outstanding_principal > 0
        && facility.minimum_collateral_coverage_bps > 0
        && coverage_bps < facility.minimum_collateral_coverage_bps;

    covenant.collateral_value = collateral_value;
    covenant.collateral_status = collateral_status;
    covenant.last_valuation_timestamp = now;
    covenant.collateral_impaired = collateral_impaired;
    covenant.updated_by = operator_key;
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
        if !matches!(facility.status, FacilityStatus::Defaulted | FacilityStatus::Matured | FacilityStatus::Initialized) {
            facility.status = FacilityStatus::Breached;
        }
    }

    write_event_log(
        event_log,
        facility,
        event_index,
        EventType::CollateralUpdated,
        operator_key,
        collateral_value,
        note_code,
        event_bump,
        now,
    )?;
    Ok(())
}
