//! Program state processor

use {
    crate::{
        error::MetadataError,
        find_metadata_pda_with_program,
        instruction::MetadataInstruction,
        state::{TokenMetadata, DESCRIPTION_MAX_LEN, IMAGE_MAX_LEN, NAME_MAX_LEN, SYMBOL_MAX_LEN},
    },
    apl_token::{self, state::Mint},
    arch_program::{
        account::{next_account_info, AccountInfo, MIN_ACCOUNT_LAMPORTS},
        entrypoint::ProgramResult,
        msg,
        program::invoke_signed,
        program_error::ProgramError,
        program_option::COption,
        program_pack::{IsInitialized, Pack},
        pubkey::Pubkey,
        system_instruction,
    },
};

/// Program state handler.
pub struct Processor {}

impl Processor {
    /// Process a single instruction
    pub fn process(
        program_id: &Pubkey,
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
                immutable,
            } => {
                msg!("Instruction: CreateMetadata");
                Self::process_create_metadata(
                    program_id,
                    accounts,
                    name,
                    symbol,
                    image,
                    description,
                    immutable,
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
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        name: String,
        symbol: String,
        image: String,
        description: String,
        immutable: bool,
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let payer_info = next_account_info(account_info_iter)?; // [writable, signer]
        let system_program_info = next_account_info(account_info_iter)?; // []
        let mint_info = next_account_info(account_info_iter)?; // []
        let metadata_info = next_account_info(account_info_iter)?; // [writable]
        let mint_authority_info = next_account_info(account_info_iter)?; // [signer]

        // Token mint must be owned by the token program
        if mint_info.owner != &apl_token::id() {
            msg!(
                "Mint is not owned by the token program expected {:?}, got {:?}",
                apl_token::id(),
                mint_info.owner
            );
            return Err(ProgramError::IncorrectProgramId);
        }

        // Deserialize mint and enforce authority policy
        let mint = Mint::unpack(&mint_info.data.borrow()) //
            .map_err(|_| ProgramError::InvalidAccountData)?;

        if !mint.is_initialized() {
            msg!("Mint is not initialized");
            return Err(ProgramError::UninitializedAccount);
        }

        // Check signer against mint authority, with freeze authority fallback
        let matched_signer: Option<Pubkey> = match mint.mint_authority {
            COption::Some(mint_auth) => {
                if cmp_pubkeys(&mint_auth, mint_authority_info.key) {
                    Some(mint_auth)
                } else {
                    msg!("Mint authority does not match expected authority");
                    return Err(MetadataError::InvalidAuthority.into());
                }
            }
            COption::None => match mint.freeze_authority {
                COption::Some(freeze_auth) => {
                    if cmp_pubkeys(&freeze_auth, mint_authority_info.key) {
                        Some(freeze_auth)
                    } else {
                        msg!("Freeze authority does not match expected authority");
                        return Err(MetadataError::InvalidAuthority.into());
                    }
                }
                COption::None => {
                    msg!("Mint has no mint or freeze authority");
                    return Err(MetadataError::InvalidAuthority.into());
                }
            },
        };

        // Validate mint authority is signer
        if !mint_authority_info.is_signer {
            msg!("Mint authority is not a signer");
            return Err(MetadataError::InvalidAuthority.into());
        }

        // Validate the metadata PDA address
        let (expected_pda, bump) = find_metadata_pda_with_program(program_id, mint_info.key);
        if !cmp_pubkeys(&expected_pda, metadata_info.key) {
            msg!("Metadata PDA does not match expected PDA");
            return Err(ProgramError::InvalidSeeds);
        }

        // If not owned by this program, create via CPI using PDA seeds
        if metadata_info.owner != program_id {
            // Require correct system program id
            if *system_program_info.key != Pubkey::system_program() {
                msg!("System program id does not match expected system program id");
                return Err(ProgramError::IncorrectProgramId);
            }

            if !payer_info.is_signer {
                msg!("Payer is not a signer");
                return Err(ProgramError::MissingRequiredSignature);
            }

            let space = TokenMetadata::LEN as u64;
            let lamports = MIN_ACCOUNT_LAMPORTS;
            let create_ix = system_instruction::create_account(
                payer_info.key,
                metadata_info.key,
                lamports,
                space,
                program_id,
            );

            invoke_signed(
                &create_ix,
                &[
                    payer_info.clone(),
                    metadata_info.clone(),
                    system_program_info.clone(),
                ],
                &[&[b"metadata", mint_info.key.as_ref(), &[bump]]],
            )?;
        }

        // Validate field sizes
        if name.len() > NAME_MAX_LEN
            || symbol.len() > SYMBOL_MAX_LEN
            || image.len() > IMAGE_MAX_LEN
            || description.len() > DESCRIPTION_MAX_LEN
        {
            return Err(MetadataError::StringTooLong.into());
        }

        // Ensure not already initialized (zero-initialized account will have first byte == 0)
        {
            let data_ref = metadata_info.data.borrow();
            if !data_ref.is_empty() && data_ref[0] != 0 {
                return Err(MetadataError::MetadataAlreadyExists.into());
            }
        }

        // Write metadata
        let metadata = TokenMetadata {
            is_initialized: true,
            mint: *mint_info.key,
            name,
            symbol,
            image,
            description,
            update_authority: if immutable { None } else { matched_signer },
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

/// Checks two pubkeys for equality using a cheap memcmp
fn cmp_pubkeys(a: &Pubkey, b: &Pubkey) -> bool {
    arch_program::program_memory::sol_memcmp(a.as_ref(), b.as_ref(), 32) == 0
}
