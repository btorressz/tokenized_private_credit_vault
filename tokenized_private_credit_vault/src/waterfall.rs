//! Payment Waterfall Allocation Engine.
//!
//! Allocates borrower repayment proceeds across servicing fees, accrued interest,
//! principal reduction, and residual cash, ensuring exact conservation of funds.

use anchor_lang::prelude::*;
use crate::errors::PrivateCreditError;
use crate::constants::MAX_FEE_BPS;
use crate::math::{bps_of, checked_add, checked_sub};
use crate::state::Facility;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq, Eq)]
pub struct WaterfallAllocation {
    pub total_payment: u64,
    pub servicing_fee_amount: u64,
    pub interest_paid: u64,
    pub principal_paid: u64,
    pub residual_amount: u64,
}

/// Calculate payment allocation priority:
/// 1. Servicing fees (servicing_fee_bps of total payment)
/// 2. Accrued interest
/// 3. Outstanding principal
/// 4. Residual cash
pub fn calculate_repayment_waterfall(
    facility: &Facility,
    payment_amount: u64,
    servicing_fee_bps: u64,
) -> Result<WaterfallAllocation> {
    require!(payment_amount > 0, PrivateCreditError::AmountMustBeGreaterThanZero);

    require!(servicing_fee_bps <= MAX_FEE_BPS, PrivateCreditError::InvalidRepaymentAllocation);
    let fee_amount = if servicing_fee_bps > 0 {
        bps_of(payment_amount, servicing_fee_bps)?
    } else {
        0
    };

    let remaining_after_fee = checked_sub(payment_amount, fee_amount)?;

    let interest_paid = remaining_after_fee.min(facility.accrued_interest);
    let remaining_after_interest = checked_sub(remaining_after_fee, interest_paid)?;

    let principal_paid = remaining_after_interest.min(facility.outstanding_principal);
    let residual_amount = checked_sub(remaining_after_interest, principal_paid)?;

    // Invariant check: total allocated must equal total_payment
    let allocated_sum = checked_add(fee_amount, interest_paid)?;
    let allocated_sum = checked_add(allocated_sum, principal_paid)?;
    let allocated_sum = checked_add(allocated_sum, residual_amount)?;
    require_eq!(allocated_sum, payment_amount, PrivateCreditError::InvalidRepaymentAllocation);

    Ok(WaterfallAllocation {
        total_payment: payment_amount,
        servicing_fee_amount: fee_amount,
        interest_paid,
        principal_paid,
        residual_amount,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_waterfall_allocation() {
        let mut facility = Facility::default();
        facility.accrued_interest = 20_000;
        facility.outstanding_principal = 100_000;

        let alloc = calculate_repayment_waterfall(&facility, 50_000, 100).unwrap(); // 1% fee = 500
        assert_eq!(alloc.servicing_fee_amount, 500);
        assert_eq!(alloc.interest_paid, 20_000);
        assert_eq!(alloc.principal_paid, 29_500);
        assert_eq!(alloc.residual_amount, 0);
        assert_eq!(alloc.total_payment, 50_000);
    }
}
