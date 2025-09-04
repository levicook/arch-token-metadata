use arch_program::pubkey::Pubkey;

pub const ARCH_TOKEN_METADATA_ELF: &[u8] = include_bytes!(std::env!("ARCH_TOKEN_METADATA_SO"));

/// Canonical program id for the Arch Token Metadata program.
/// Must match programs/arch-token-metadata/src/lib.rs::id().
pub const PROGRAM_ID: Pubkey = Pubkey::new_from_array(*b"ArchTokenMetadata111111111111111");
