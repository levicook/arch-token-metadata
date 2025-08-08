//! State transition types

use {
    arch_program::{
        program_error::ProgramError,
        program_pack::{IsInitialized, Pack, Sealed},
        pubkey::Pubkey,
    },
    borsh::{BorshDeserialize, BorshSerialize},
};

/// Core metadata account - always present, optimized for performance
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct TokenMetadata {
    /// The mint address this metadata belongs to
    pub mint: Pubkey,
    /// The name of the token
    pub name: String,
    /// The symbol of the token
    pub symbol: String,
    /// The image URI for the token
    pub image: String,
    /// The description of the token
    pub description: String,
    /// Optional update authority for the metadata
    pub update_authority: Option<Pubkey>,
}

impl Sealed for TokenMetadata {}
impl IsInitialized for TokenMetadata {
    fn is_initialized(&self) -> bool {
        true // Always initialized when created
    }
}

impl Pack for TokenMetadata {
    const LEN: usize = 32 + 4 + 256 + 4 + 16 + 4 + 512 + 4 + 1 + 32; // Approximate size

    fn unpack_from_slice(src: &[u8]) -> Result<Self, ProgramError> {
        borsh::from_slice(src).map_err(|_| ProgramError::InvalidAccountData)
    }

    fn pack_into_slice(&self, dst: &mut [u8]) {
        let data = borsh::to_vec(self).unwrap();
        dst[..data.len()].copy_from_slice(&data);
    }
}

/// Optional metadata attributes account - linked to core metadata
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct TokenMetadataAttributes {
    /// The mint address this attributes belong to
    pub mint: Pubkey,
    /// Key-value pairs for extensible attributes
    pub data: Vec<(String, String)>, // Key-value pairs for extensibility
}

impl Sealed for TokenMetadataAttributes {}
impl IsInitialized for TokenMetadataAttributes {
    fn is_initialized(&self) -> bool {
        true
    }
}

impl Pack for TokenMetadataAttributes {
    const LEN: usize = 32 + 4 + 1024; // Approximate size

    fn unpack_from_slice(src: &[u8]) -> Result<Self, ProgramError> {
        borsh::from_slice(src).map_err(|_| ProgramError::InvalidAccountData)
    }

    fn pack_into_slice(&self, dst: &mut [u8]) {
        let data = borsh::to_vec(self).unwrap();
        dst[..data.len()].copy_from_slice(&data);
    }
}
