//! Manual PDA account creation, deserialization, serialization, and security checks.

use anchor_lang::prelude::*;
use anchor_lang::solana_program::{program::invoke_signed, system_instruction};
use std::io::Cursor;
use crate::constants::ACCOUNT_DISCRIMINATOR_SIZE;
use crate::errors::PrivateCreditError;
use crate::state::{InvestorPosition, InvestorPositionStatus, RedemptionRequest, RedemptionStatus};

pub fn create_pda_account<'info>(
    account_info: &AccountInfo<'info>,
    payer_info: &AccountInfo<'info>,
    system_program_info: &AccountInfo<'info>,
    space: usize,
    signer_seeds: &[&[u8]],
) -> Result<()> {
    if account_info.data_is_empty() {
        let rent = Rent::get()?;
        let lamports = rent.minimum_balance(space);
        let ix = system_instruction::create_account(
            payer_info.key,
            account_info.key,
            lamports,
            space as u64,
            &crate::ID,
        );
        invoke_signed(
            &ix,
            &[payer_info.clone(), account_info.clone(), system_program_info.clone()],
            &[signer_seeds],
        )?;
    }
    Ok(())
}

pub fn load_or_init_investor_position<'info>(
    account_info: &AccountInfo<'info>,
    payer_info: &AccountInfo<'info>,
    system_program_info: &AccountInfo<'info>,
    facility: &Pubkey,
    investor: &Pubkey,
    bump: u8,
    now: i64,
) -> Result<InvestorPosition> {
    let space = ACCOUNT_DISCRIMINATOR_SIZE + InvestorPosition::INIT_SPACE;
    let bump_seed = [bump];
    let signer_seeds: &[&[u8]] = &[
        b"investor", facility.as_ref(), investor.as_ref(), bump_seed.as_ref(),
    ];
    if account_info.data_is_empty() {
        create_pda_account(account_info, payer_info, system_program_info, space, signer_seeds)?;
        let position = InvestorPosition {
            facility: *facility,
            investor: *investor,
            principal: 0,
            share_balance: 0,
            deposited_amount: 0,
            redeemed_amount: 0,
            pending_redemption_amount: 0,
            status: InvestorPositionStatus::Active,
            created_at: now,
            updated_at: now,
            bump,
        };
        save_investor_position(account_info, &position)?;
        Ok(position)
    } else {
        load_existing_investor_position(account_info)
    }
}

pub fn load_existing_investor_position<'info>(account_info: &AccountInfo<'info>) -> Result<InvestorPosition> {
    require!(!account_info.data_is_empty(), PrivateCreditError::InvestorPositionNotFound);
    require_keys_eq!(*account_info.owner, crate::ID, PrivateCreditError::Unauthorized);
    let data = account_info.try_borrow_data()?;
    let mut data_slice: &[u8] = &data[..];
    InvestorPosition::try_deserialize(&mut data_slice)
}

pub fn save_investor_position<'info>(account_info: &AccountInfo<'info>, position: &InvestorPosition) -> Result<()> {
    require_keys_eq!(*account_info.owner, crate::ID, PrivateCreditError::Unauthorized);
    let mut data = account_info.try_borrow_mut_data()?;
    let mut cursor = Cursor::new(&mut data[..]);
    position.try_serialize(&mut cursor)?;
    Ok(())
}

pub fn load_or_init_redemption_request<'info>(
    account_info: &AccountInfo<'info>,
    payer_info: &AccountInfo<'info>,
    system_program_info: &AccountInfo<'info>,
    facility: &Pubkey,
    investor: &Pubkey,
    bump: u8,
    queue_index: u64,
    now: i64,
) -> Result<(RedemptionRequest, bool)> {
    let space = ACCOUNT_DISCRIMINATOR_SIZE + RedemptionRequest::INIT_SPACE;
    let bump_seed = [bump];
    let signer_seeds: &[&[u8]] = &[
        b"redemption", facility.as_ref(), investor.as_ref(), bump_seed.as_ref(),
    ];
    if account_info.data_is_empty() {
        create_pda_account(account_info, payer_info, system_program_info, space, signer_seeds)?;
        let request = RedemptionRequest {
            facility: *facility,
            investor: *investor,
            requested_amount: 0,
            processed_amount: 0,
            timestamp: now,
            processed_timestamp: 0,
            status: RedemptionStatus::Requested,
            queue_index,
            bump,
        };
        save_redemption_request(account_info, &request)?;
        Ok((request, true))
    } else {
        Ok((load_existing_redemption_request(account_info)?, false))
    }
}

pub fn load_existing_redemption_request<'info>(account_info: &AccountInfo<'info>) -> Result<RedemptionRequest> {
    require!(!account_info.data_is_empty(), PrivateCreditError::RedemptionRequestNotFound);
    require_keys_eq!(*account_info.owner, crate::ID, PrivateCreditError::Unauthorized);
    let data = account_info.try_borrow_data()?;
    let mut data_slice: &[u8] = &data[..];
    RedemptionRequest::try_deserialize(&mut data_slice)
}

pub fn save_redemption_request<'info>(account_info: &AccountInfo<'info>, request: &RedemptionRequest) -> Result<()> {
    require_keys_eq!(*account_info.owner, crate::ID, PrivateCreditError::Unauthorized);
    let mut data = account_info.try_borrow_mut_data()?;
    let mut cursor = Cursor::new(&mut data[..]);
    request.try_serialize(&mut cursor)?;
    Ok(())
}
