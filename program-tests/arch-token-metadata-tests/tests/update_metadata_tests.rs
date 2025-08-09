use apl_token::instruction::initialize_mint2;
use arch_program::{
    account::AccountMeta, instruction::Instruction, program_pack::Pack, pubkey::Pubkey,
    sanitized::ArchMessage, system_instruction,
};
use arch_sdk::Status;
use arch_testing::TestRunner;
use arch_token_metadata::{
    find_metadata_pda_with_program, instruction::MetadataInstruction, state::TokenMetadata,
};
use arch_token_metadata_tests::{create_and_init_mint, deploy_token_metadata_program};
use serial_test::serial;

#[tokio::test]
#[serial]
async fn update_metadata_success() {
    TestRunner::run(|ctx| async move {
        let program_id = deploy_token_metadata_program(&ctx).await?;
        let (payer_kp, payer_pk, _) = ctx.generate_new_keypair();
        ctx.fund_keypair_with_faucet(&payer_kp).await?;

        let (mint_kp, mint_pk, _) = ctx.generate_new_keypair();
        let (metadata_pda, _bump) = find_metadata_pda_with_program(&program_id, &mint_pk);

        // Create and init mint
        let create_mint_ix = system_instruction::create_account(
            &payer_pk,
            &mint_pk,
            arch_program::account::MIN_ACCOUNT_LAMPORTS,
            apl_token::state::Mint::LEN as u64,
            &apl_token::id(),
        );
        let init_mint_ix = initialize_mint2(&apl_token::id(), &mint_pk, &payer_pk, None, 9)?;

        // Create metadata with update_authority = payer
        let create_md = MetadataInstruction::CreateMetadata {
            name: "Name".into(),
            symbol: "SYM".into(),
            image: "https://i".into(),
            description: "desc".into(),
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

        // Update name and symbol
        let upd = MetadataInstruction::UpdateMetadata {
            name: Some("NewName".into()),
            symbol: Some("NS".into()),
            image: None,
            description: None,
        }
        .pack();
        let upd_ix = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(metadata_pda, false),
                AccountMeta::new_readonly(payer_pk, true),
            ],
            data: upd,
        };

        let recent = ctx.get_recent_blockhash().await?;
        let msg = ArchMessage::new(
            &[create_mint_ix, init_mint_ix, create_md_ix, upd_ix],
            Some(payer_pk),
            recent.parse()?,
        );
        let tx = ctx
            .build_and_sign_transaction(msg, vec![payer_kp, mint_kp])
            .await?;
        let txid = ctx.send_transaction(tx).await?;
        let res = ctx.wait_for_transaction(&txid).await?;
        assert_eq!(res.status, Status::Processed);

        let acct = ctx.read_account_info(metadata_pda).await?;
        let md = TokenMetadata::unpack(&acct.data).unwrap();
        assert_eq!(md.name, "NewName");
        assert_eq!(md.symbol, "NS");
        Ok(())
    })
    .await
}

#[tokio::test]
#[serial]
async fn transfer_authority_and_make_immutable() {
    TestRunner::run(|ctx| async move {
        let program_id = deploy_token_metadata_program(&ctx).await?;
        let (payer_kp, payer_pk, _) = ctx.generate_new_keypair();
        ctx.fund_keypair_with_faucet(&payer_kp).await?;

        // Create mint with A as authority
        let (auth_a_kp, auth_a_pk, _) = ctx.generate_new_keypair();
        ctx.fund_keypair_with_faucet(&auth_a_kp).await?;
        let (mint_kp, mint_pk, _) = ctx.generate_new_keypair();
        create_and_init_mint(
            &ctx,
            &payer_kp,
            payer_pk,
            &mint_kp,
            mint_pk,
            &auth_a_pk,
            Some(&auth_a_pk),
        )
        .await?;

        // Derive PDAs
        let (metadata_pda, _bump_md) = find_metadata_pda_with_program(&program_id, &mint_pk);

        // Create metadata (mutable)
        let create_md = MetadataInstruction::CreateMetadata {
            name: "Token".to_string(),
            symbol: "TOK".to_string(),
            image: "i".to_string(),
            description: "d".to_string(),
            immutable: false,
        }
        .pack();
        let ix = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(payer_pk, true),
                AccountMeta::new_readonly(Pubkey::system_program(), false),
                AccountMeta::new_readonly(mint_pk, false),
                AccountMeta::new(metadata_pda, false),
                AccountMeta::new_readonly(auth_a_pk, true),
            ],
            data: create_md,
        };
        let recent = ctx.get_recent_blockhash().await?;
        let msg = ArchMessage::new(&[ix], Some(payer_pk), recent.parse()?);
        let tx = ctx
            .build_and_sign_transaction(msg, vec![payer_kp, auth_a_kp.clone()])
            .await?;
        let txid = ctx.send_transaction(tx).await?;
        let res = ctx.wait_for_transaction(&txid).await?;
        assert_eq!(res.status, Status::Processed);

        // Transfer authority A -> B
        let (auth_b_kp, auth_b_pk, _) = ctx.generate_new_keypair();
        ctx.fund_keypair_with_faucet(&auth_b_kp).await?;
        let xfer = MetadataInstruction::TransferAuthority {
            new_authority: auth_b_pk,
        }
        .pack();
        let ix = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(metadata_pda, false),
                AccountMeta::new_readonly(auth_a_pk, true),
            ],
            data: xfer,
        };
        let recent = ctx.get_recent_blockhash().await?;
        let msg = ArchMessage::new(&[ix], Some(payer_pk), recent.parse()?);
        let tx = ctx
            .build_and_sign_transaction(msg, vec![payer_kp, auth_a_kp.clone()])
            .await?;
        let txid = ctx.send_transaction(tx).await?;
        let res = ctx.wait_for_transaction(&txid).await?;
        assert_eq!(res.status, Status::Processed);

        // Old A can no longer update
        let fail_update = MetadataInstruction::UpdateMetadata {
            name: Some("X".into()),
            symbol: None,
            image: None,
            description: None,
        }
        .pack();
        let ix = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(metadata_pda, false),
                AccountMeta::new_readonly(auth_a_pk, true),
            ],
            data: fail_update,
        };
        let recent = ctx.get_recent_blockhash().await?;
        let msg = ArchMessage::new(&[ix], Some(payer_pk), recent.parse()?);
        let tx = ctx
            .build_and_sign_transaction(msg, vec![payer_kp, auth_a_kp])
            .await?;
        let txid = ctx.send_transaction(tx).await?;
        let res = ctx.wait_for_transaction(&txid).await?;
        assert!(matches!(res.status, Status::Failed(_)));

        // New B can update
        let ok_update = MetadataInstruction::UpdateMetadata {
            name: Some("Y".into()),
            symbol: None,
            image: None,
            description: None,
        }
        .pack();
        let ix = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(metadata_pda, false),
                AccountMeta::new_readonly(auth_b_pk, true),
            ],
            data: ok_update,
        };
        let recent = ctx.get_recent_blockhash().await?;
        let msg = ArchMessage::new(&[ix], Some(payer_pk), recent.parse()?);
        let tx = ctx
            .build_and_sign_transaction(msg, vec![payer_kp, auth_b_kp])
            .await?;
        let txid = ctx.send_transaction(tx).await?;
        let res = ctx.wait_for_transaction(&txid).await?;
        assert_eq!(res.status, Status::Processed);

        // Make immutable by B
        let make_imm = MetadataInstruction::MakeImmutable.pack();
        let ix = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(metadata_pda, false),
                AccountMeta::new_readonly(auth_b_pk, true),
            ],
            data: make_imm,
        };
        let recent = ctx.get_recent_blockhash().await?;
        let msg = ArchMessage::new(&[ix], Some(payer_pk), recent.parse()?);
        let tx = ctx
            .build_and_sign_transaction(msg, vec![payer_kp, auth_b_kp])
            .await?;
        let txid = ctx.send_transaction(tx).await?;
        let res = ctx.wait_for_transaction(&txid).await?;
        assert_eq!(res.status, Status::Processed);

        // Further transfer or update should fail
        let xfer_again = MetadataInstruction::TransferAuthority {
            new_authority: auth_a_pk,
        }
        .pack();
        let ix = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(metadata_pda, false),
                AccountMeta::new_readonly(auth_b_pk, true),
            ],
            data: xfer_again,
        };
        let recent = ctx.get_recent_blockhash().await?;
        let msg = ArchMessage::new(&[ix], Some(payer_pk), recent.parse()?);
        let tx = ctx
            .build_and_sign_transaction(msg, vec![payer_kp, auth_b_kp])
            .await?;
        let txid = ctx.send_transaction(tx).await?;
        let res = ctx.wait_for_transaction(&txid).await?;
        assert!(matches!(res.status, Status::Failed(_)));

        Ok(())
    })
    .await
}

#[tokio::test]
#[serial]
async fn update_metadata_wrong_authority_fails() {
    TestRunner::run(|ctx| async move {
        let program_id = deploy_token_metadata_program(&ctx).await?;
        let (payer_kp, payer_pk, _) = ctx.generate_new_keypair();
        ctx.fund_keypair_with_faucet(&payer_kp).await?;
        let (wrong_kp, wrong_pk, _) = ctx.generate_new_keypair();
        ctx.fund_keypair_with_faucet(&wrong_kp).await?;

        let (mint_kp, mint_pk, _) = ctx.generate_new_keypair();
        let (metadata_pda, _bump) = find_metadata_pda_with_program(&program_id, &mint_pk);

        let create_mint_ix = system_instruction::create_account(
            &payer_pk,
            &mint_pk,
            arch_program::account::MIN_ACCOUNT_LAMPORTS,
            apl_token::state::Mint::LEN as u64,
            &apl_token::id(),
        );
        let init_mint_ix = initialize_mint2(&apl_token::id(), &mint_pk, &payer_pk, None, 9)?;
        let create_md = MetadataInstruction::CreateMetadata {
            name: "Name".into(),
            symbol: "SYM".into(),
            image: "https://i".into(),
            description: "desc".into(),
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

        // Wrong signer tries to update
        let upd = MetadataInstruction::UpdateMetadata {
            name: Some("X".into()),
            symbol: None,
            image: None,
            description: None,
        }
        .pack();
        let upd_ix = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(metadata_pda, false),
                AccountMeta::new_readonly(wrong_pk, true),
            ],
            data: upd,
        };

        let recent = ctx.get_recent_blockhash().await?;
        let msg = ArchMessage::new(
            &[create_mint_ix, init_mint_ix, create_md_ix, upd_ix],
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

#[tokio::test]
#[serial]
async fn update_metadata_immutable_fails() {
    TestRunner::run(|ctx| async move {
        let program_id = deploy_token_metadata_program(&ctx).await?;
        let (payer_kp, payer_pk, _) = ctx.generate_new_keypair();
        ctx.fund_keypair_with_faucet(&payer_kp).await?;

        let (mint_kp, mint_pk, _) = ctx.generate_new_keypair();
        let (metadata_pda, _bump) = find_metadata_pda_with_program(&program_id, &mint_pk);

        let create_mint_ix = system_instruction::create_account(
            &payer_pk,
            &mint_pk,
            arch_program::account::MIN_ACCOUNT_LAMPORTS,
            apl_token::state::Mint::LEN as u64,
            &apl_token::id(),
        );

        let init_mint_ix = initialize_mint2(&apl_token::id(), &mint_pk, &payer_pk, None, 9)?;

        let create_md = MetadataInstruction::CreateMetadata {
            name: "Name".into(),
            symbol: "SYM".into(),
            image: "https://i".into(),
            description: "desc".into(),
            immutable: true,
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

        let upd = MetadataInstruction::UpdateMetadata {
            name: Some("X".into()),
            symbol: None,
            image: None,
            description: None,
        }
        .pack();
        let upd_ix = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(metadata_pda, false),
                AccountMeta::new_readonly(payer_pk, true),
            ],
            data: upd,
        };

        let recent = ctx.get_recent_blockhash().await?;
        let msg = ArchMessage::new(
            &[create_mint_ix, init_mint_ix, create_md_ix, upd_ix],
            Some(payer_pk),
            recent.parse()?,
        );
        let tx = ctx
            .build_and_sign_transaction(msg, vec![payer_kp, mint_kp])
            .await?;
        let txid = ctx.send_transaction(tx).await?;
        let res = ctx.wait_for_transaction(&txid).await?;
        assert!(matches!(res.status, Status::Failed(_)));
        Ok(())
    })
    .await
}

// TransferAuthority should fail if signer is not the current update_authority
#[tokio::test]
#[serial]
async fn transfer_authority_wrong_signer_fails() {
    TestRunner::run(|ctx| async move {
        let program_id = deploy_token_metadata_program(&ctx).await?;
        let (payer_kp, payer_pk, _) = ctx.generate_new_keypair();
        ctx.fund_keypair_with_faucet(&payer_kp).await?;

        // Current authority A, wrong signer W, new target T
        let (auth_a_kp, auth_a_pk, _) = ctx.generate_new_keypair();
        ctx.fund_keypair_with_faucet(&auth_a_kp).await?;
        let (wrong_kp, wrong_pk, _) = ctx.generate_new_keypair();
        ctx.fund_keypair_with_faucet(&wrong_kp).await?;
        let (_target_kp, target_pk, _) = ctx.generate_new_keypair();

        // Create mint under A
        let (mint_kp, mint_pk, _) = ctx.generate_new_keypair();
        create_and_init_mint(
            &ctx,
            &payer_kp,
            payer_pk,
            &mint_kp,
            mint_pk,
            &auth_a_pk,
            Some(&auth_a_pk),
        )
        .await?;

        // Derive metadata PDA
        let (metadata_pda, _bump_md) = find_metadata_pda_with_program(&program_id, &mint_pk);

        // Create metadata (mutable, A is authority)
        let create_md = MetadataInstruction::CreateMetadata {
            name: "Token".to_string(),
            symbol: "TOK".to_string(),
            image: "i".to_string(),
            description: "d".to_string(),
            immutable: false,
        }
        .pack();
        let ix = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(payer_pk, true),
                AccountMeta::new_readonly(Pubkey::system_program(), false),
                AccountMeta::new_readonly(mint_pk, false),
                AccountMeta::new(metadata_pda, false),
                AccountMeta::new_readonly(auth_a_pk, true),
            ],
            data: create_md,
        };
        let recent = ctx.get_recent_blockhash().await?;
        let msg = ArchMessage::new(&[ix], Some(payer_pk), recent.parse()?);
        let tx = ctx
            .build_and_sign_transaction(msg, vec![payer_kp, auth_a_kp.clone()])
            .await?;
        let txid = ctx.send_transaction(tx).await?;
        let res = ctx.wait_for_transaction(&txid).await?;
        assert_eq!(res.status, Status::Processed);

        // Wrong signer attempts transfer A -> T (signed by wrong_pk instead of auth_a_pk)
        let xfer = MetadataInstruction::TransferAuthority {
            new_authority: target_pk,
        }
        .pack();
        let ix = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(metadata_pda, false),
                AccountMeta::new_readonly(wrong_pk, true),
            ],
            data: xfer,
        };
        let recent = ctx.get_recent_blockhash().await?;
        let msg = ArchMessage::new(&[ix], Some(payer_pk), recent.parse()?);
        let tx = ctx
            .build_and_sign_transaction(msg, vec![payer_kp, wrong_kp])
            .await?;
        let txid = ctx.send_transaction(tx).await?;
        let res = ctx.wait_for_transaction(&txid).await?;
        assert!(matches!(res.status, Status::Failed(_)));

        Ok(())
    })
    .await
}

// MakeImmutable should fail if metadata is already immutable
#[tokio::test]
#[serial]
async fn make_immutable_already_immutable_fails() {
    TestRunner::run(|ctx| async move {
        let program_id = deploy_token_metadata_program(&ctx).await?;
        let (payer_kp, payer_pk, _) = ctx.generate_new_keypair();
        ctx.fund_keypair_with_faucet(&payer_kp).await?;

        // Use creator as nominal authority while creating immutable metadata
        let (auth_kp, auth_pk, _) = ctx.generate_new_keypair();
        ctx.fund_keypair_with_faucet(&auth_kp).await?;

        let (mint_kp, mint_pk, _) = ctx.generate_new_keypair();
        create_and_init_mint(
            &ctx,
            &payer_kp,
            payer_pk,
            &mint_kp,
            mint_pk,
            &auth_pk,
            Some(&auth_pk),
        )
        .await?;

        let (metadata_pda, _bump_md) = find_metadata_pda_with_program(&program_id, &mint_pk);

        // Create metadata immutable=true (update_authority=None)
        let create_md = MetadataInstruction::CreateMetadata {
            name: "Name".into(),
            symbol: "SYM".into(),
            image: "i".into(),
            description: "d".into(),
            immutable: true,
        }
        .pack();
        let ix = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(payer_pk, true),
                AccountMeta::new_readonly(Pubkey::system_program(), false),
                AccountMeta::new_readonly(mint_pk, false),
                AccountMeta::new(metadata_pda, false),
                AccountMeta::new_readonly(auth_pk, true),
            ],
            data: create_md,
        };
        let recent = ctx.get_recent_blockhash().await?;
        let msg = ArchMessage::new(&[ix], Some(payer_pk), recent.parse()?);
        let tx = ctx
            .build_and_sign_transaction(msg, vec![payer_kp, auth_kp.clone()])
            .await?;
        let txid = ctx.send_transaction(tx).await?;
        let res = ctx.wait_for_transaction(&txid).await?;
        assert_eq!(res.status, Status::Processed);

        // Attempt MakeImmutable should fail since update_authority is None
        let make_imm = MetadataInstruction::MakeImmutable.pack();
        let ix = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(metadata_pda, false),
                AccountMeta::new_readonly(auth_pk, true),
            ],
            data: make_imm,
        };
        let recent = ctx.get_recent_blockhash().await?;
        let msg = ArchMessage::new(&[ix], Some(payer_pk), recent.parse()?);
        let tx = ctx
            .build_and_sign_transaction(msg, vec![payer_kp, auth_kp])
            .await?;
        let txid = ctx.send_transaction(tx).await?;
        let res = ctx.wait_for_transaction(&txid).await?;
        assert!(matches!(res.status, Status::Failed(_)));

        Ok(())
    })
    .await
}
