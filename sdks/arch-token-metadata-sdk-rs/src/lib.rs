//! Arch Token Metadata – Rust SDK (client-side helpers)
//!
//! This crate provides:
//! - PDA helpers for core metadata and attributes accounts
//! - Instruction builders with correct account ordering and client-side validation
//! - Transaction builders for common flows (compose Vec<Instruction>)
//!
//! The builders mirror the on-chain program invariants documented in `docs/SECURITY.md`.
//! Signers, recent blockhashes, and submission are left to the caller.

use arch_program::program_pack::Pack;
use arch_program::{
    account::AccountMeta, instruction::Instruction, pubkey::Pubkey, system_instruction,
};

use apl_token;

use arch_token_metadata as program;
use program::state::{
    DESCRIPTION_MAX_LEN, IMAGE_MAX_LEN, MAX_ATTRIBUTES, MAX_KEY_LENGTH, MAX_VALUE_LENGTH,
    NAME_MAX_LEN, SYMBOL_MAX_LEN,
};

/// Thin client for building PDAs and instructions for the Arch Token Metadata program.
///
/// The `program_id` must be the deployed Arch Token Metadata program id.
pub struct TokenMetadataClient {
    pub program_id: Pubkey,
}

impl TokenMetadataClient {
    pub fn new(program_id: Pubkey) -> Self {
        Self { program_id }
    }

    /// Derive the metadata PDA for a given mint.
    pub fn metadata_pda(&self, mint: &Pubkey) -> Pubkey {
        let (pda, _bump) = program::find_metadata_pda_with_program(&self.program_id, mint);
        pda
    }

    /// Derive the metadata PDA for a given mint, with the bump.
    pub fn metadata_pda_and_bump(&self, mint: &Pubkey) -> (Pubkey, u8) {
        program::find_metadata_pda_with_program(&self.program_id, mint)
    }

    /// Derive the attributes PDA for a given mint.
    pub fn attributes_pda(&self, mint: &Pubkey) -> Pubkey {
        let (pda, _bump) = program::find_attributes_pda_with_program(&self.program_id, mint);
        pda
    }

    /// Derive the attributes PDA for a given mint, with the bump.
    pub fn attributes_pda_and_bump(&self, mint: &Pubkey) -> (Pubkey, u8) {
        program::find_attributes_pda_with_program(&self.program_id, mint)
    }

    /// Build a CreateMetadata instruction.
    ///
    /// Accounts (strict order):
    /// - payer (writable, signer)
    /// - system_program (readonly)
    /// - mint (readonly)
    /// - metadata_pda (writable)
    /// - mint_or_freeze_authority (readonly, signer)
    pub fn create_metadata_ix(&self, params: CreateMetadataParams) -> anyhow::Result<Instruction> {
        let metadata_pda = self.metadata_pda(&params.mint);
        self.validate_metadata_fields(
            &params.name,
            &params.symbol,
            &params.image,
            &params.description,
        )?;

        let data = program::instruction::MetadataInstruction::CreateMetadata {
            name: params.name,
            symbol: params.symbol,
            image: params.image,
            description: params.description,
            immutable: params.immutable,
        }
        .pack();

        Ok(Instruction {
            program_id: self.program_id,
            accounts: vec![
                AccountMeta::new(params.payer, true),
                AccountMeta::new_readonly(Pubkey::system_program(), false),
                AccountMeta::new_readonly(params.mint, false),
                AccountMeta::new(metadata_pda, false),
                AccountMeta::new_readonly(params.mint_or_freeze_authority, true),
            ],
            data,
        })
    }

    /// Build an UpdateMetadata instruction.
    ///
    /// Accounts (strict order):
    /// - metadata_pda (writable)
    /// - update_authority (readonly, signer)
    pub fn update_metadata_ix(&self, params: UpdateMetadataParams) -> anyhow::Result<Instruction> {
        let metadata_pda = self.metadata_pda(&params.mint);
        self.validate_optional_metadata_fields(
            params.name.as_ref(),
            params.symbol.as_ref(),
            params.image.as_ref(),
            params.description.as_ref(),
        )?;
        let data = program::instruction::MetadataInstruction::UpdateMetadata {
            name: params.name,
            symbol: params.symbol,
            image: params.image,
            description: params.description,
        }
        .pack();

        Ok(Instruction {
            program_id: self.program_id,
            accounts: vec![
                AccountMeta::new(metadata_pda, false),
                AccountMeta::new_readonly(params.update_authority, true),
            ],
            data,
        })
    }

    /// Build a CreateAttributes instruction.
    ///
    /// Accounts (strict order):
    /// - payer (writable, signer)
    /// - system_program (readonly)
    /// - mint (readonly)
    /// - attributes_pda (writable)
    /// - update_authority (readonly, signer)
    /// - metadata_pda (readonly)
    pub fn create_attributes_ix(
        &self,
        params: CreateAttributesParams,
    ) -> anyhow::Result<Instruction> {
        let metadata_pda = self.metadata_pda(&params.mint);
        let attributes_pda = self.attributes_pda(&params.mint);
        self.validate_attributes(&params.data)?;

        let data =
            program::instruction::MetadataInstruction::CreateAttributes { data: params.data }
                .pack();

        Ok(Instruction {
            program_id: self.program_id,
            accounts: vec![
                AccountMeta::new(params.payer, true),
                AccountMeta::new_readonly(Pubkey::system_program(), false),
                AccountMeta::new_readonly(params.mint, false),
                AccountMeta::new(attributes_pda, false),
                AccountMeta::new_readonly(params.update_authority, true),
                AccountMeta::new_readonly(metadata_pda, false),
            ],
            data,
        })
    }

    /// Build a ReplaceAttributes instruction.
    ///
    /// Accounts (strict order):
    /// - attributes_pda (writable)
    /// - update_authority (readonly, signer)
    /// - metadata_pda (readonly)
    pub fn replace_attributes_ix(
        &self,
        params: ReplaceAttributesParams,
    ) -> anyhow::Result<Instruction> {
        let metadata_pda = self.metadata_pda(&params.mint);
        let attributes_pda = self.attributes_pda(&params.mint);
        self.validate_attributes(&params.data)?;

        let data =
            program::instruction::MetadataInstruction::ReplaceAttributes { data: params.data }
                .pack();

        Ok(Instruction {
            program_id: self.program_id,
            accounts: vec![
                AccountMeta::new(attributes_pda, false),
                AccountMeta::new_readonly(params.update_authority, true),
                AccountMeta::new_readonly(metadata_pda, false),
            ],
            data,
        })
    }

    /// Build a TransferAuthority instruction.
    ///
    /// Accounts (strict order):
    /// - metadata_pda (writable)
    /// - current_update_authority (readonly, signer)
    pub fn transfer_authority_ix(
        &self,
        params: TransferAuthorityParams,
    ) -> anyhow::Result<Instruction> {
        let metadata_pda = self.metadata_pda(&params.mint);
        let data = program::instruction::MetadataInstruction::TransferAuthority {
            new_authority: params.new_authority,
        }
        .pack();

        Ok(Instruction {
            program_id: self.program_id,
            accounts: vec![
                AccountMeta::new(metadata_pda, false),
                AccountMeta::new_readonly(params.current_update_authority, true),
            ],
            data,
        })
    }

    /// Build a MakeImmutable instruction.
    ///
    /// Accounts (strict order):
    /// - metadata_pda (writable)
    /// - current_update_authority (readonly, signer)
    pub fn make_immutable_ix(&self, params: MakeImmutableParams) -> anyhow::Result<Instruction> {
        let metadata_pda = self.metadata_pda(&params.mint);
        let data = program::instruction::MetadataInstruction::MakeImmutable.pack();

        Ok(Instruction {
            program_id: self.program_id,
            accounts: vec![
                AccountMeta::new(metadata_pda, false),
                AccountMeta::new_readonly(params.current_update_authority, true),
            ],
            data,
        })
    }

    // Upstream APL Token program helpers
    /// Build a SystemProgram create_account to allocate an APL Token mint account.
    pub fn create_mint_account_ix(&self, payer: Pubkey, mint: Pubkey) -> Instruction {
        use arch_program::account::MIN_ACCOUNT_LAMPORTS;
        system_instruction::create_account(
            &payer,
            &mint,
            MIN_ACCOUNT_LAMPORTS,
            apl_token::state::Mint::LEN as u64,
            &apl_token::id(),
        )
    }

    /// Build an APL Token initialize_mint2 instruction.
    pub fn initialize_mint2_ix(
        &self,
        mint: Pubkey,
        mint_authority: Pubkey,
        freeze_authority: Option<Pubkey>,
        decimals: u8,
    ) -> anyhow::Result<Instruction> {
        let ix = apl_token::instruction::initialize_mint2(
            &apl_token::id(),
            &mint,
            &mint_authority,
            freeze_authority.as_ref(),
            decimals,
        )?;
        Ok(ix)
    }

    /// Build an APL Token set_authority(MintTokens) instruction.
    pub fn set_mint_authority_ix(
        &self,
        mint: Pubkey,
        new_authority: Option<Pubkey>,
        current_authority: Pubkey,
    ) -> anyhow::Result<Instruction> {
        let ix = apl_token::instruction::set_authority(
            &apl_token::id(),
            &mint,
            new_authority.as_ref(),
            apl_token::instruction::AuthorityType::MintTokens,
            &current_authority,
            &[],
        )?;
        Ok(ix)
    }

    // Transaction patterns (compose instructions; signing and submission left to caller)
    /// Create an APL Token mint and Arch Token Metadata in one sequence.
    ///
    /// Returns a Vec<Instruction> with: [create_mint, initialize_mint2, create_metadata].
    pub fn create_token_with_metadata_tx(
        &self,
        params: TxCreateTokenWithMetadataParams,
    ) -> anyhow::Result<Vec<Instruction>> {
        let create_mint_ix = self.create_mint_account_ix(params.payer, params.mint);

        let init_mint_ix = self.initialize_mint2_ix(
            params.mint,
            params.mint_authority,
            params.freeze_authority,
            params.decimals,
        )?;

        let create_md_ix = self.create_metadata_ix(CreateMetadataParams {
            payer: params.payer,
            mint: params.mint,
            mint_or_freeze_authority: params.mint_authority,
            name: params.name,
            symbol: params.symbol,
            image: params.image,
            description: params.description,
            immutable: params.immutable,
        })?;

        Ok(vec![create_mint_ix, init_mint_ix, create_md_ix])
    }

    /// Same as `create_token_with_metadata_tx` but also returns derived PDAs for ergonomics.
    pub fn create_token_with_metadata_tx_with_pdas(
        &self,
        params: TxCreateTokenWithMetadataParams,
    ) -> anyhow::Result<(Vec<Instruction>, DerivedPdas)> {
        let md_pda = self.metadata_pda(&params.mint);
        let tx = self.create_token_with_metadata_tx(params)?;
        Ok((
            tx,
            DerivedPdas {
                metadata_pda: md_pda,
                attributes_pda: None,
            },
        ))
    }

    /// Create mint, initialize, create metadata, and create attributes in one sequence.
    /// Returns: [create_mint, initialize_mint2, create_metadata, create_attributes].
    pub fn create_token_with_metadata_and_attributes_tx(
        &self,
        params: TxCreateTokenWithMetadataAndAttributesParams,
    ) -> anyhow::Result<Vec<Instruction>> {
        let create_mint_ix = self.create_mint_account_ix(params.payer, params.mint);
        let init_mint_ix = self.initialize_mint2_ix(
            params.mint,
            params.mint_authority,
            params.freeze_authority,
            params.decimals,
        )?;

        let create_md_ix = self.create_metadata_ix(CreateMetadataParams {
            payer: params.payer,
            mint: params.mint,
            mint_or_freeze_authority: params.mint_authority,
            name: params.name,
            symbol: params.symbol,
            image: params.image,
            description: params.description,
            immutable: params.immutable,
        })?;

        let create_attrs_ix = self.create_attributes_ix(CreateAttributesParams {
            payer: params.payer,
            mint: params.mint,
            update_authority: params.mint_authority,
            data: params.attributes,
        })?;

        Ok(vec![
            create_mint_ix,
            init_mint_ix,
            create_md_ix,
            create_attrs_ix,
        ])
    }

    /// Same as above, but also returns both PDAs.
    pub fn create_token_with_metadata_and_attributes_tx_with_pdas(
        &self,
        params: TxCreateTokenWithMetadataAndAttributesParams,
    ) -> anyhow::Result<(Vec<Instruction>, DerivedPdas)> {
        let md_pda = self.metadata_pda(&params.mint);
        let attrs_pda = self.attributes_pda(&params.mint);
        let tx = self.create_token_with_metadata_and_attributes_tx(params)?;
        Ok((
            tx,
            DerivedPdas {
                metadata_pda: md_pda,
                attributes_pda: Some(attrs_pda),
            },
        ))
    }

    /// Create metadata using freeze authority by clearing mint authority beforehand.
    /// Returns: [create_mint, initialize_mint2(with freeze), set_authority(MintTokens -> None), create_metadata]
    pub fn create_token_with_freeze_auth_metadata_tx(
        &self,
        params: TxCreateTokenWithFreezeAuthMetadataParams,
    ) -> anyhow::Result<Vec<Instruction>> {
        let create_mint_ix = self.create_mint_account_ix(params.payer, params.mint);
        let init_mint_ix = self.initialize_mint2_ix(
            params.mint,
            params.initial_mint_authority,
            Some(params.freeze_authority),
            params.decimals,
        )?;

        let clear_mint_auth_ix =
            self.set_mint_authority_ix(params.mint, None, params.initial_mint_authority)?;

        // create metadata signed by freeze authority
        let create_md_ix = self.create_metadata_ix(CreateMetadataParams {
            payer: params.payer,
            mint: params.mint,
            mint_or_freeze_authority: params.freeze_authority,
            name: params.name,
            symbol: params.symbol,
            image: params.image,
            description: params.description,
            immutable: params.immutable,
        })?;

        Ok(vec![
            create_mint_ix,
            init_mint_ix,
            clear_mint_auth_ix,
            create_md_ix,
        ])
    }

    /// Convenience wrapper returning one-instruction Vec for create_attributes.
    pub fn create_attributes_tx(
        &self,
        params: CreateAttributesParams,
    ) -> anyhow::Result<Vec<Instruction>> {
        Ok(vec![self.create_attributes_ix(params)?])
    }

    /// Convenience wrapper returning one-instruction Vec for replace_attributes.
    pub fn replace_attributes_tx(
        &self,
        params: ReplaceAttributesParams,
    ) -> anyhow::Result<Vec<Instruction>> {
        Ok(vec![self.replace_attributes_ix(params)?])
    }

    /// Transfer authority then immediately update metadata. Requires both current and new authorities to sign.
    /// Returns: [transfer_authority, update_metadata]
    pub fn transfer_authority_then_update_tx(
        &self,
        params: TxTransferAuthorityThenUpdateParams,
    ) -> anyhow::Result<Vec<Instruction>> {
        let transfer_ix = self.transfer_authority_ix(TransferAuthorityParams {
            mint: params.mint,
            current_update_authority: params.current_update_authority,
            new_authority: params.new_authority,
        })?;

        let update_ix = self.update_metadata_ix(UpdateMetadataParams {
            mint: params.mint,
            update_authority: params.new_authority,
            name: params.name,
            symbol: params.symbol,
            image: params.image,
            description: params.description,
        })?;

        Ok(vec![transfer_ix, update_ix])
    }

    /// Convenience wrapper returning one-instruction Vec for make_immutable.
    pub fn make_immutable_tx(
        &self,
        params: MakeImmutableParams,
    ) -> anyhow::Result<Vec<Instruction>> {
        Ok(vec![self.make_immutable_ix(params)?])
    }
}

// === Params ===
/// Parameters for CreateMetadata instruction.
pub struct CreateMetadataParams {
    /// Account that pays for the metadata PDA creation
    pub payer: Pubkey,
    /// Token mint the metadata is associated with
    pub mint: Pubkey,
    /// Signer that must match the mint authority, or the freeze authority if mint authority is None
    pub mint_or_freeze_authority: Pubkey,
    /// Token name (<= NAME_MAX_LEN)
    pub name: String,
    /// Token symbol (<= SYMBOL_MAX_LEN)
    pub symbol: String,
    /// Image URI (<= IMAGE_MAX_LEN)
    pub image: String,
    /// Description text (<= DESCRIPTION_MAX_LEN)
    pub description: String,
    /// If true, metadata is immutable (no update authority retained)
    pub immutable: bool,
}

/// Parameters for UpdateMetadata instruction.
pub struct UpdateMetadataParams {
    /// Token mint whose metadata is being updated
    pub mint: Pubkey,
    /// Current update authority (must sign)
    pub update_authority: Pubkey,
    /// Optional new name (<= NAME_MAX_LEN)
    pub name: Option<String>,
    /// Optional new symbol (<= SYMBOL_MAX_LEN)
    pub symbol: Option<String>,
    /// Optional new image URI (<= IMAGE_MAX_LEN)
    pub image: Option<String>,
    /// Optional new description (<= DESCRIPTION_MAX_LEN)
    pub description: Option<String>,
}

/// Parameters for CreateAttributes instruction.
pub struct CreateAttributesParams {
    /// Payer for attributes PDA creation
    pub payer: Pubkey,
    /// Token mint the attributes are associated with
    pub mint: Pubkey,
    /// Current update authority (must sign)
    pub update_authority: Pubkey,
    /// Attribute key-value pairs; length <= MAX_ATTRIBUTES; each key/value length constrained
    pub data: Vec<(String, String)>,
}

/// Parameters for ReplaceAttributes instruction.
pub struct ReplaceAttributesParams {
    /// Token mint whose attributes are being replaced
    pub mint: Pubkey,
    /// Current update authority (must sign)
    pub update_authority: Pubkey,
    /// New full attributes vector to replace the existing one
    pub data: Vec<(String, String)>,
}

/// Parameters for TransferAuthority instruction.
pub struct TransferAuthorityParams {
    /// Token mint whose metadata authority is being transferred
    pub mint: Pubkey,
    /// Current update authority (must sign)
    pub current_update_authority: Pubkey,
    /// New authority to set
    pub new_authority: Pubkey,
}

/// Parameters for MakeImmutable instruction.
pub struct MakeImmutableParams {
    /// Token mint whose metadata is being made immutable
    pub mint: Pubkey,
    /// Current update authority (must sign)
    pub current_update_authority: Pubkey,
}

/// Parameters for tx_create_token_with_metadata transaction pattern.
pub struct TxCreateTokenWithMetadataParams {
    /// Payer that funds the mint account and metadata PDA
    pub payer: Pubkey,
    /// Mint account public key
    pub mint: Pubkey,
    /// Initial mint authority
    pub mint_authority: Pubkey,
    /// Optional freeze authority for the mint
    pub freeze_authority: Option<Pubkey>,
    /// Number of decimals for the mint
    pub decimals: u8,
    /// Metadata name
    pub name: String,
    /// Metadata symbol
    pub symbol: String,
    /// Metadata image URI
    pub image: String,
    /// Metadata description
    pub description: String,
    /// Whether metadata should be immutable at creation
    pub immutable: bool,
}

/// Parameters for tx_create_token_with_metadata_and_attributes transaction pattern.
pub struct TxCreateTokenWithMetadataAndAttributesParams {
    /// Payer that funds the mint account and PDAs
    pub payer: Pubkey,
    /// Mint account public key
    pub mint: Pubkey,
    /// Initial mint authority (and update authority for metadata/attributes)
    pub mint_authority: Pubkey,
    /// Optional freeze authority for the mint
    pub freeze_authority: Option<Pubkey>,
    /// Number of decimals for the mint
    pub decimals: u8,
    /// Metadata fields
    pub name: String,
    pub symbol: String,
    pub image: String,
    pub description: String,
    pub immutable: bool,
    /// Initial attributes to set
    pub attributes: Vec<(String, String)>,
}

/// Parameters for freeze-authority metadata creation flow.
pub struct TxCreateTokenWithFreezeAuthMetadataParams {
    /// Payer that funds the mint account and metadata PDA
    pub payer: Pubkey,
    /// Mint account public key
    pub mint: Pubkey,
    /// Initial mint authority that will be cleared to None before create_metadata
    pub initial_mint_authority: Pubkey,
    /// Freeze authority who will sign create_metadata
    pub freeze_authority: Pubkey,
    /// Number of decimals for the mint
    pub decimals: u8,
    /// Metadata fields
    pub name: String,
    pub symbol: String,
    pub image: String,
    pub description: String,
    pub immutable: bool,
}

/// Parameters for transfer_authority followed by update in one transaction.
pub struct TxTransferAuthorityThenUpdateParams {
    /// Token mint
    pub mint: Pubkey,
    /// Current update authority (signs transfer)
    pub current_update_authority: Pubkey,
    /// New authority to transfer to (signs update)
    pub new_authority: Pubkey,
    /// Update fields applied by the new authority
    pub name: Option<String>,
    pub symbol: Option<String>,
    pub image: Option<String>,
    pub description: Option<String>,
}

/// Convenience return type when a builder returns derived PDAs too.
pub struct DerivedPdas {
    /// Derived metadata PDA for the mint
    pub metadata_pda: Pubkey,
    /// Derived attributes PDA for the mint, if relevant to the tx
    pub attributes_pda: Option<Pubkey>,
}

// === Validation helpers ===
impl TokenMetadataClient {
    fn validate_metadata_fields(
        &self,
        name: &str,
        symbol: &str,
        image: &str,
        description: &str,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(name.len() <= NAME_MAX_LEN, "name too long");
        anyhow::ensure!(symbol.len() <= SYMBOL_MAX_LEN, "symbol too long");
        anyhow::ensure!(image.len() <= IMAGE_MAX_LEN, "image too long");
        anyhow::ensure!(
            description.len() <= DESCRIPTION_MAX_LEN,
            "description too long"
        );
        Ok(())
    }

    fn validate_optional_metadata_fields(
        &self,
        name: Option<&String>,
        symbol: Option<&String>,
        image: Option<&String>,
        description: Option<&String>,
    ) -> anyhow::Result<()> {
        if let Some(v) = name {
            anyhow::ensure!(v.len() <= NAME_MAX_LEN, "name too long");
        }
        if let Some(v) = symbol {
            anyhow::ensure!(v.len() <= SYMBOL_MAX_LEN, "symbol too long");
        }
        if let Some(v) = image {
            anyhow::ensure!(v.len() <= IMAGE_MAX_LEN, "image too long");
        }
        if let Some(v) = description {
            anyhow::ensure!(v.len() <= DESCRIPTION_MAX_LEN, "description too long");
        }
        Ok(())
    }

    fn validate_attributes(&self, data: &[(String, String)]) -> anyhow::Result<()> {
        anyhow::ensure!(data.len() <= MAX_ATTRIBUTES, "too many attributes");
        for (k, v) in data.iter() {
            anyhow::ensure!(k.len() <= MAX_KEY_LENGTH, "attribute key too long");
            anyhow::ensure!(v.len() <= MAX_VALUE_LENGTH, "attribute value too long");
        }
        Ok(())
    }
}
