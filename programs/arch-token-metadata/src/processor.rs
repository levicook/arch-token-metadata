//! Program state processor

use {
    crate::{error::MetadataError, instruction::MetadataInstruction, state::TokenMetadata},
    arch_program::{
        account::{next_account_info, AccountInfo},
        entrypoint::ProgramResult,
        msg,
        program_pack::Pack,
        pubkey::Pubkey,
    },
};

/// Program state handler.
pub struct Processor {}

impl Processor {
    /// Process a single instruction
    pub fn process(
        _program_id: &Pubkey,
        accounts: &[AccountInfo],
        instruction_data: &[u8],
    ) -> ProgramResult {
        let instruction = MetadataInstruction::unpack(instruction_data)?;

        match instruction {
            MetadataInstruction::CreateMetadata {
                name,
                symbol,
                image,
                description,
                update_authority,
            } => {
                msg!("Instruction: CreateMetadata");
                Self::process_create_metadata(
                    accounts,
                    name,
                    symbol,
                    image,
                    description,
                    update_authority,
                )
            }
            MetadataInstruction::UpdateMetadata {
                name,
                symbol,
                image,
                description,
            } => {
                msg!("Instruction: UpdateMetadata");
                Self::process_update_metadata(accounts, name, symbol, image, description)
            }
            MetadataInstruction::CreateAttributes { data } => {
                msg!("Instruction: CreateAttributes");
                Self::process_create_attributes(accounts, data)
            }
            MetadataInstruction::UpdateAttributes { data } => {
                msg!("Instruction: UpdateAttributes");
                Self::process_update_attributes(accounts, data)
            }
            MetadataInstruction::TransferAuthority { new_authority } => {
                msg!("Instruction: TransferAuthority");
                Self::process_transfer_authority(accounts, new_authority)
            }
        }
    }

    fn process_create_metadata(
        accounts: &[AccountInfo],
        name: String,
        symbol: String,
        image: String,
        description: String,
        update_authority: Option<Pubkey>,
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let mint_info = next_account_info(account_info_iter)?;
        let metadata_info = next_account_info(account_info_iter)?;
        let mint_authority_info = next_account_info(account_info_iter)?;

        // Validate mint authority is signer
        if !mint_authority_info.is_signer {
            return Err(MetadataError::InvalidAuthority.into());
        }

        // Create metadata
        let metadata = TokenMetadata {
            mint: *mint_info.key,
            name,
            symbol,
            image,
            description,
            update_authority,
        };

        metadata.pack_into_slice(&mut metadata_info.data.borrow_mut());
        Ok(())
    }

    fn process_update_metadata(
        _accounts: &[AccountInfo],
        _name: Option<String>,
        _symbol: Option<String>,
        _image: Option<String>,
        _description: Option<String>,
    ) -> ProgramResult {
        // TODO: Implement update logic
        msg!("Update metadata not yet implemented");
        Ok(())
    }

    fn process_create_attributes(
        _accounts: &[AccountInfo],
        _data: Vec<(String, String)>,
    ) -> ProgramResult {
        // TODO: Implement attributes creation
        msg!("Create attributes not yet implemented");
        Ok(())
    }

    fn process_update_attributes(
        _accounts: &[AccountInfo],
        _data: Vec<(String, String)>,
    ) -> ProgramResult {
        // TODO: Implement attributes update
        msg!("Update attributes not yet implemented");
        Ok(())
    }

    fn process_transfer_authority(
        _accounts: &[AccountInfo],
        _new_authority: Option<Pubkey>,
    ) -> ProgramResult {
        // TODO: Implement authority transfer
        msg!("Transfer authority not yet implemented");
        Ok(())
    }
}
