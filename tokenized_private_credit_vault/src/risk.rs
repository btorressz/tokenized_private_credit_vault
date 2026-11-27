//! Deterministic Credit Risk Engine for tokenized private credit facilities.
//!
//! Heuristic scoring (0..=100) combining exposure utilization, collateral
//! coverage shortfalls, missed payment history, covenant breach status,
//! reporting freshness, and maturity stress.

use anchor_lang::prelude::*;
use crate::constants::{BPS_DENOMINATOR, REPORTING_DEADLINE_SECONDS, STALE_VALUATION_SECONDS};
use crate::errors::PrivateCreditError;
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
        ratio_bps(facility.outstanding_principal, facility.facility_size)
    };

    let coverage_bps = collateral_coverage_bps(covenant.collateral_value, facility.outstanding_principal)?;

    let pending_ratio_bps = ratio_bps(facility.total_pending_redemptions, facility.available_liquidity);
    let reporting_current = covenant.reporting_current && is_fresh(covenant.last_report_timestamp, now, REPORTING_DEADLINE_SECONDS);
    let collateral_current = is_fresh(covenant.last_valuation_timestamp, now, STALE_VALUATION_SECONDS);

    let seconds_to_maturity = facility.maturity_timestamp.saturating_sub(now);

    let defaulted = facility.status == FacilityStatus::Defaulted;

    let score = risk_score(
        utilization_bps,
        coverage_bps,
        facility.missed_payment_count,
        covenant.breached,
        reporting_current,
        covenant.collateral_impaired,
        seconds_to_maturity,
        pending_ratio_bps,
    )?;

    let risk_class = if defaulted { RiskClass::Defaulted } else if !reporting_current || (facility.outstanding_principal > 0 && !collateral_current) { RiskClass::InsufficientData } else { RiskClass::from_score(score, false) };

    Ok(RiskAssessment {
        score,
        risk_class,
        utilization_bps,
        collateral_coverage_bps: coverage_bps,
        dscr_bps: covenant.debt_service_coverage_ratio_bps,
        ltv_bps: covenant.loan_to_value_bps,
        missed_payment_count: facility.missed_payment_count,
        covenant_breached: covenant.breached,
        reporting_current,
        collateral_impaired: covenant.collateral_impaired,
        seconds_to_maturity,
        evaluated_at: now,
    })
}

pub fn apply_collateral_haircut(collateral_value: u64, haircut_bps: u64) -> Result<u64> {
    require!(haircut_bps <= 10_000, PrivateCreditError::InvalidRiskConfig);
    let haircut = bps_of(collateral_value, haircut_bps)?;
    Ok(collateral_value.saturating_sub(haircut))
}

fn ratio_bps(numerator: u64, denominator: u64) -> u64 {
    if denominator == 0 { return if numerator == 0 { 0 } else { u64::MAX }; }
    ((numerator as u128 * BPS_DENOMINATOR / denominator as u128).min(u64::MAX as u128)) as u64
}

fn is_fresh(timestamp: i64, now: i64, limit: i64) -> bool {
    timestamp > 0 && now.checked_sub(timestamp).map_or(false, |age| age >= 0 && age <= limit)
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
