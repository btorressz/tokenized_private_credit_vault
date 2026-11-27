//! Investor redemption queue engine and position freeze management.

use anchor_lang::prelude::*;
use crate::errors::PrivateCreditError;
use crate::events::write_event_log;
use crate::math::{checked_add, checked_sub};
use crate::pda::{
    load_existing_investor_position, load_existing_redemption_request,
    load_or_init_redemption_request, save_investor_position, save_redemption_request,
};
use crate::state::{
    EventLog, EventType, Facility, InvestorPositionStatus, RedemptionStatus,
};
use crate::validation::{
    require_not_defaulted_or_closed, require_operator, require_redemptions_allowed,
};

pub fn process_request_redemption<'info>(
    facility: &mut Account<Facility>,
    investor_position_info: &AccountInfo<'info>,
    redemption_request_info: &AccountInfo<'info>,
    investor_info: &AccountInfo<'info>,
    system_program_info: &AccountInfo<'info>,
    event_log: &mut Account<EventLog>,
    amount: u64,
    event_index: u64,
    request_bump: u8,
    event_bump: u8,
    now: i64,
) -> Result<()> {
    require!(amount > 0, PrivateCreditError::AmountMustBeGreaterThanZero);
    require_redemptions_allowed(facility)?;

    let facility_key = facility.key();
    let investor_key = investor_info.key();

    let mut position = load_existing_investor_position(investor_position_info)?;
    require_keys_eq!(position.facility, facility_key, PrivateCreditError::Unauthorized);
    require_keys_eq!(position.investor, investor_key, PrivateCreditError::Unauthorized);
    require!(
        position.status != InvestorPositionStatus::Frozen,
        PrivateCreditError::InvestorPositionFrozen
    );

    let redeemable_balance = checked_sub(position.share_balance, position.pending_redemption_amount)?;
    require!(amount <= redeemable_balance, PrivateCreditError::ExceedsInvestorBalance);

    let queue_index = facility.redemption_queue_index;
    let (mut request, created) = load_or_init_redemption_request(
        redemption_request_info,
        investor_info,
        system_program_info,
        &facility_key,
        &investor_key,
        request_bump,
        queue_index,
        now,
    )?;

    require!(
        request.status != RedemptionStatus::Frozen,
        PrivateCreditError::RedemptionFrozen
    );

    let mut should_increment_queue = created;
    if !created && matches!(request.status, RedemptionStatus::Cancelled | RedemptionStatus::Processed) {
        let remaining = checked_sub(request.requested_amount, request.processed_amount)?;
        require!(remaining == 0, PrivateCreditError::RedemptionNotCancellable);
        request.requested_amount = 0;
        request.processed_amount = 0;
        request.timestamp = now;
        request.processed_timestamp = 0;
        request.queue_index = queue_index;
        should_increment_queue = true;
    }

    request.requested_amount = checked_add(request.requested_amount, amount)?;
    request.timestamp = now;
    request.status = RedemptionStatus::Queued;

    position.pending_redemption_amount = checked_add(position.pending_redemption_amount, amount)?;
    position.status = InvestorPositionStatus::RedemptionPending;
    position.updated_at = now;

    save_investor_position(investor_position_info, &position)?;
    save_redemption_request(redemption_request_info, &request)?;

    facility.total_pending_redemptions = checked_add(facility.total_pending_redemptions, amount)?;
    if should_increment_queue {
        facility.redemption_queue_index = checked_add(facility.redemption_queue_index, 1)?;
    }

    emit!(crate::events::RedemptionQueueEvidence { facility: facility.key(), event_index, queue_index: request.queue_index });
    write_event_log(
        event_log,
        facility,
        event_index,
        EventType::RedemptionRequested,
        investor_key,
        amount,
        u16::try_from(request.queue_index).unwrap_or(u16::MAX),
        event_bump,
        now,
    )?;
    Ok(())
}

pub fn process_cancel_redemption<'info>(
    facility: &mut Account<Facility>,
    investor_position_info: &AccountInfo<'info>,
    redemption_request_info: &AccountInfo<'info>,
    investor_key: Pubkey,
    event_log: &mut Account<EventLog>,
    event_index: u64,
    event_bump: u8,
    now: i64,
) -> Result<()> {
    require_not_defaulted_or_closed(facility)?;

    let facility_key = facility.key();
    let mut position = load_existing_investor_position(investor_position_info)?;
    let mut request = load_existing_redemption_request(redemption_request_info)?;

    require_keys_eq!(position.facility, facility_key, PrivateCreditError::Unauthorized);
    require_keys_eq!(position.investor, investor_key, PrivateCreditError::Unauthorized);
    require_keys_eq!(request.facility, facility_key, PrivateCreditError::Unauthorized);
    require_keys_eq!(request.investor, investor_key, PrivateCreditError::Unauthorized);
    require!(
        position.status != InvestorPositionStatus::Frozen,
        PrivateCreditError::InvestorPositionFrozen
    );
    require!(
        matches!(
            request.status,
            RedemptionStatus::Requested | RedemptionStatus::Queued | RedemptionStatus::PartiallyProcessed
        ),
        PrivateCreditError::RedemptionNotCancellable
    );

    let remaining_amount = checked_sub(request.requested_amount, request.processed_amount)?;
    require!(remaining_amount > 0, PrivateCreditError::RedemptionNotCancellable);
    require!(remaining_amount <= position.pending_redemption_amount, PrivateCreditError::ExceedsInvestorBalance);
    require!(remaining_amount <= facility.total_pending_redemptions, PrivateCreditError::ExceedsInvestorBalance);

    position.pending_redemption_amount = checked_sub(position.pending_redemption_amount, remaining_amount)?;
    if position.pending_redemption_amount > 0 {
        position.status = InvestorPositionStatus::RedemptionPending;
    } else if position.share_balance == 0 {
        position.status = InvestorPositionStatus::Redeemed;
    } else if position.redeemed_amount > 0 {
        position.status = InvestorPositionStatus::PartiallyRedeemed;
    } else {
        position.status = InvestorPositionStatus::Active;
    }
    position.updated_at = now;

    request.requested_amount = request.processed_amount;
    request.status = RedemptionStatus::Cancelled;
    request.timestamp = now;

    save_investor_position(investor_position_info, &position)?;
    save_redemption_request(redemption_request_info, &request)?;

    facility.total_pending_redemptions = checked_sub(facility.total_pending_redemptions, remaining_amount)?;

    emit!(crate::events::RedemptionQueueEvidence { facility: facility.key(), event_index, queue_index: request.queue_index });
    write_event_log(
        event_log,
        facility,
        event_index,
        EventType::RedemptionCancelled,
        investor_key,
        remaining_amount,
        u16::try_from(request.queue_index).unwrap_or(u16::MAX),
        event_bump,
        now,
    )?;
    Ok(())
}

pub fn process_process_redemption<'info>(
    facility: &mut Account<Facility>,
    investor_position_info: &AccountInfo<'info>,
    redemption_request_info: &AccountInfo<'info>,
    operator_key: Pubkey,
    investor_key: Pubkey,
    event_log: &mut Account<EventLog>,
    amount: u64,
    event_index: u64,
    event_bump: u8,
    now: i64,
) -> Result<()> {
    require_operator(facility, operator_key)?;
    require!(amount > 0, PrivateCreditError::AmountMustBeGreaterThanZero);
    require_redemptions_allowed(facility)?;

    let facility_key = facility.key();
    let mut position = load_existing_investor_position(investor_position_info)?;
    let mut request = load_existing_redemption_request(redemption_request_info)?;

    require_keys_eq!(position.facility, facility_key, PrivateCreditError::Unauthorized);
    require_keys_eq!(position.investor, investor_key, PrivateCreditError::Unauthorized);
    require_keys_eq!(request.facility, facility_key, PrivateCreditError::Unauthorized);
    require_keys_eq!(request.investor, investor_key, PrivateCreditError::Unauthorized);
    require!(
        position.status != InvestorPositionStatus::Frozen,
        PrivateCreditError::InvestorPositionFrozen
    );
    require!(
        request.status != RedemptionStatus::Processed,
        PrivateCreditError::RedemptionAlreadyProcessed
    );
    require!(
        request.status != RedemptionStatus::Frozen,
        PrivateCreditError::RedemptionFrozen
    );
    require!(
        request.status != RedemptionStatus::Cancelled,
        PrivateCreditError::RedemptionFrozen
    );

    let remaining_request = checked_sub(request.requested_amount, request.processed_amount)?;
    require!(amount <= remaining_request, PrivateCreditError::ExceedsInvestorBalance);
    require!(amount <= facility.available_liquidity, PrivateCreditError::ExceedsAvailableLiquidity);

    let reserve = if facility.status == crate::state::FacilityStatus::Matured && facility.outstanding_principal == 0 && facility.accrued_interest == 0 && facility.total_pik_accrued == 0 { 0 } else { facility.minimum_liquidity_reserve };
    let post_redemption_liquidity = crate::token_vault::cash_after(facility.available_liquidity, amount, crate::token_vault::SettlementKind::InvestorRedemption, reserve)?;
    require!(amount <= position.share_balance, PrivateCreditError::ExceedsInvestorBalance);
    require!(amount <= position.pending_redemption_amount, PrivateCreditError::ExceedsInvestorBalance);

    position.share_balance = checked_sub(position.share_balance, amount)?;
    position.principal = checked_sub(position.principal, amount)?;
    position.redeemed_amount = checked_add(position.redeemed_amount, amount)?;
    position.pending_redemption_amount = checked_sub(position.pending_redemption_amount, amount)?;
    position.updated_at = now;

    if position.share_balance == 0 {
        position.status = InvestorPositionStatus::Redeemed;
    } else if position.pending_redemption_amount > 0 {
        position.status = InvestorPositionStatus::RedemptionPending;
    } else {
        position.status = InvestorPositionStatus::PartiallyRedeemed;
    }

    request.processed_amount = checked_add(request.processed_amount, amount)?;
    request.processed_timestamp = now;

    if request.processed_amount == request.requested_amount {
        request.status = RedemptionStatus::Processed;
    } else {
        request.status = RedemptionStatus::PartiallyProcessed;
    }

    save_investor_position(investor_position_info, &position)?;
    save_redemption_request(redemption_request_info, &request)?;

    facility.available_liquidity = post_redemption_liquidity;
    facility.total_pending_redemptions = checked_sub(facility.total_pending_redemptions, amount)?;

    emit!(crate::events::RedemptionQueueEvidence { facility: facility.key(), event_index, queue_index: request.queue_index });
    write_event_log(
        event_log,
        facility,
        event_index,
        EventType::RedemptionProcessed,
        operator_key,
        amount,
        u16::try_from(request.queue_index).unwrap_or(u16::MAX),
        event_bump,
        now,
    )?;
    Ok(())
}

pub fn process_freeze_investor_position<'info>(
    facility: &mut Account<Facility>,
    investor_position_info: &AccountInfo<'info>,
    redemption_request_info: &AccountInfo<'info>,
    operator_key: Pubkey,
    investor_key: Pubkey,
    event_log: &mut Account<EventLog>,
    event_index: u64,
    event_bump: u8,
    now: i64,
) -> Result<()> {
    require_operator(facility, operator_key)?;
    let facility_key = facility.key();

    let mut position = load_existing_investor_position(investor_position_info)?;
    require_keys_eq!(position.facility, facility_key, PrivateCreditError::Unauthorized);
    require_keys_eq!(position.investor, investor_key, PrivateCreditError::Unauthorized);

    position.status = InvestorPositionStatus::Frozen;
    position.updated_at = now;
    save_investor_position(investor_position_info, &position)?;

    if !redemption_request_info.data_is_empty() {
        let mut request = load_existing_redemption_request(redemption_request_info)?;
        require_keys_eq!(request.facility, facility_key, PrivateCreditError::Unauthorized);
        require_keys_eq!(request.investor, investor_key, PrivateCreditError::Unauthorized);
        if matches!(
            request.status,
            RedemptionStatus::Requested | RedemptionStatus::Queued | RedemptionStatus::PartiallyProcessed
        ) {
            request.status = RedemptionStatus::Frozen;
            save_redemption_request(redemption_request_info, &request)?;
        }
    }

    write_event_log(
        event_log,
        facility,
        event_index,
        EventType::InvestorFrozen,
        operator_key,
        position.pending_redemption_amount,
        0,
        event_bump,
        now,
    )?;
    Ok(())
}
