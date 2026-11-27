//! Audit logging engine for facility lifecycle, transactions, and compliance.

use anchor_lang::prelude::*;
use crate::errors::PrivateCreditError;
use crate::math::checked_add;
use crate::state::{EventLog, EventType, Facility};

pub fn write_event_log(
    event_log: &mut Account<EventLog>,
    facility: &mut Account<Facility>,
    event_index: u64,
    event_type: EventType,
    actor: Pubkey,
    amount: u64,
    note_code: u16,
    bump: u8,
    now: i64,
) -> Result<()> {
    require!(event_index == facility.event_index, PrivateCreditError::InvalidEventIndex);
    event_log.facility = facility.key();
    event_log.event_index = event_index;
    event_log.event_type = event_type;
    event_log.actor = actor;
    event_log.timestamp = now;
    event_log.amount = amount;
    event_log.status_snapshot = facility.status;
    event_log.payment_status_snapshot = facility.payment_status;
    event_log.note_code = note_code;
    event_log.bump = bump;

    facility.event_index = checked_add(facility.event_index, 1)?;
    facility.updated_at = now;
    Ok(())
}

/// Full-width queue index accompanying the legacy u16 note code.
#[event]
pub struct RedemptionQueueEvidence {
    pub facility: Pubkey,
    pub event_index: u64,
    pub queue_index: u64,
}
