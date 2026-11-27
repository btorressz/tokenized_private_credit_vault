//! On-chain account layouts and enums.
//!
//! Compatibility: the six original accounts and six enums are byte-for-byte
//! identical to the pre-refactor monolith, so serialized accounts (and the
//! IDL shape of their fields) are unchanged. New persistent state lives in
//! *extension PDAs* (VaultExtension, InvestorEligibility), never by resizing
//! the original accounts.

use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct Facility {
    #[max_len(32)]
    pub facility_id: String,
    pub operator: Pubkey,
    pub borrower: Pubkey,
    pub stablecoin_mint: Pubkey,
    pub facility_size: u64,
    pub interest_rate_bps: u16,
    pub maturity_timestamp: i64,
    pub max_drawdown_amount: u64,
    pub minimum_collateral_coverage_bps: u64,
    pub minimum_liquidity_reserve: u64,
    pub collateral_description_hash: [u8; 32],
    pub status: FacilityStatus,
    pub payment_status: PaymentStatus,
    pub collateral_status: CollateralStatus,
    pub total_deposited: u64,
    pub total_drawn: u64,
    pub total_repaid: u64,
    pub available_liquidity: u64,
    pub outstanding_principal: u64,
    pub accrued_interest: u64,
    pub last_interest_accrual_timestamp: i64,
    pub missed_payment_count: u16,
    pub total_pending_redemptions: u64,
    pub redemption_queue_index: u64,
    pub pik_enabled: bool,
    pub total_pik_accrued: u64,
    pub last_pik_timestamp: i64,
    pub default_reason_code: u16,
    pub event_index: u64,
    pub created_at: i64,
    pub updated_at: i64,
    pub bump: u8,
}

#[account]
#[derive(InitSpace)]
pub struct InvestorPosition {
    pub facility: Pubkey,
    pub investor: Pubkey,
    pub principal: u64,
    pub share_balance: u64,
    pub deposited_amount: u64,
    pub redeemed_amount: u64,
    pub pending_redemption_amount: u64,
    pub status: InvestorPositionStatus,
    pub created_at: i64,
    pub updated_at: i64,
    pub bump: u8,
}

#[account]
#[derive(InitSpace)]
pub struct RepaymentSchedule {
    pub facility: Pubkey,
    pub expected_total_repayment: u64,
    pub next_due_timestamp: i64,
    pub scheduled_payment_amount: u64,
    pub last_payment_timestamp: i64,
    pub last_repayment_amount: u64,
    pub last_principal_portion: u64,
    pub last_interest_portion: u64,
    pub payments_made: u32,
    pub scheduled_payments_satisfied: u32,
    pub status: PaymentStatus,
    pub created_at: i64,
    pub updated_at: i64,
    pub bump: u8,
}

#[account]
#[derive(InitSpace)]
pub struct CovenantState {
    pub facility: Pubkey,
    pub debt_service_coverage_ratio_bps: u64,
    pub loan_to_value_bps: u64,
    pub minimum_liquidity_bps: u64,
    pub reporting_current: bool,
    pub last_report_timestamp: i64,
    pub breached: bool,
    #[max_len(96)]
    pub breach_reason: String,
    pub collateral_value: u64,
    pub collateral_status: CollateralStatus,
    pub last_valuation_timestamp: i64,
    pub collateral_impaired: bool,
    pub updated_by: Pubkey,
    pub created_at: i64,
    pub updated_at: i64,
    pub bump: u8,
}

#[account]
#[derive(InitSpace)]
pub struct RedemptionRequest {
    pub facility: Pubkey,
    pub investor: Pubkey,
    pub requested_amount: u64,
    pub processed_amount: u64,
    pub timestamp: i64,
    pub processed_timestamp: i64,
    pub status: RedemptionStatus,
    pub queue_index: u64,
    pub bump: u8,
}

#[account]
#[derive(InitSpace)]
pub struct EventLog {
    pub facility: Pubkey,
    pub event_index: u64,
    pub event_type: EventType,
    pub actor: Pubkey,
    pub timestamp: i64,
    pub amount: u64,
    pub status_snapshot: FacilityStatus,
    pub payment_status_snapshot: PaymentStatus,
    pub note_code: u16,
    pub bump: u8,
}

/// Extension PDA seeded `[b"extension", facility.key()]`.
/// Holds risk/valuation/waterfall/recovery/compliance configuration and state
/// so the original account layouts stay unchanged.
#[account]
#[derive(InitSpace)]
pub struct VaultExtension {
    pub facility: Pubkey,
    /// When true, deposits require an InvestorEligibility record.
    pub permissioned_mode: bool,
    /// Servicing fee for the waterfall, in bps of each payment.
    pub servicing_fee_bps: u64,
    /// FIFO pointer: the lowest unprocessed redemption queue index.
    pub fifo_head: u64,
    pub grace_period_seconds: i64,
    // ---- Valuation (fixed 1:1 mode until explicitly migrated) ----
    pub valuation_mode: ValuationMode,
    /// NAV per share, in VALUATION_PRECISION fixed point.
    pub nav_per_share: u128,
    pub gross_asset_value: u64,
    pub net_asset_value: u64,
    pub credit_loss_adjustment: u64,
    pub last_valuation_timestamp: i64,
    // ---- Recovery ----
    pub recovery_status: RecoveryStatus,
    pub default_timestamp: i64,
    pub obligations_at_default_principal: u64,
    pub obligations_at_default_interest: u64,
    pub recovered_proceeds: u64,
    pub principal_written_down: u64,
    pub interest_written_down: u64,
    pub created_at: i64,
    pub updated_at: i64,
    pub bump: u8,
}

/// Per-investor eligibility record for permissioned facilities,
/// seeded `[b"eligibility", facility.key(), investor.key()]`.
#[account]
#[derive(InitSpace)]
pub struct InvestorEligibility {
    pub facility: Pubkey,
    pub investor: Pubkey,
    pub eligible: bool,
    /// 0 = no expiry.
    pub expires_at: i64,
    pub reason_code: u16,
    pub updated_by: Pubkey,
    pub updated_at: i64,
    pub bump: u8,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, InitSpace)]
pub enum FacilityStatus {
    Initialized,
    Active,
    Drawn,
    Performing,
    Breached,
    Defaulted,
    Matured,
    Closed,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, InitSpace)]
pub enum PaymentStatus {
    Current,
    Due,
    Late,
    Missed,
    Defaulted,
    Repaid,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, InitSpace)]
pub enum InvestorPositionStatus {
    Active,
    RedemptionPending,
    PartiallyRedeemed,
    Redeemed,
    Frozen,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, InitSpace)]
pub enum RedemptionStatus {
    Requested,
    Queued,
    PartiallyProcessed,
    Processed,
    Cancelled,
    Frozen,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, InitSpace)]
pub enum CollateralStatus {
    Unknown,
    Healthy,
    Watchlist,
    Impaired,
    LiquidationPending,
    Liquidated,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, InitSpace, Debug)]
pub enum ValuationMode {
    /// Legacy fixed 1:1 simulated accounting (default; unchanged behavior).
    FixedOneToOne,
    /// NAV-based share pricing; only after an explicit migration instruction.
    NavBased,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, InitSpace)]
pub enum RecoveryStatus {
    None,
    InProgress,
    Restructured,
    Completed,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, InitSpace)]
pub enum EventType {
    FacilityInitialized,
    FacilityActivated,
    CapitalDeposited,
    BorrowerDrawdown,
    InterestAccrued,
    RepaymentMade,
    PaymentMissed,
    CovenantUpdated,
    CovenantBreached,
    PikToggled,
    PikCapitalized,
    CollateralUpdated,
    RedemptionRequested,
    RedemptionCancelled,
    RedemptionProcessed,
    InvestorFrozen,
    FacilityMatured,
    FacilityDefaulted,
    TermsUpdated,
    FacilityClosed,
    // Appended:
    InvestorUnfrozen,
    RiskAssessed,
    ValuationUpdated,
    RecoveryProceedsRecorded,
    DebtWrittenDown,
    FacilityRestructured,
    RecoveryCompleted,
    EligibilityUpdated,
    WaterfallApplied,
    AdministrativeNote,
}

/// Deterministic risk classification (heuristic, not a statistical PD).
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum RiskClass {
    InsufficientData,
    Healthy,
    Low,
    Moderate,
    Elevated,
    High,
    Critical,
    Defaulted,
}

impl RiskClass {
    pub fn from_score(score: u128, defaulted: bool) -> Self {
        if defaulted {
            return RiskClass::Defaulted;
        }
        match score {
            0..=9 => RiskClass::Healthy,
            10..=29 => RiskClass::Low,
            30..=49 => RiskClass::Moderate,
            50..=69 => RiskClass::Elevated,
            70..=89 => RiskClass::High,
            _ => RiskClass::Critical,
        }
    }
}

// Test-only fixture; not an on-chain initializer.
#[cfg(test)]
impl Default for Facility {
    fn default() -> Self {
        Self {
            facility_id: String::new(),
            operator: Pubkey::default(),
            borrower: Pubkey::default(),
            stablecoin_mint: Pubkey::default(),
            facility_size: 0,
            interest_rate_bps: 0,
            maturity_timestamp: 0,
            max_drawdown_amount: 0,
            minimum_collateral_coverage_bps: 0,
            minimum_liquidity_reserve: 0,
            collateral_description_hash: [0; 32],
            status: FacilityStatus::Initialized,
            payment_status: PaymentStatus::Current,
            collateral_status: CollateralStatus::Unknown,
            total_deposited: 0,
            total_drawn: 0,
            total_repaid: 0,
            available_liquidity: 0,
            outstanding_principal: 0,
            accrued_interest: 0,
            last_interest_accrual_timestamp: 0,
            missed_payment_count: 0,
            total_pending_redemptions: 0,
            redemption_queue_index: 0,
            pik_enabled: false,
            total_pik_accrued: 0,
            last_pik_timestamp: 0,
            default_reason_code: 0,
            event_index: 0,
            created_at: 0,
            updated_at: 0,
            bump: 0,
        }
    }
}
