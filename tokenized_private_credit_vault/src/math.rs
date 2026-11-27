//! Checked, deterministic financial arithmetic. All on-chain money math goes
//! through these helpers; no floating point anywhere. Rounding policy: all
//! division rounds down (floor) and every function is overflow-safe.

use crate::constants::{BPS_DENOMINATOR, YEAR_SECONDS};
use crate::errors::PrivateCreditError;
use crate::state::RiskClass;
use anchor_lang::prelude::*;

pub fn checked_add(a: u64, b: u64) -> Result<u64> {
    a.checked_add(b).ok_or_else(|| error!(PrivateCreditError::MathOverflow))
}

pub fn checked_sub(a: u64, b: u64) -> Result<u64> {
    a.checked_sub(b).ok_or_else(|| error!(PrivateCreditError::MathOverflow))
}

pub fn checked_mul(a: u64, b: u64) -> Result<u64> {
    a.checked_mul(b).ok_or_else(|| error!(PrivateCreditError::MathOverflow))
}

/// Floor division with a zero-denominator guard.
pub fn checked_div(numerator: u64, denominator: u64) -> Result<u64> {
    require!(denominator > 0, PrivateCreditError::MathOverflow);
    Ok(numerator / denominator)
}

/// `value * bps / 10_000`, floor, overflow-safe.
pub fn bps_of(value: u64, bps: u64) -> Result<u64> {
    let v = (value as u128)
        .checked_mul(bps as u128)
        .ok_or_else(|| error!(PrivateCreditError::MathOverflow))?;
    let out = v / BPS_DENOMINATOR;
    u64::try_from(out).map_err(|_| error!(PrivateCreditError::MathOverflow))
}

/// Time-based simple interest: `principal * rate_bps * elapsed / (year * 10_000)`.
pub fn calculate_interest_amount(principal: u64, interest_rate_bps: u16, elapsed_seconds: i64) -> Result<u64> {
    if principal == 0 || interest_rate_bps == 0 || elapsed_seconds <= 0 {
        return Ok(0);
    }
    let numerator = (principal as u128)
        .checked_mul(interest_rate_bps as u128)
        .ok_or_else(|| error!(PrivateCreditError::MathOverflow))?
        .checked_mul(elapsed_seconds as u128)
        .ok_or_else(|| error!(PrivateCreditError::MathOverflow))?;
    let denominator = (YEAR_SECONDS as u128)
        .checked_mul(BPS_DENOMINATOR)
        .ok_or_else(|| error!(PrivateCreditError::MathOverflow))?;
    let interest = numerator / denominator;
    u64::try_from(interest).map_err(|_| error!(PrivateCreditError::MathOverflow))
}

/// Collateral coverage in bps (u64::MAX when there is no exposure).
pub fn collateral_coverage_bps(collateral_value: u64, outstanding_principal: u64) -> Result<u64> {
    if outstanding_principal == 0 {
        return Ok(u64::MAX);
    }
    let numerator = (collateral_value as u128)
        .checked_mul(BPS_DENOMINATOR)
        .ok_or_else(|| error!(PrivateCreditError::MathOverflow))?;
    let coverage = numerator / outstanding_principal as u128;
    u64::try_from(coverage).map_err(|_| error!(PrivateCreditError::MathOverflow))
}

/// Loan-to-value in bps (u64::MAX when there is no collateral).
pub fn loan_to_value_bps(outstanding_principal: u64, collateral_value: u64) -> Result<u64> {
    if collateral_value == 0 {
        return Ok(u64::MAX);
    }
    let numerator = (outstanding_principal as u128)
        .checked_mul(BPS_DENOMINATOR)
        .ok_or_else(|| error!(PrivateCreditError::MathOverflow))?;
    let ltv = numerator / collateral_value as u128;
    u64::try_from(ltv).map_err(|_| error!(PrivateCreditError::MathOverflow))
}

/// Debt service coverage in bps (annualized debt service over elapsed period).
pub fn debt_service_coverage_bps(available_cash: u64, debt_service: u64) -> Result<u64> {
    if debt_service == 0 {
        return Ok(u64::MAX);
    }
    let numerator = (available_cash as u128)
        .checked_mul(BPS_DENOMINATOR)
        .ok_or_else(|| error!(PrivateCreditError::MathOverflow))?;
    let dscr = numerator / debt_service as u128;
    u64::try_from(dscr).map_err(|_| error!(PrivateCreditError::MathOverflow))
}

/// NAV per share in VALUATION_PRECISION fixed point, floor.
pub fn nav_per_share(net_asset_value: u64, total_shares: u64) -> Result<u128> {
    require!(total_shares > 0, PrivateCreditError::InvalidShareValuation);
    let numerator = (net_asset_value as u128)
        .checked_mul(crate::constants::VALUATION_PRECISION)
        .ok_or_else(|| error!(PrivateCreditError::MathOverflow))?;
    Ok(numerator / total_shares as u128)
}

/// Convert `shares` at `price` (fixed point) into an amount, floor.
pub fn shares_to_amount(shares: u64, price: u128) -> Result<u64> {
    let v = (shares as u128)
        .checked_mul(price)
        .ok_or_else(|| error!(PrivateCreditError::MathOverflow))?
        / crate::constants::VALUATION_PRECISION;
    u64::try_from(v).map_err(|_| error!(PrivateCreditError::MathOverflow))
}

/// Convert an `amount` into shares at `price` (fixed point), floor.
pub fn amount_to_shares(amount: u64, price: u128) -> Result<u64> {
    require!(price > 0, PrivateCreditError::InvalidShareValuation);
    let v = (amount as u128)
        .checked_mul(crate::constants::VALUATION_PRECISION)
        .ok_or_else(|| error!(PrivateCreditError::MathOverflow))?
        / price;
    u64::try_from(v).map_err(|_| error!(PrivateCreditError::MathOverflow))
}

/// Split `amount` into `n` proportional parts per `weights[i]/sum(weights)`.
/// Returns parts whose sum equals `amount` (remainder to the first parts).
pub fn proportional_split(amount: u64, weights: &[u64]) -> Result<Vec<u64>> {
    let total: u128 = weights.iter().map(|w| *w as u128).sum();
    if total == 0 || weights.is_empty() {
        return Ok(vec![0; weights.len()]);
    }
    let mut parts = Vec::with_capacity(weights.len());
    let mut distributed: u64 = 0;
    for w in weights {
        let raw = (amount as u128)
            .checked_mul(*w as u128)
            .ok_or_else(|| error!(PrivateCreditError::MathOverflow))?
            / total;
        let p = u64::try_from(raw).map_err(|_| error!(PrivateCreditError::MathOverflow))?;
        parts.push(p);
        distributed = checked_add(distributed, p)?;
    }
    // Dust: distribute the (sum(parts) < amount) remainder left by flooring.
    if let Some(first) = parts.first_mut() {
        *first = checked_add(*first, checked_sub(amount, distributed)?)?;
    }
    Ok(parts)
}

/// Bounded, explainable risk score (0..=100). Heuristic — NOT a validated
/// probability of default. Each factor contributes a documented, capped amount.
pub fn risk_score(
    utilization_bps: u64,           // facility_size denominator
    collateral_coverage: u64,       // bps
    missed_payments: u16,
    covenant_breached: bool,
    reporting_current: bool,
    collateral_impaired: bool,
    seconds_to_maturity: i64,
    pending_redemption_ratio_bps: u64, // of available liquidity
) -> Result<u128> {
    let mut score: u128 = 0;
    // Utilization: up to 20 points (full utilization = fully drawn = riskier).
    score = score.saturating_add((utilization_bps.min(10_000) as u128) * 20 / 10_000);
    // Collateral: below 100% coverage adds up to 25 points.
    if collateral_coverage != u64::MAX {
        let shortfall = 10_000u64.saturating_sub(collateral_coverage.min(10_000));
        score = score.saturating_add((shortfall as u128) * 25 / 10_000);
    }
    // Missed payments: 15 points each, capped at 30.
    score = score.saturating_add((missed_payments as u128).saturating_mul(15).min(30));
    if covenant_breached {
        score = score.saturating_add(15);
    }
    if !reporting_current {
        score = score.saturating_add(10);
    }
    if collateral_impaired {
        score = score.saturating_add(10);
    }
    // Liquidity stress: pending redemptions vs available liquidity, up to 10.
    score = score.saturating_add((pending_redemption_ratio_bps.min(10_000) as u128) * 10 / 10_000);
    // Maturity wall: inside 30 days adds 5.
    if seconds_to_maturity >= 0 && seconds_to_maturity < 2_592_000 {
        score = score.saturating_add(5);
    }
    Ok(score.min(crate::constants::RISK_SCALE))
}

pub fn classify(score: u128, defaulted: bool) -> RiskClass {
    RiskClass::from_score(score, defaulted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interest_accrues_linearly() {
        // 10% bps? 1_000 bps on 1_000_000 for a full year = 100_000.
        assert_eq!(calculate_interest_amount(1_000_000, 1_000, YEAR_SECONDS).unwrap(), 100_000);
        assert_eq!(calculate_interest_amount(0, 1_000, 100).unwrap(), 0);
        assert_eq!(calculate_interest_amount(1_000, 1_000, 0).unwrap(), 0);
    }

    #[test]
    fn coverage_and_ltv() {
        assert_eq!(collateral_coverage_bps(2_000_000, 1_000_000).unwrap(), 20_000);
        assert_eq!(collateral_coverage_bps(100, 0).unwrap(), u64::MAX);
        assert_eq!(loan_to_value_bps(500_000, 1_000_000).unwrap(), 5_000);
    }

    #[test]
    fn split_conserves_amount() {
        let parts = proportional_split(1_003, &[1, 1, 1]).unwrap();
        assert_eq!(parts.iter().sum::<u64>(), 1_003);
    }

    #[test]
    fn risk_score_is_bounded() {
        let s = risk_score(10_000, 5_000, 2, true, false, true, 0, 10_000).unwrap();
        assert!(s <= 100);
        assert_eq!(classify(s, true), RiskClass::Defaulted);
        assert_eq!(classify(0, false), RiskClass::Healthy);
    }

    #[test]
    fn nav_round_trip() {
        let price = nav_per_share(1_500_000, 1_000_000).unwrap();
        assert_eq!(shares_to_amount(1_000_000, price).unwrap(), 1_500_000);
        let shares = amount_to_shares(750_000, price).unwrap();
        assert_eq!(shares_to_amount(shares, price).unwrap(), 750_000);
    }
}
