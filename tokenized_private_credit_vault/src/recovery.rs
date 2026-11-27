//! Default Management, Debt Restructuring, and Recovery Engine.
//!
//! Handles post-default recovery workflows, debt write-downs, collateral
//! liquidation proceeds allocation, and restructuring accounting.
//! All accounting remains simulated for research/MVP purposes.

use anchor_lang::prelude::*;
use crate::errors::PrivateCreditError;
use crate::math::{bps_of, checked_add, checked_sub};

/// Recovery lifecycle status.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq, InitSpace)]
pub enum RecoveryState {
    NotStarted,
    UnderReview,
    RecoveryInProgress,
    RestructuringProposed,
    Restructured,
    PartiallyRecovered,
    FullyRecovered,
    WrittenOff,
    Closed,
}

/// Recovery metrics snapshot.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq, Eq)]
pub struct RecoveryMetrics {
    pub total_debt_at_default: u64,
    pub total_recovered: u64,
    pub principal_written_down: u64,
    pub interest_written_down: u64,
    pub net_realized_loss: u64,
    pub recovery_rate_bps: u64,
    pub loss_severity_bps: u64,
}

/// Calculate total outstanding debt exposure at default.
pub fn calculate_total_default_debt(
    outstanding_principal: u64,
    accrued_interest: u64,
    total_pik_accrued: u64,
) -> Result<u64> {
    let debt = checked_add(outstanding_principal, accrued_interest)?;
    checked_add(debt, total_pik_accrued)
}

/// Calculate loss severity and recovery rate in basis points (10,000 bps = 100%).
pub fn calculate_recovery_metrics(
    total_debt: u64,
    recovered_amount: u64,
    principal_written_down: u64,
    interest_written_down: u64,
) -> Result<RecoveryMetrics> {
    if total_debt == 0 {
        return Ok(RecoveryMetrics {
            total_debt_at_default: 0,
            total_recovered: recovered_amount,
            principal_written_down,
            interest_written_down,
            net_realized_loss: 0,
            recovery_rate_bps: 10_000,
            loss_severity_bps: 0,
        });
    }

    let total_written_down = checked_add(principal_written_down, interest_written_down)?;
    let net_realized_loss = total_written_down.min(total_debt);

    let recovery_rate_bps = ((recovered_amount as u128) * 10_000 / (total_debt as u128)) as u64;
    let loss_severity_bps = ((net_realized_loss as u128) * 10_000 / (total_debt as u128)) as u64;

    Ok(RecoveryMetrics {
        total_debt_at_default: total_debt,
        total_recovered: recovered_amount,
        principal_written_down,
        interest_written_down,
        net_realized_loss,
        recovery_rate_bps: recovery_rate_bps.min(10_000),
        loss_severity_bps: loss_severity_bps.min(10_000),
    })
}

/// Apply a principal write-down safely.
pub fn apply_principal_writedown(
    outstanding_principal: u64,
    writedown_amount: u64,
) -> Result<u64> {
    require!(
        writedown_amount <= outstanding_principal,
        PrivateCreditError::InvalidRepaymentAmount
    );
    checked_sub(outstanding_principal, writedown_amount)
}

/// Apply an interest write-down safely.
pub fn apply_interest_writedown(
    accrued_interest: u64,
    writedown_amount: u64,
) -> Result<u64> {
    require!(
        writedown_amount <= accrued_interest,
        PrivateCreditError::InvalidRepaymentAmount
    );
    checked_sub(accrued_interest, writedown_amount)
}

/// Allocate recovery proceeds between recovery fees, interest recovery, and principal recovery.
pub fn allocate_recovery_proceeds(
    recovered_amount: u64,
    recovery_fee_bps: u64,
    outstanding_interest: u64,
    outstanding_principal: u64,
) -> Result<(u64, u64, u64)> {
    let fee = bps_of(recovered_amount, recovery_fee_bps)?;
    let net_proceeds = checked_sub(recovered_amount, fee)?;

    let interest_covered = net_proceeds.min(outstanding_interest);
    let remaining_after_interest = checked_sub(net_proceeds, interest_covered)?;

    let principal_covered = remaining_after_interest.min(outstanding_principal);

    Ok((fee, interest_covered, principal_covered))
}

/// Validate recovery state machine transitions.
pub fn validate_recovery_transition(
    current: RecoveryState,
    target: RecoveryState,
) -> Result<()> {
    let valid = match (current, target) {
        (RecoveryState::NotStarted, RecoveryState::UnderReview) => true,
        (RecoveryState::NotStarted, RecoveryState::RecoveryInProgress) => true,
        (RecoveryState::UnderReview, RecoveryState::RecoveryInProgress) => true,
        (RecoveryState::UnderReview, RecoveryState::RestructuringProposed) => true,
        (RecoveryState::RecoveryInProgress, RecoveryState::RestructuringProposed) => true,
        (RecoveryState::RecoveryInProgress, RecoveryState::PartiallyRecovered) => true,
        (RecoveryState::RecoveryInProgress, RecoveryState::FullyRecovered) => true,
        (RecoveryState::RecoveryInProgress, RecoveryState::WrittenOff) => true,
        (RecoveryState::RestructuringProposed, RecoveryState::Restructured) => true,
        (RecoveryState::RestructuringProposed, RecoveryState::RecoveryInProgress) => true,
        (RecoveryState::Restructured, RecoveryState::RecoveryInProgress) => true,
        (RecoveryState::Restructured, RecoveryState::PartiallyRecovered) => true,
        (RecoveryState::PartiallyRecovered, RecoveryState::FullyRecovered) => true,
        (RecoveryState::PartiallyRecovered, RecoveryState::WrittenOff) => true,
        (RecoveryState::FullyRecovered, RecoveryState::Closed) => true,
        (RecoveryState::WrittenOff, RecoveryState::Closed) => true,
        _ => false,
    };

    require!(valid, PrivateCreditError::InvalidRecoveryTransition);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_total_debt_calculation() {
        let debt = calculate_total_default_debt(1_000_000, 50_000, 10_000).unwrap();
        assert_eq!(debt, 1_060_000);
    }

    #[test]
    fn test_recovery_metrics() {
        let metrics = calculate_recovery_metrics(1_000_000, 400_000, 600_000, 0).unwrap();
        assert_eq!(metrics.recovery_rate_bps, 4_000); // 40%
        assert_eq!(metrics.loss_severity_bps, 6_000); // 60%
    }

    #[test]
    fn test_allocation() {
        let (fee, interest, principal) = allocate_recovery_proceeds(500_000, 500, 50_000, 1_000_000).unwrap();
        // 5% fee = 25,000. Net = 475,000.
        assert_eq!(fee, 25_000);
        assert_eq!(interest, 50_000);
        assert_eq!(principal, 425_000);
    }

    #[test]
    fn test_valid_transitions() {
        assert!(validate_recovery_transition(RecoveryState::NotStarted, RecoveryState::UnderReview).is_ok());
        assert!(validate_recovery_transition(RecoveryState::UnderReview, RecoveryState::Closed).is_err());
    }
}
