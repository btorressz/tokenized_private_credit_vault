//! Simulated settlement only. These pure calculations never transfer tokens,
//! create a second cash ledger, or mutate accounts. Instruction handlers own
//! the single authoritative balance update.
use anchor_lang::prelude::*;
use crate::errors::PrivateCreditError;
use crate::math::{checked_add, checked_sub};

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettlementMode { SimulatedAccounting, FutureSplTokenSettlement }
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettlementKind { CapitalDeposit, BorrowerDrawdown, BorrowerRepayment, InvestorRedemption, RecoveryProceeds, Distribution }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SettlementResult {
    pub kind: SettlementKind,
    pub amount: u64,
    pub previous_cash: u64,
    pub next_cash: u64,
}

pub fn cash_after(cash: u64, amount: u64, kind: SettlementKind, reserve: u64) -> Result<u64> {
    require!(amount > 0, PrivateCreditError::AmountMustBeGreaterThanZero);
    match kind {
        SettlementKind::CapitalDeposit | SettlementKind::BorrowerRepayment | SettlementKind::RecoveryProceeds => checked_add(cash, amount),
        SettlementKind::BorrowerDrawdown | SettlementKind::InvestorRedemption | SettlementKind::Distribution => {
            require!(amount <= cash, PrivateCreditError::ExceedsAvailableLiquidity);
            let next = checked_sub(cash, amount)?;
            require!(next >= reserve, PrivateCreditError::InsufficientLiquidityReserve);
            Ok(next)
        }
    }
}

/// Mint keys identify simulated denominations, not verified SPL mint accounts.
pub fn preview_settlement(
    mode: SettlementMode, configured_mint: Pubkey, requested_mint: Pubkey,
    cash: u64, amount: u64, kind: SettlementKind, reserve: u64,
) -> Result<SettlementResult> {
    require!(mode == SettlementMode::SimulatedAccounting, PrivateCreditError::UnsupportedSettlementMode);
    require_keys_eq!(configured_mint, requested_mint, PrivateCreditError::Unauthorized);
    let next_cash = cash_after(cash, amount, kind, reserve)?;
    Ok(SettlementResult { kind, amount, previous_cash: cash, next_cash })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cash_conservation_and_reserves() {
        let cash = cash_after(0, 100, SettlementKind::CapitalDeposit, 0).unwrap();
        let cash = cash_after(cash, 60, SettlementKind::BorrowerDrawdown, 40).unwrap();
        assert_eq!(cash, 40);
        assert!(cash_after(cash, 1, SettlementKind::InvestorRedemption, 40).is_err());
        assert_eq!(cash_after(cash, 60, SettlementKind::BorrowerRepayment, 0).unwrap(), 100);
        assert!(cash_after(u64::MAX, 1, SettlementKind::CapitalDeposit, 0).is_err());
        assert!(cash_after(0, 1, SettlementKind::Distribution, 0).is_err());
        assert!(cash_after(0, 0, SettlementKind::CapitalDeposit, 0).is_err());
    }
    #[test]
    fn rejects_real_settlement_and_wrong_mint() {
        let mint = Pubkey::new_unique();
        assert!(preview_settlement(SettlementMode::FutureSplTokenSettlement, mint, mint, 0, 1, SettlementKind::CapitalDeposit, 0).is_err());
        assert!(preview_settlement(SettlementMode::SimulatedAccounting, mint, Pubkey::new_unique(), 0, 1, SettlementKind::CapitalDeposit, 0).is_err());
    }
}
