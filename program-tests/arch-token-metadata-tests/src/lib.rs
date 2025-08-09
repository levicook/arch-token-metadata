pub const ARCH_TOKEN_METADATA_ELF: &[u8] = include_bytes!(std::env!("ARCH_TOKEN_METADATA_SO"));

use arch_program::{
    account::{AccountMeta, MIN_ACCOUNT_LAMPORTS},
    instruction::Instruction,
    program_pack::Pack,
    pubkey::Pubkey,
    system_instruction,
};
use arch_testing::TestContext;
use arch_token_metadata::find_metadata_pda_with_program;
use arch_token_metadata::instruction::MetadataInstruction;
use bitcoin::key::Keypair;

pub async fn deploy_token_metadata_program(ctx: &TestContext) -> anyhow::Result<Pubkey> {
    let (deployer_kp, _deployer_pk, _) = ctx.generate_new_keypair();
    ctx.fund_keypair_with_faucet(&deployer_kp).await?;

    let (program_kp, program_id, _) = ctx.generate_new_keypair();
    ctx.deploy_program(program_kp, deployer_kp, ARCH_TOKEN_METADATA_ELF)
        .await?;

    Ok(program_id)
}

pub fn create_and_init_mint_instructions(
    payer_pk: Pubkey,
    mint_pk: Pubkey,
    mint_authority: Pubkey,
    freeze_authority: Option<Pubkey>,
) -> anyhow::Result<[Instruction; 2]> {
    let create_mint_ix = system_instruction::create_account(
        &payer_pk,
        &mint_pk,
        MIN_ACCOUNT_LAMPORTS,
        apl_token::state::Mint::LEN as u64,
        &apl_token::id(),
    );
    let init_mint_ix = apl_token::instruction::initialize_mint2(
        &apl_token::id(),
        &mint_pk,
        &mint_authority,
        freeze_authority.as_ref(),
        9,
    )?;
    Ok([create_mint_ix, init_mint_ix])
}

pub async fn create_and_init_mint(
    ctx: &TestContext,
    payer_kp: &Keypair,
    payer_pk: Pubkey,
    mint_kp: &Keypair,
    mint_pk: Pubkey,
    mint_authority: &Pubkey,
    freeze_authority: Option<&Pubkey>,
) -> anyhow::Result<()> {
    let [create_mint_ix, init_mint_ix] = create_and_init_mint_instructions(
        payer_pk,
        mint_pk,
        *mint_authority,
        freeze_authority.copied(),
    )?;
    let recent = ctx.get_recent_blockhash().await?;
    let msg = arch_program::sanitized::ArchMessage::new(
        &[create_mint_ix, init_mint_ix],
        Some(payer_pk),
        recent.parse()?,
    );
    let tx = ctx
        .build_and_sign_transaction(msg, vec![payer_kp.clone(), mint_kp.clone()])
        .await?;
    let txid = ctx.send_transaction(tx).await?;
    let res = ctx.wait_for_transaction(&txid).await?;
    anyhow::ensure!(
        res.status == arch_sdk::Status::Processed,
        "mint init failed"
    );
    Ok(())
}

pub async fn build_create_metadata_ix(
    program_id: Pubkey,
    payer_pk: Pubkey,
    mint_pk: Pubkey,
    mint_authority_pk: Pubkey,
    name: &str,
    symbol: &str,
    image: &str,
    description: &str,
    immutable: bool,
) -> (Instruction, Pubkey) {
    let (metadata_pda, _bump) = find_metadata_pda_with_program(&program_id, &mint_pk);

    let data = MetadataInstruction::CreateMetadata {
        name: name.to_string(),
        symbol: symbol.to_string(),
        image: image.to_string(),
        description: description.to_string(),
        immutable,
    }
    .pack();

    let accounts = vec![
        AccountMeta::new(payer_pk, true),
        AccountMeta::new_readonly(Pubkey::system_program(), false),
        AccountMeta::new_readonly(mint_pk, false),
        AccountMeta::new(metadata_pda, false),
        AccountMeta::new_readonly(mint_authority_pk, true),
    ];

    let instruction = Instruction {
        program_id,
        accounts,
        data,
    };

    (instruction, metadata_pda)
}
