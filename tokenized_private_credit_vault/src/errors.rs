//! Custom Anchor errors. Existing variant ordering is preserved; new variants
//! are appended only.

use anchor_lang::prelude::*;

#[error_code]
pub enum PrivateCreditError {
    #[msg("Unauthorized")]
    Unauthorized,
    #[msg("Invalid facility status for this action")]
    InvalidFacilityStatus,
    #[msg("Facility is closed")]
    FacilityClosed,
    #[msg("Facility is defaulted")]
    FacilityDefaulted,
    #[msg("Amount must be greater than zero")]
    AmountMustBeGreaterThanZero,
    #[msg("Amount exceeds facility size or max drawdown")]
    ExceedsFacilitySize,
    #[msg("Amount exceeds available liquidity")]
    ExceedsAvailableLiquidity,
    #[msg("Amount exceeds investor balance")]
    ExceedsInvestorBalance,
    #[msg("Redemption has already been processed")]
    RedemptionAlreadyProcessed,
    #[msg("Redemption is frozen or unavailable")]
    RedemptionFrozen,
    #[msg("Redemption is not cancellable")]
    RedemptionNotCancellable,
    #[msg("Investor position is frozen")]
    InvestorPositionFrozen,
    #[msg("Invalid maturity timestamp")]
    InvalidMaturity,
    #[msg("Math overflow")]
    MathOverflow,
    #[msg("Covenant is already breached")]
    CovenantAlreadyBreached,
    #[msg("Cannot close facility")]
    CannotCloseFacility,
    #[msg("Invalid repayment amount")]
    InvalidRepaymentAmount,
    #[msg("Invalid event index")]
    InvalidEventIndex,
    #[msg("String too long")]
    StringTooLong,
    #[msg("Investor position was not found")]
    InvestorPositionNotFound,
    #[msg("Redemption request was not found")]
    RedemptionRequestNotFound,
    #[msg("Facility terms cannot be updated after this lifecycle point")]
    TermsCannotBeUpdated,
    // ---- Appended for the modular MVP expansion (codes 22+) ----
    #[msg("Invalid risk configuration")]
    InvalidRiskConfig,
    #[msg("Insufficient collateral coverage")]
    InsufficientCollateralCoverage,
    #[msg("Insufficient liquidity reserve")]
    InsufficientLiquidityReserve,
    #[msg("Invalid NAV calculation")]
    InvalidNavCalculation,
    #[msg("Invalid share valuation")]
    InvalidShareValuation,
    #[msg("Invalid repayment allocation")]
    InvalidRepaymentAllocation,
    #[msg("Payment is not yet due")]
    PaymentNotYetDue,
    #[msg("Duplicate missed payment for the current period")]
    DuplicateMissedPayment,
    #[msg("Invalid PIK capitalization")]
    InvalidPikCapitalization,
    #[msg("Invalid recovery state transition")]
    InvalidRecoveryTransition,
    #[msg("Investor is not authorized for this facility")]
    InvestorNotAuthorized,
    #[msg("Investor eligibility has expired")]
    InvestorEligibilityExpired,
    #[msg("Invalid redemption queue position")]
    InvalidRedemptionQueuePosition,
    #[msg("Valuation is stale")]
    StaleValuation,
    #[msg("Invalid accounting state")]
    InvalidAccountingState,
    #[msg("Outstanding obligations remain")]
    OutstandingObligations,
    #[msg("Unsupported token settlement mode")]
    UnsupportedSettlementMode,
    #[msg("Vault extension account is not initialized")]
    ExtensionNotInitialized,
    #[msg("Event type is not permitted for administrative logging")]
    InvalidEventTypeForAdminLog,
}
