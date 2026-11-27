//! Simulated investor permission policy. No identity data or actual KYC/AML.
//! Permissioned facility activation is deferred until all instruction contexts
//! require the canonical extension and eligibility PDAs (no optional bypass).
use anchor_lang::prelude::*;
use crate::errors::PrivateCreditError;
use crate::state::{InvestorEligibility, InvestorPosition, InvestorPositionStatus, RedemptionRequest, RedemptionStatus, VaultExtension};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PermissionAction { Deposit, RequestRedemption, ProcessRedemption }

/// Caller must supply program-owned, canonical PDA records. A missing extension
/// is NOT a substitute for proving that a facility is open.
pub fn require_permission(
    facility: Pubkey,
    investor: Pubkey,
    extension: &VaultExtension,
    eligibility: Option<&InvestorEligibility>,
    now: i64,
    _action: PermissionAction,
) -> Result<()> {
    require_keys_eq!(extension.facility, facility, PrivateCreditError::Unauthorized);
    if !extension.permissioned_mode { return Ok(()); }
    let record = eligibility.ok_or_else(|| error!(PrivateCreditError::InvestorNotAuthorized))?;
    require_keys_eq!(record.facility, facility, PrivateCreditError::Unauthorized);
    require_keys_eq!(record.investor, investor, PrivateCreditError::Unauthorized);
    require!(record.eligible, PrivateCreditError::InvestorNotAuthorized);
    require!(record.expires_at == 0 || record.expires_at > now, PrivateCreditError::InvestorEligibilityExpired);
    Ok(())
}

/// Restore position status without changing financial claims or queue index.
/// Only an authenticated operator instruction may call this.
pub fn restore_position(
    position: &mut InvestorPosition,
    request: Option<&mut RedemptionRequest>,
    now: i64,
) -> Result<()> {
    require!(position.status == InvestorPositionStatus::Frozen, PrivateCreditError::InvalidAccountingState);
    if position.pending_redemption_amount > 0 {
        let request = request.ok_or_else(|| error!(PrivateCreditError::RedemptionRequestNotFound))?;
        require_keys_eq!(request.facility, position.facility, PrivateCreditError::Unauthorized);
        require_keys_eq!(request.investor, position.investor, PrivateCreditError::Unauthorized);
        require!(request.status == RedemptionStatus::Frozen, PrivateCreditError::RedemptionFrozen);
        let remaining = crate::math::checked_sub(request.requested_amount, request.processed_amount)?;
        require!(remaining == position.pending_redemption_amount && remaining <= position.share_balance, PrivateCreditError::InvalidAccountingState);
        request.status = if request.processed_amount > 0 { RedemptionStatus::PartiallyProcessed } else { RedemptionStatus::Queued };
        position.status = InvestorPositionStatus::RedemptionPending;
    } else {
        position.status = if position.share_balance == 0 { InvestorPositionStatus::Redeemed }
            else if position.redeemed_amount > 0 { InvestorPositionStatus::PartiallyRedeemed }
            else { InvestorPositionStatus::Active };
    }
    position.updated_at = now;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn position() -> InvestorPosition {
        InvestorPosition { facility: Pubkey::new_unique(), investor: Pubkey::new_unique(), principal: 10, share_balance: 10,
            deposited_amount: 10, redeemed_amount: 0, pending_redemption_amount: 0,
            status: InvestorPositionStatus::Frozen, created_at: 1, updated_at: 1, bump: 1 }
    }
    #[test]
    fn restores_without_changing_balances() {
        let mut p = position();
        restore_position(&mut p, None, 2).unwrap();
        assert!(p.status == InvestorPositionStatus::Active);
        assert_eq!(p.share_balance, 10);
        assert!(restore_position(&mut p, None, 3).is_err());
    }
    #[test]
    fn missing_pending_request_fails_closed() {
        let mut p = position();
        p.pending_redemption_amount = 5;
        assert!(restore_position(&mut p, None, 2).is_err());
        assert!(p.status == InvestorPositionStatus::Frozen);
    }
}
