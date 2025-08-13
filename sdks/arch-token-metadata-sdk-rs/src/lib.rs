//! Arch Token Metadata – Rust SDK (client-side helpers)
//!
//! This crate provides:
//! - PDA helpers for core metadata and attributes accounts
//! - Instruction builders with correct account ordering and client-side validation
//! - Transaction builders for common flows (compose Vec<Instruction>)
//!
//! The builders mirror the on-chain program invariants documented in `docs/SECURITY.md`.
//! Signers, recent blockhashes, and submission are left to the caller.

use arch_program::compute_budget::ComputeBudgetInstruction;
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

// Reader support
use anyhow::Context as _;
use program::state::{TokenMetadata, TokenMetadataAttributes};

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

    /// Same as `create_token_with_metadata_tx`, but allows prepending compute-budget instructions.
    pub fn create_token_with_metadata_tx_with_budget(
        &self,
        params: TxCreateTokenWithMetadataParams,
        budget: ComputeBudgetOptions,
    ) -> anyhow::Result<Vec<Instruction>> {
        let mut out = self.compute_budget_ixs(&budget);
        out.extend(self.create_token_with_metadata_tx(params)?);
        Ok(out)
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

    /// Same as `create_token_with_metadata_and_attributes_tx`, with compute-budget instructions.
    pub fn create_token_with_metadata_and_attributes_tx_with_budget(
        &self,
        params: TxCreateTokenWithMetadataAndAttributesParams,
        budget: ComputeBudgetOptions,
    ) -> anyhow::Result<Vec<Instruction>> {
        let mut out = self.compute_budget_ixs(&budget);
        out.extend(self.create_token_with_metadata_and_attributes_tx(params)?);
        Ok(out)
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

    /// Same as `create_token_with_freeze_auth_metadata_tx`, with compute-budget instructions.
    pub fn create_token_with_freeze_auth_metadata_tx_with_budget(
        &self,
        params: TxCreateTokenWithFreezeAuthMetadataParams,
        budget: ComputeBudgetOptions,
    ) -> anyhow::Result<Vec<Instruction>> {
        let mut out = self.compute_budget_ixs(&budget);
        out.extend(self.create_token_with_freeze_auth_metadata_tx(params)?);
        Ok(out)
    }

    /// Convenience wrapper returning one-instruction Vec for create_attributes.
    pub fn create_attributes_tx(
        &self,
        params: CreateAttributesParams,
    ) -> anyhow::Result<Vec<Instruction>> {
        Ok(vec![self.create_attributes_ix(params)?])
    }

    /// `create_attributes_tx` with compute-budget instructions prepended.
    pub fn create_attributes_tx_with_budget(
        &self,
        params: CreateAttributesParams,
        budget: ComputeBudgetOptions,
    ) -> anyhow::Result<Vec<Instruction>> {
        let mut out = self.compute_budget_ixs(&budget);
        out.extend(self.create_attributes_tx(params)?);
        Ok(out)
    }

    /// Convenience wrapper returning one-instruction Vec for replace_attributes.
    pub fn replace_attributes_tx(
        &self,
        params: ReplaceAttributesParams,
    ) -> anyhow::Result<Vec<Instruction>> {
        Ok(vec![self.replace_attributes_ix(params)?])
    }

    /// `replace_attributes_tx` with compute-budget instructions prepended.
    pub fn replace_attributes_tx_with_budget(
        &self,
        params: ReplaceAttributesParams,
        budget: ComputeBudgetOptions,
    ) -> anyhow::Result<Vec<Instruction>> {
        let mut out = self.compute_budget_ixs(&budget);
        out.extend(self.replace_attributes_tx(params)?);
        Ok(out)
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

    /// `transfer_authority_then_update_tx` with compute-budget instructions prepended.
    pub fn transfer_authority_then_update_tx_with_budget(
        &self,
        params: TxTransferAuthorityThenUpdateParams,
        budget: ComputeBudgetOptions,
    ) -> anyhow::Result<Vec<Instruction>> {
        let mut out = self.compute_budget_ixs(&budget);
        out.extend(self.transfer_authority_then_update_tx(params)?);
        Ok(out)
    }

    /// Convenience wrapper returning one-instruction Vec for make_immutable.
    pub fn make_immutable_tx(
        &self,
        params: MakeImmutableParams,
    ) -> anyhow::Result<Vec<Instruction>> {
        Ok(vec![self.make_immutable_ix(params)?])
    }

    /// `make_immutable_tx` with compute-budget instructions prepended.
    pub fn make_immutable_tx_with_budget(
        &self,
        params: MakeImmutableParams,
        budget: ComputeBudgetOptions,
    ) -> anyhow::Result<Vec<Instruction>> {
        let mut out = self.compute_budget_ixs(&budget);
        out.extend(self.make_immutable_tx(params)?);
        Ok(out)
    }

    /// Compute budget: set a per-transaction compute unit limit.
    pub fn set_compute_unit_limit_ix(&self, units: u32) -> Instruction {
        ComputeBudgetInstruction::set_compute_unit_limit(units)
    }

    /// Compute budget: request a specific heap frame size in bytes (multiple of 1024).
    pub fn request_heap_frame_ix(&self, bytes: u32) -> Instruction {
        ComputeBudgetInstruction::request_heap_frame(bytes)
    }

    fn compute_budget_ixs(&self, opts: &ComputeBudgetOptions) -> Vec<Instruction> {
        let mut out: Vec<Instruction> = Vec::new();
        if let Some(units) = opts.units {
            out.push(self.set_compute_unit_limit_ix(units));
        }
        if let Some(bytes) = opts.heap_bytes {
            out.push(self.request_heap_frame_ix(bytes));
        }
        out
    }

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
            anyhow::ensure!(
                !k.is_empty() && !v.is_empty(),
                "attribute key and value must be non-empty"
            );
            anyhow::ensure!(k.len() <= MAX_KEY_LENGTH, "attribute key too long");
            anyhow::ensure!(v.len() <= MAX_VALUE_LENGTH, "attribute value too long");
        }
        Ok(())
    }
}

// Well-known attribute keys
pub mod well_known_attributes {
    pub const TWITTER: &str = "twitter";
    pub const TELEGRAM: &str = "telegram";
    pub const WEBSITE: &str = "website";
    pub const DISCORD: &str = "discord";
    pub const COINGECKO: &str = "coingecko";
    pub const WHITEPAPER: &str = "whitepaper";
    pub const AUDIT: &str = "audit";
    pub const CATEGORY: &str = "category";
    pub const TAGS: &str = "tags";
}

// === Reader (async RPC-based helpers) ===

/// Minimal account data used by the reader utilities.
pub struct AccountDataLite {
    pub data: Vec<u8>,
    pub owner: Pubkey,
}

/// Minimal async RPC trait required by reader utilities. Implemented for arch_sdk client via an adapter.
#[async_trait::async_trait]
pub trait AsyncAccountReader: Send + Sync {
    async fn get_multiple_accounts(
        &self,
        pubkeys: &[Pubkey],
    ) -> anyhow::Result<Vec<Option<AccountDataLite>>>;
}

/// Reader for fetching and decoding token metadata accounts using an injected async RPC.
pub struct TokenMetadataReader<R: AsyncAccountReader> {
    program_id: Pubkey,
    rpc: R,
}

impl<R: AsyncAccountReader> TokenMetadataReader<R> {
    pub fn new(program_id: Pubkey, rpc: R) -> Self {
        Self { program_id, rpc }
    }

    fn metadata_pda(&self, mint: &Pubkey) -> Pubkey {
        let (pda, _bump) = program::find_metadata_pda_with_program(&self.program_id, mint);
        pda
    }
    fn attributes_pda(&self, mint: &Pubkey) -> Pubkey {
        let (pda, _bump) = program::find_attributes_pda_with_program(&self.program_id, mint);
        pda
    }

    fn is_owner_ok(&self, owner: &Pubkey) -> bool {
        owner == &self.program_id
    }

    pub async fn get_token_metadata(&self, mint: Pubkey) -> anyhow::Result<Option<TokenMetadata>> {
        let pda = self.metadata_pda(&mint);
        let v = self.rpc.get_multiple_accounts(&[pda]).await?.pop().unwrap();
        let Some(acc) = v else { return Ok(None) };
        if !self.is_owner_ok(&acc.owner) {
            return Ok(None);
        }
        let md = TokenMetadata::unpack_from_slice(&acc.data).context("unpack TokenMetadata")?;
        Ok(Some(md))
    }

    pub async fn get_token_metadata_attributes(
        &self,
        mint: Pubkey,
    ) -> anyhow::Result<Option<TokenMetadataAttributes>> {
        let pda = self.attributes_pda(&mint);
        let v = self.rpc.get_multiple_accounts(&[pda]).await?.pop().unwrap();
        let Some(acc) = v else { return Ok(None) };
        if !self.is_owner_ok(&acc.owner) {
            return Ok(None);
        }
        let attrs = TokenMetadataAttributes::unpack_from_slice(&acc.data)
            .context("unpack TokenMetadataAttributes")?;
        Ok(Some(attrs))
    }

    pub async fn get_token_details(
        &self,
        mint: Pubkey,
    ) -> anyhow::Result<(Option<TokenMetadata>, Option<TokenMetadataAttributes>)> {
        let md_pda = self.metadata_pda(&mint);
        let at_pda = self.attributes_pda(&mint);
        let res = self.rpc.get_multiple_accounts(&[md_pda, at_pda]).await?;
        let md_opt = match &res[0] {
            Some(acc) if self.is_owner_ok(&acc.owner) => {
                Some(TokenMetadata::unpack_from_slice(&acc.data).context("unpack TokenMetadata")?)
            }
            _ => None,
        };
        let at_opt = match &res[1] {
            Some(acc) if self.is_owner_ok(&acc.owner) => Some(
                TokenMetadataAttributes::unpack_from_slice(&acc.data)
                    .context("unpack TokenMetadataAttributes")?,
            ),
            _ => None,
        };
        Ok((md_opt, at_opt))
    }

    pub async fn get_token_metadata_batch(
        &self,
        mints: &[Pubkey],
    ) -> anyhow::Result<Vec<Option<TokenMetadata>>> {
        let pdas: Vec<Pubkey> = mints.iter().map(|m| self.metadata_pda(m)).collect();
        let res = self.rpc.get_multiple_accounts(&pdas).await?;
        let mut out = Vec::with_capacity(res.len());
        for maybe in res {
            if let Some(acc) = maybe {
                if self.is_owner_ok(&acc.owner) {
                    let md = TokenMetadata::unpack_from_slice(&acc.data)
                        .context("unpack TokenMetadata")?;
                    out.push(Some(md));
                    continue;
                }
            }
            out.push(None);
        }
        Ok(out)
    }

    pub async fn get_token_metadata_attributes_batch(
        &self,
        mints: &[Pubkey],
    ) -> anyhow::Result<Vec<Option<TokenMetadataAttributes>>> {
        let pdas: Vec<Pubkey> = mints.iter().map(|m| self.attributes_pda(m)).collect();
        let res = self.rpc.get_multiple_accounts(&pdas).await?;
        let mut out = Vec::with_capacity(res.len());
        for maybe in res {
            if let Some(acc) = maybe {
                if self.is_owner_ok(&acc.owner) {
                    let attrs = TokenMetadataAttributes::unpack_from_slice(&acc.data)
                        .context("unpack TokenMetadataAttributes")?;
                    out.push(Some(attrs));
                    continue;
                }
            }
            out.push(None);
        }
        Ok(out)
    }
}

// Concrete adapter for arch_sdk::AsyncArchRpcClient
#[async_trait::async_trait]
impl AsyncAccountReader for arch_sdk::AsyncArchRpcClient {
    async fn get_multiple_accounts(
        &self,
        pubkeys: &[Pubkey],
    ) -> anyhow::Result<Vec<Option<AccountDataLite>>> {
        let mut out: Vec<Option<AccountDataLite>> = Vec::with_capacity(pubkeys.len());
        for pk in pubkeys.iter() {
            match self.read_account_info(*pk).await {
                Ok(info) => {
                    out.push(Some(AccountDataLite {
                        data: info.data,
                        owner: info.owner,
                    }));
                }
                Err(_e) => {
                    // If account missing or RPC error, return None for this entry
                    out.push(None);
                }
            }
        }
        Ok(out)
    }
}

// === Params ===
/// Parameters for CreateMetadata instruction.
#[derive(Clone, Debug)]
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
#[derive(Clone, Debug)]
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
#[derive(Clone, Debug)]
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
#[derive(Clone, Debug)]
pub struct ReplaceAttributesParams {
    /// Token mint whose attributes are being replaced
    pub mint: Pubkey,
    /// Current update authority (must sign)
    pub update_authority: Pubkey,
    /// New full attributes vector to replace the existing one
    pub data: Vec<(String, String)>,
}

/// Parameters for TransferAuthority instruction.
#[derive(Clone, Debug)]
pub struct TransferAuthorityParams {
    /// Token mint whose metadata authority is being transferred
    pub mint: Pubkey,
    /// Current update authority (must sign)
    pub current_update_authority: Pubkey,
    /// New authority to set
    pub new_authority: Pubkey,
}

/// Parameters for MakeImmutable instruction.
#[derive(Clone, Debug)]
pub struct MakeImmutableParams {
    /// Token mint whose metadata is being made immutable
    pub mint: Pubkey,
    /// Current update authority (must sign)
    pub current_update_authority: Pubkey,
}

/// Parameters for tx_create_token_with_metadata transaction pattern.
#[derive(Clone, Debug)]
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
#[derive(Clone, Debug)]
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
#[derive(Clone, Debug)]
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
#[derive(Clone, Debug)]
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

/// Optional compute budget options to prepend to transaction builders.
#[derive(Clone, Copy, Debug, Default)]
pub struct ComputeBudgetOptions {
    /// Optional per-transaction compute unit limit.
    pub units: Option<u32>,
    /// Optional requested heap frame size (multiple of 1024).
    pub heap_bytes: Option<u32>,
}
