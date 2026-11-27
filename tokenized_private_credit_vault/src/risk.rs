//! Deterministic Credit Risk Engine for tokenized private credit facilities.
//!
//! Heuristic scoring (0..=100) combining exposure utilization, collateral
//! coverage shortfalls, missed payment history, covenant breach status,
//! reporting freshness, and maturity stress.

use anchor_lang::prelude::*;
use crate::constants::BPS_DENOMINATOR;
use crate::math::{bps_of, collateral_coverage_bps, risk_score};
use crate::state::{CovenantState, Facility, FacilityStatus, RiskClass};

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq, Eq)]
pub struct RiskAssessment {
    pub score: u128,
    pub risk_class: RiskClass,
    pub utilization_bps: u64,
    pub collateral_coverage_bps: u64,
    pub dscr_bps: u64,
    pub ltv_bps: u64,
    pub missed_payment_count: u16,
    pub covenant_breached: bool,
    pub reporting_current: bool,
    pub collateral_impaired: bool,
    pub seconds_to_maturity: i64,
    pub evaluated_at: i64,
}

pub fn evaluate_facility_risk(
    facility: &Facility,
    covenant: &CovenantState,
    now: i64,
) -> Result<RiskAssessment> {
    let utilization_bps = if facility.facility_size == 0 {
        0
    } else {
        ((facility.outstanding_principal as u128) * BPS_DENOMINATOR / (facility.facility_size as u128)) as u64
    };

    let coverage_bps = collateral_coverage_bps(covenant.collateral_value, facility.outstanding_principal)?;

    let pending_ratio_bps = if facility.available_liquidity == 0 {
        0
    } else {
        ((facility.total_pending_redemptions as u128) * BPS_DENOMINATOR / (facility.available_liquidity as u128)) as u64
    };

    let seconds_to_maturity = facility.maturity_timestamp.saturating_sub(now);

    let defaulted = facility.status == FacilityStatus::Defaulted;

    let score = risk_score(
        utilization_bps,
        coverage_bps,
        facility.missed_payment_count,
        covenant.breached,
        covenant.reporting_current,
        covenant.collateral_impaired,
        seconds_to_maturity,
        pending_ratio_bps,
    )?;

    let risk_class = RiskClass::from_score(score, defaulted);

    Ok(RiskAssessment {
        score,
        risk_class,
        utilization_bps,
        collateral_coverage_bps: coverage_bps,
        dscr_bps: covenant.debt_service_coverage_ratio_bps,
        ltv_bps: covenant.loan_to_value_bps,
        missed_payment_count: facility.missed_payment_count,
        covenant_breached: covenant.breached,
        reporting_current: covenant.reporting_current,
        collateral_impaired: covenant.collateral_impaired,
        seconds_to_maturity,
        evaluated_at: now,
    })
}

pub fn apply_collateral_haircut(collateral_value: u64, haircut_bps: u64) -> Result<u64> {
    let haircut = bps_of(collateral_value, haircut_bps)?;
    Ok(collateral_value.saturating_sub(haircut))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_haircut() {
        let val = 1_000_000;
        let discounted = apply_collateral_haircut(val, 2_000).unwrap(); // 20% haircut
        assert_eq!(discounted, 800_000);
    }
}
