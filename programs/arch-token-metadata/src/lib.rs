#![deny(missing_docs)]
#![cfg_attr(not(test), forbid(unsafe_code))]

//! Arch Network Token Metadata Standard

pub mod error;
pub mod instruction;
pub mod processor;
pub mod state;

#[cfg(not(feature = "no-entrypoint"))]
mod entrypoint;

use arch_program::{entrypoint::ProgramResult, program_error::ProgramError, pubkey::Pubkey};

/// The program ID for the Arch Token Metadata program
pub fn id() -> Pubkey {
    Pubkey::from_slice(b"arch-metadata000000000000000000")
}

/// Checks that the supplied program ID is the correct one for Arch Token Metadata
pub fn check_program_account(program_id: &Pubkey) -> ProgramResult {
    if program_id != &id() {
        return Err(ProgramError::IncorrectProgramId);
    }
    Ok(())
}
