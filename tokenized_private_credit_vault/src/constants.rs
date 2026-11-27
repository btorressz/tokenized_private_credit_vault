//! Protocol-wide constants. Facility-level terms (sizes, rates, maturities)
//! remain configurable per facility; these are global, non-configurable bounds.

/// Maximum length of a facility identifier string.
pub const MAX_FACILITY_ID_LEN: usize = 32;
/// Maximum length of a covenant breach reason string.
pub const MAX_COVENANT_REASON_LEN: usize = 96;
/// Number of consecutive missed payments before automatic default.
pub const MISSED_PAYMENT_DEFAULT_THRESHOLD: u16 = 2;
/// Anchor account discriminator size.
pub const ACCOUNT_DISCRIMINATOR_SIZE: usize = 8;
/// Default payment period: 30 days in seconds.
pub const THIRTY_DAYS_SECONDS: i64 = 2_592_000;
/// Seconds in a 365-day year (actuarial denominator).
pub const YEAR_SECONDS: i64 = 31_536_000;
/// Basis-point denominator (10,000 bps = 100%).
pub const BPS_DENOMINATOR: u128 = 10_000;
/// Default grace period after a due date before a payment is "missed": 5 days.
pub const DEFAULT_GRACE_PERIOD_SECONDS: i64 = 432_000;
/// A collateral valuation older than this is stale: 7 days.
pub const STALE_VALUATION_SECONDS: i64 = 604_800;
/// A financial report older than this is not "current": 45 days.
pub const REPORTING_DEADLINE_SECONDS: i64 = 3_888_000;
/// Maximum servicing fee accepted by the waterfall: 20%.
pub const MAX_FEE_BPS: u64 = 2_000;
/// Maximum annual interest rate: 50%.
pub const MAX_INTEREST_RATE_BPS: u16 = 5_000;
/// Maximum collateral coverage requirement: 10,000%.
pub const MAX_COVERAGE_BPS: u64 = 1_000_000;
/// Fixed-point precision for NAV/share valuation (1e9).
pub const VALUATION_PRECISION: u128 = 1_000_000_000;
/// Risk score scale: scores are 0..=100.
pub const RISK_SCALE: u128 = 100;
