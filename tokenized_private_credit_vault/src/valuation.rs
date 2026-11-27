//! Deterministic Valuation Engine for NAV calculation and share pricing.
//!
//! Provides GAV, NAV, credit loss adjustments, and fixed-point share conversion.

use anchor_lang::prelude::*;
use crate::constants::VALUATION_PRECISION;
use crate::errors::PrivateCreditError;
use crate::math::{amount_to_shares, checked_add, checked_sub, nav_per_share, shares_to_amount};
use crate::state::{Facility, ValuationMode, VaultExtension};

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq, Eq)]
pub struct ValuationSnapshot {
    pub gross_asset_value: u64,
    pub net_asset_value: u64,
    pub credit_loss_adjustment: u64,
    pub nav_per_share: u128,
    pub valuation_mode: ValuationMode,
    pub evaluated_at: i64,
}

/// Calculate Gross Asset Value (GAV) = Available Cash + Outstanding Principal + Accrued Interest + PIK Accrued.
pub fn calculate_gav(facility: &Facility) -> Result<u64> {
    let gav = checked_add(facility.available_liquidity, facility.outstanding_principal)?;
    let gav = checked_add(gav, facility.accrued_interest)?;
    let gav = checked_add(gav, facility.total_pik_accrued)?;
    Ok(gav)
}

/// Calculate Net Asset Value (NAV) = GAV - Credit Loss Adjustment.
pub fn calculate_nav(facility: &Facility, credit_loss_adjustment: u64) -> Result<u64> {
    let gav = calculate_gav(facility)?;
    Ok(gav.saturating_sub(credit_loss_adjustment))
}

/// Perform valuation update on VaultExtension.
pub fn update_vault_valuation(
    facility: &Facility,
    extension: &mut VaultExtension,
    total_shares: u64,
    credit_loss_adjustment: u64,
    now: i64,
) -> Result<ValuationSnapshot> {
    let gav = calculate_gav(facility)?;
    let nav = gav.saturating_sub(credit_loss_adjustment);

    let price = if total_shares == 0 || extension.valuation_mode == ValuationMode::FixedOneToOne {
        VALUATION_PRECISION
    } else {
        nav_per_share(nav, total_shares)?
    };

    extension.gross_asset_value = gav;
    extension.net_asset_value = nav;
    extension.credit_loss_adjustment = credit_loss_adjustment;
    extension.nav_per_share = price;
    extension.last_valuation_timestamp = now;
    extension.updated_at = now;

    Ok(ValuationSnapshot {
        gross_asset_value: gav,
        net_asset_value: nav,
        credit_loss_adjustment,
        nav_per_share: price,
        valuation_mode: extension.valuation_mode,
        evaluated_at: now,
    })
}

/// Convert deposit amount to shares based on current valuation mode.
pub fn deposit_amount_to_shares(
    amount: u64,
    extension_nav: u128,
    mode: ValuationMode,
) -> Result<u64> {
    if mode == ValuationMode::FixedOneToOne || extension_nav == 0 {
        Ok(amount)
    } else {
        amount_to_shares(amount, extension_nav)
    }
}

/// Convert share balance to deposit/redemption value based on valuation mode.
pub fn shares_to_value(
    shares: u64,
    extension_nav: u128,
    mode: ValuationMode,
) -> Result<u64> {
    if mode == ValuationMode::FixedOneToOne || extension_nav == 0 {
        Ok(shares)
    } else {
        shares_to_amount(shares, extension_nav)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gav_calculation() {
        let mut facility = Facility::default();
        facility.available_liquidity = 500_000;
        facility.outstanding_principal = 1_000_000;
        facility.accrued_interest = 50_000;
        facility.total_pik_accrued = 10_000;

        let gav = calculate_gav(&facility).unwrap();
        assert_eq!(gav, 1_560_000);
    }
}
