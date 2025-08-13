use anyhow::Context;
use apl_token;
use arch_program::account::MIN_ACCOUNT_LAMPORTS;
use arch_program::program_pack::Pack;
use arch_program::pubkey::Pubkey;
use arch_program::system_instruction;
use arch_token_metadata::{
    find_attributes_pda_with_program, find_metadata_pda_with_program, id as program_id_fn,
    instruction::MetadataInstruction,
    state::{TokenMetadata, TokenMetadataAttributes},
};
use serde_json::json;

fn main() -> anyhow::Result<()> {
    // Deterministic example inputs
    let name = "Name".to_string();
    let symbol = "SYM".to_string();
    let image = "https://i".to_string();
    let description = "desc".to_string();

    let create = MetadataInstruction::CreateMetadata {
        name: name.clone(),
        symbol: symbol.clone(),
        image: image.clone(),
        description: description.clone(),
        immutable: false,
    };
    let update = MetadataInstruction::UpdateMetadata {
        name: Some("New".into()),
        symbol: None,
        image: None,
        description: None,
    };
    let create_attrs = MetadataInstruction::CreateAttributes {
        data: vec![("k1".into(), "v1".into()), ("k2".into(), "v2".into())],
    };
    let replace_attrs = MetadataInstruction::ReplaceAttributes {
        data: vec![("a".into(), "1".into())],
    };
    let new_auth = Pubkey::from_slice(&[7u8; 32]);
    let transfer = MetadataInstruction::TransferAuthority {
        new_authority: new_auth,
    };
    let make_imm = MetadataInstruction::MakeImmutable;

    let program_id = program_id_fn();
    // Two sample mints for PDA fixtures
    let mint_a = Pubkey::from_slice(&[2u8; 32]);
    let mint_b = Pubkey::from_slice(&[3u8; 32]);
    let (md_a, _) = find_metadata_pda_with_program(&program_id, &mint_a);
    let (md_b, _) = find_metadata_pda_with_program(&program_id, &mint_b);
    let (attrs_a, _) = find_attributes_pda_with_program(&program_id, &mint_a);
    let (attrs_b, _) = find_attributes_pda_with_program(&program_id, &mint_b);

    // Upstream fixtures: system + token program
    let token_program_id = apl_token::id();
    let payer = Pubkey::from_slice(&[1u8; 32]);
    let mint = mint_a;
    let sys_create_mint = system_instruction::create_account(
        &payer,
        &mint,
        MIN_ACCOUNT_LAMPORTS,
        apl_token::state::Mint::LEN as u64,
        &token_program_id,
    );
    let init_mint2 =
        apl_token::instruction::initialize_mint2(&token_program_id, &mint, &payer, None, 9)?;
    let set_mint_auth_none = apl_token::instruction::set_authority(
        &token_program_id,
        &mint,
        None,
        apl_token::instruction::AuthorityType::MintTokens,
        &payer,
        &[],
    )?;
    let set_mint_auth_some = apl_token::instruction::set_authority(
        &token_program_id,
        &mint,
        Some(&new_auth),
        apl_token::instruction::AuthorityType::MintTokens,
        &payer,
        &[],
    )?;

    let fixtures = json!({
        "CreateMetadata": hex::encode(create.pack()),
        "UpdateMetadata": hex::encode(update.pack()),
        "CreateAttributes": hex::encode(create_attrs.pack()),
        "ReplaceAttributes": hex::encode(replace_attrs.pack()),
        "TransferAuthority": hex::encode(transfer.pack()),
        "MakeImmutable": hex::encode(make_imm.pack()),
        "SystemProgram": hex::encode(Pubkey::system_program()),
        "ProgramId": hex::encode(program_id),
        "TokenProgramId": hex::encode(token_program_id),
        "PdaSamples": [
            {
                "mint": hex::encode(mint_a),
                "metadata": hex::encode(md_a),
                "attributes": hex::encode(attrs_a)
            },
            {
                "mint": hex::encode(mint_b),
                "metadata": hex::encode(md_b),
                "attributes": hex::encode(attrs_b)
            }
        ],
        "SystemCreateAccountMint": hex::encode(sys_create_mint.data),
        "TokenInitializeMint2": hex::encode(init_mint2.data),
        "TokenSetAuthorityMintNone": hex::encode(set_mint_auth_none.data),
        "TokenSetAuthorityMintSome": hex::encode(set_mint_auth_some.data),
        // Compute budget fixtures (discriminants are LE u32; 0=RequestHeapFrame, 1=SetComputeUnitLimit)
        "ComputeBudget": {
            "ProgramId": hex::encode(arch_program::compute_budget::COMPUTE_BUDGET_PROGRAM_ID),
            "RequestHeapFrame_64k": hex::encode({
                let mut v = Vec::new();
                v.extend_from_slice(&0u32.to_le_bytes());
                v.extend_from_slice(&(64u32 * 1024).to_le_bytes());
                v
            }),
            "SetComputeUnitLimit_12000": hex::encode({
                let mut v = Vec::new();
                v.extend_from_slice(&1u32.to_le_bytes());
                v.extend_from_slice(&12000u32.to_le_bytes());
                v
            })
        },
        // Packed account bytes for sample metadata/attributes accounts (LEN-sized, zero-padded)
        "Sample": {
            "mint": hex::encode(mint_a),
            "metadata_account": sample_packed_metadata_hex(mint_a, &payer, &name, &symbol, &image, &description)?,
            "attributes_account": sample_packed_attributes_hex(mint_a)?,
        },
        "Sample2": {
            "mint": hex::encode(mint_b),
            "metadata_account": sample_packed_metadata_hex(mint_b, &payer, &name, &symbol, &image, &description)?,
            "attributes_account": sample_packed_attributes_hex(mint_b)?,
        }
    });

    let out_dir = std::env::var("OUT_FIXTURES_DIR").unwrap_or_else(|_| {
        // default to project-relative path used by tests
        "sdks/arch-token-metadata-sdk-ts/test/fixtures".to_string()
    });
    std::fs::create_dir_all(&out_dir).context("create fixtures dir")?;
    let path = format!("{}/metadata_instructions.json", out_dir);
    std::fs::write(&path, serde_json::to_vec_pretty(&fixtures)?)
        .with_context(|| format!("write {}", path))?;

    println!("wrote fixtures to {}", path);
    Ok(())
}

fn sample_packed_metadata_hex(
    mint: Pubkey,
    update_authority: &Pubkey,
    name: &str,
    symbol: &str,
    image: &str,
    description: &str,
) -> anyhow::Result<String> {
    let md = TokenMetadata {
        is_initialized: true,
        mint,
        name: name.to_string(),
        symbol: symbol.to_string(),
        image: image.to_string(),
        description: description.to_string(),
        update_authority: Some(*update_authority),
    };
    let mut buf = vec![0u8; TokenMetadata::LEN];
    md.pack_into_slice(&mut buf);
    Ok(hex::encode(buf))
}

fn sample_packed_attributes_hex(mint: Pubkey) -> anyhow::Result<String> {
    let attrs = TokenMetadataAttributes {
        is_initialized: true,
        mint,
        data: vec![("k1".into(), "v1".into()), ("k2".into(), "v2".into())],
    };
    let mut buf = vec![0u8; TokenMetadataAttributes::LEN];
    attrs.pack_into_slice(&mut buf);
    Ok(hex::encode(buf))
}
