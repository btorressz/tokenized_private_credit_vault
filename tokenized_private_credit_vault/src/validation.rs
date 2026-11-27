//! Authorization, lifecycle, and safety invariant guards.

use anchor_lang::prelude::*;
use crate::errors::PrivateCreditError;
use crate::state::{Facility, FacilityStatus, InvestorPosition, InvestorPositionStatus};

pub fn require_operator(facility: &Facility, actor: Pubkey) -> Result<()> {
    require_keys_eq!(facility.operator, actor, PrivateCreditError::Unauthorized);
    Ok(())
}

pub fn require_borrower(facility: &Facility, actor: Pubkey) -> Result<()> {
    require_keys_eq!(facility.borrower, actor, PrivateCreditError::Unauthorized);
    Ok(())
}

pub fn require_borrower_or_operator(facility: &Facility, actor: Pubkey) -> Result<()> {
    require!(
        actor == facility.borrower || actor == facility.operator,
        PrivateCreditError::Unauthorized
    );
    Ok(())
}

pub fn require_not_defaulted_or_closed(facility: &Facility) -> Result<()> {
    require!(
        facility.status != FacilityStatus::Defaulted,
        PrivateCreditError::FacilityDefaulted
    );
    require!(
        facility.status != FacilityStatus::Closed,
        PrivateCreditError::FacilityClosed
    );
    Ok(())
}

pub fn require_deposits_allowed(facility: &Facility) -> Result<()> {
    require_not_defaulted_or_closed(facility)?;
    require!(
        matches!(
            facility.status,
            FacilityStatus::Initialized | FacilityStatus::Active
        ),
        PrivateCreditError::InvalidFacilityStatus
    );
    Ok(())
}

pub fn require_drawdowns_allowed(facility: &Facility) -> Result<()> {
    require_not_defaulted_or_closed(facility)?;
    require!(
        matches!(
            facility.status,
            FacilityStatus::Active | FacilityStatus::Drawn | FacilityStatus::Performing
        ),
        PrivateCreditError::InvalidFacilityStatus
    );
    Ok(())
}

pub fn require_redemptions_allowed(facility: &Facility) -> Result<()> {
    require_not_defaulted_or_closed(facility)?;
    require!(
        facility.status != FacilityStatus::Matured || facility.available_liquidity > 0,
        PrivateCreditError::ExceedsAvailableLiquidity
    );
    Ok(())
}

pub fn require_investor_active(position: &InvestorPosition) -> Result<()> {
    require!(
        position.status != InvestorPositionStatus::Frozen,
        PrivateCreditError::InvestorPositionFrozen
    );
    Ok(())
}
