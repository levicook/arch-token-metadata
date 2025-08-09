use arch_program::{
    account::AccountMeta, instruction::Instruction, program_pack::Pack, pubkey::Pubkey,
    sanitized::ArchMessage, system_instruction,
};
use arch_sdk::Status;
use arch_testing::TestRunner;
use arch_token_metadata::{
    find_attributes_pda_with_program, find_metadata_pda_with_program,
    instruction::MetadataInstruction, state::TokenMetadataAttributes,
};
use arch_token_metadata_tests::deploy_token_metadata_program;
use serial_test::serial;

#[tokio::test]
#[serial]
// #[ignore = "Creating two PDAs via CPI in the same transaction currently triggers InvalidRealloc in the runtime"]
async fn create_attributes_success_same_tx() {
    TestRunner::run(|ctx| async move {
        let program_id = deploy_token_metadata_program(&ctx).await?;

        let (payer_kp, payer_pk, _) = ctx.generate_new_keypair();
        ctx.fund_keypair_with_faucet(&payer_kp).await?;

        let (mint_kp, mint_pk, _) = ctx.generate_new_keypair();
        let (metadata_pda, _bump_md) = find_metadata_pda_with_program(&program_id, &mint_pk);
        let (attrs_pda, _bump_attrs) = find_attributes_pda_with_program(&program_id, &mint_pk);

        // mint
        let create_mint_ix = system_instruction::create_account(
            &payer_pk,
            &mint_pk,
            arch_program::account::MIN_ACCOUNT_LAMPORTS,
            apl_token::state::Mint::LEN as u64,
            &apl_token::id(),
        );

        let init_mint_ix = apl_token::instruction::initialize_mint2(
            &apl_token::id(),
            &mint_pk,
            &payer_pk,
            None,
            9,
        )?;

        // metadata (update_authority=payer)
        let create_md = MetadataInstruction::CreateMetadata {
            name: "N".into(),
            symbol: "S".into(),
            image: "i".into(),
            description: "d".into(),
            immutable: false,
        }
        .pack();

        let create_md_ix = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(payer_pk, true),
                AccountMeta::new_readonly(Pubkey::system_program(), false),
                AccountMeta::new_readonly(mint_pk, false),
                AccountMeta::new(metadata_pda, false),
                AccountMeta::new_readonly(payer_pk, true),
            ],
            data: create_md,
        };

        // attributes
        let data = vec![("k1".into(), "v1".into()), ("k2".into(), "v2".into())];
        let create_attrs = MetadataInstruction::CreateAttributes { data: data.clone() }.pack();
        let create_attrs_ix = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(payer_pk, true),
                AccountMeta::new_readonly(Pubkey::system_program(), false),
                AccountMeta::new_readonly(mint_pk, false),
                AccountMeta::new(attrs_pda, false),
                AccountMeta::new_readonly(payer_pk, true),
                AccountMeta::new_readonly(metadata_pda, false),
            ],
            data: create_attrs,
        };

        let recent = ctx.get_recent_blockhash().await?;

        let msg = ArchMessage::new(
            &[create_mint_ix, init_mint_ix, create_md_ix, create_attrs_ix],
            Some(payer_pk),
            recent.parse()?,
        );

        let tx = ctx
            .build_and_sign_transaction(msg, vec![payer_kp, mint_kp])
            .await?;

        let txid = ctx.send_transaction(tx).await?;
        let res = ctx.wait_for_transaction(&txid).await?;
        assert_eq!(res.status, Status::Processed);

        let acct = ctx.read_account_info(attrs_pda).await?;
        let attrs = TokenMetadataAttributes::unpack(&acct.data).unwrap();
        assert!(attrs.is_initialized);
        assert_eq!(attrs.data, data);
        Ok(())
    })
    .await
}

#[tokio::test]
#[serial]
async fn create_attributes_success_two_txs() {
    TestRunner::run(|ctx| async move {
        let program_id = deploy_token_metadata_program(&ctx).await?;

        let (payer_kp, payer_pk, _) = ctx.generate_new_keypair();
        ctx.fund_keypair_with_faucet(&payer_kp).await?;

        let (mint_kp, mint_pk, _) = ctx.generate_new_keypair();
        let (metadata_pda, _bump_md) = find_metadata_pda_with_program(&program_id, &mint_pk);
        let (attrs_pda, _bump_attrs) = find_attributes_pda_with_program(&program_id, &mint_pk);

        // Tx1: create and init mint + create metadata
        let create_mint_ix = system_instruction::create_account(
            &payer_pk,
            &mint_pk,
            arch_program::account::MIN_ACCOUNT_LAMPORTS,
            apl_token::state::Mint::LEN as u64,
            &apl_token::id(),
        );
        let init_mint_ix = apl_token::instruction::initialize_mint2(
            &apl_token::id(),
            &mint_pk,
            &payer_pk,
            None,
            9,
        )?;
        let create_md = MetadataInstruction::CreateMetadata {
            name: "N".into(),
            symbol: "S".into(),
            image: "i".into(),
            description: "d".into(),
            immutable: false,
        }
        .pack();
        let create_md_ix = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(payer_pk, true),
                AccountMeta::new_readonly(Pubkey::system_program(), false),
                AccountMeta::new_readonly(mint_pk, false),
                AccountMeta::new(metadata_pda, false),
                AccountMeta::new_readonly(payer_pk, true),
            ],
            data: create_md,
        };
        let recent1 = ctx.get_recent_blockhash().await?;
        let msg1 = ArchMessage::new(
            &[create_mint_ix, init_mint_ix, create_md_ix],
            Some(payer_pk),
            recent1.parse()?,
        );
        let tx1 = ctx
            .build_and_sign_transaction(msg1, vec![payer_kp.clone(), mint_kp.clone()])
            .await?;
        let txid1 = ctx.send_transaction(tx1).await?;
        let res1 = ctx.wait_for_transaction(&txid1).await?;
        res1.logs.iter().for_each(|l| println!(">>> {}", l));

        assert_eq!(res1.status, Status::Processed);

        // Tx2: create attributes
        let data = vec![("k1".into(), "v1".into()), ("k2".into(), "v2".into())];
        let create_attrs = MetadataInstruction::CreateAttributes { data: data.clone() }.pack();
        let create_attrs_ix = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(payer_pk, true),
                AccountMeta::new_readonly(Pubkey::system_program(), false),
                AccountMeta::new_readonly(mint_pk, false),
                AccountMeta::new(attrs_pda, false),
                AccountMeta::new_readonly(payer_pk, true),
                AccountMeta::new_readonly(metadata_pda, false),
            ],
            data: create_attrs,
        };
        let recent2 = ctx.get_recent_blockhash().await?;
        let msg2 = ArchMessage::new(&[create_attrs_ix], Some(payer_pk), recent2.parse()?);
        let tx2 = ctx
            .build_and_sign_transaction(msg2, vec![payer_kp, mint_kp])
            .await?;
        let txid2 = ctx.send_transaction(tx2).await?;
        let res2 = ctx.wait_for_transaction(&txid2).await?;
        res2.logs.iter().for_each(|l| println!(">>> {}", l));
        assert_eq!(res2.status, Status::Processed);

        let acct = ctx.read_account_info(attrs_pda).await?;
        let attrs = TokenMetadataAttributes::unpack(&acct.data).unwrap();
        assert!(attrs.is_initialized);
        assert_eq!(attrs.data, data);
        Ok(())
    })
    .await
}

#[tokio::test]
#[serial]
async fn create_attributes_wrong_authority_fails() {
    TestRunner::run(|ctx| async move {
        let program_id = deploy_token_metadata_program(&ctx).await?;
        let (payer_kp, payer_pk, _) = ctx.generate_new_keypair();
        ctx.fund_keypair_with_faucet(&payer_kp).await?;
        let (wrong_kp, wrong_pk, _) = ctx.generate_new_keypair();
        ctx.fund_keypair_with_faucet(&wrong_kp).await?;
        let (mint_kp, mint_pk, _) = ctx.generate_new_keypair();
        let (metadata_pda, _bmd) = find_metadata_pda_with_program(&program_id, &mint_pk);
        let (attrs_pda, _ba) = find_attributes_pda_with_program(&program_id, &mint_pk);

        let create_mint_ix = system_instruction::create_account(
            &payer_pk,
            &mint_pk,
            arch_program::account::MIN_ACCOUNT_LAMPORTS,
            apl_token::state::Mint::LEN as u64,
            &apl_token::id(),
        );
        let init_mint_ix = apl_token::instruction::initialize_mint2(
            &apl_token::id(),
            &mint_pk,
            &payer_pk,
            None,
            9,
        )?;
        let create_md = MetadataInstruction::CreateMetadata {
            name: "N".into(),
            symbol: "S".into(),
            image: "i".into(),
            description: "d".into(),
            immutable: false,
        }
        .pack();
        let create_md_ix = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(payer_pk, true),
                AccountMeta::new_readonly(Pubkey::system_program(), false),
                AccountMeta::new_readonly(mint_pk, false),
                AccountMeta::new(metadata_pda, false),
                AccountMeta::new_readonly(payer_pk, true),
            ],
            data: create_md,
        };

        let create_attrs = MetadataInstruction::CreateAttributes {
            data: vec![("a".into(), "b".into())],
        }
        .pack();
        let create_attrs_ix = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(payer_pk, true),
                AccountMeta::new_readonly(Pubkey::system_program(), false),
                AccountMeta::new_readonly(mint_pk, false),
                AccountMeta::new(attrs_pda, false),
                AccountMeta::new_readonly(wrong_pk, true),
                AccountMeta::new_readonly(metadata_pda, false),
            ],
            data: create_attrs,
        };

        let recent = ctx.get_recent_blockhash().await?;
        let msg = ArchMessage::new(
            &[create_mint_ix, init_mint_ix, create_md_ix, create_attrs_ix],
            Some(payer_pk),
            recent.parse()?,
        );
        let tx = ctx
            .build_and_sign_transaction(msg, vec![payer_kp, mint_kp, wrong_kp])
            .await?;
        let txid = ctx.send_transaction(tx).await?;
        let res = ctx.wait_for_transaction(&txid).await?;
        assert!(matches!(res.status, Status::Failed(_)));
        Ok(())
    })
    .await
}
