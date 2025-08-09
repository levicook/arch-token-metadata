use arch_program::{
    account::{AccountMeta, MIN_ACCOUNT_LAMPORTS},
    instruction::Instruction,
    program_pack::Pack,
    pubkey::Pubkey,
    sanitized::ArchMessage,
    system_instruction,
};
use arch_sdk::Status;
use arch_testing::TestRunner;
use arch_token_metadata::find_metadata_pda_with_program;
use arch_token_metadata::instruction::MetadataInstruction;
use arch_token_metadata::state::TokenMetadata;
use arch_token_metadata_tests::deploy_token_metadata_program;
use serial_test::serial;

// Mint authority rotation: after rotating A -> B, old A must fail
#[tokio::test]
#[serial]
async fn create_metadata_mint_authority_rotation_old_fails() {
    TestRunner::run(|ctx| async move {
        let program_id = deploy_token_metadata_program(&ctx).await?;

        let (payer_kp, payer_pk, _) = ctx.generate_new_keypair();
        ctx.fund_keypair_with_faucet(&payer_kp).await?;

        // old authority A and new authority B
        let (auth_a_kp, auth_a_pk, _) = ctx.generate_new_keypair();
        ctx.fund_keypair_with_faucet(&auth_a_kp).await?;
        let (auth_b_kp, auth_b_pk, _) = ctx.generate_new_keypair();
        ctx.fund_keypair_with_faucet(&auth_b_kp).await?;

        // Create mint with A as mint authority
        let (mint_kp, mint_pk, _) = ctx.generate_new_keypair();
        let (metadata_pda, _bump) = find_metadata_pda_with_program(&program_id, &mint_pk);

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
            &auth_a_pk,
            None,
            9,
        )?;

        // Rotate mint authority A -> B, signed by A
        let rotate_to_b_ix = apl_token::instruction::set_authority(
            &apl_token::id(),
            &mint_pk,
            Some(&auth_b_pk),
            apl_token::instruction::AuthorityType::MintTokens,
            &auth_a_pk,
            &[],
        )?;

        // Now try to create metadata signed by OLD authority A (should fail)
        let ix_data = MetadataInstruction::CreateMetadata {
            name: "RotateOldFail".to_string(),
            symbol: "ROF".to_string(),
            image: "https://example.com/rof.png".to_string(),
            description: "old fails".to_string(),
            immutable: false,
        };
        let data = ix_data.pack();
        let accounts = vec![
            AccountMeta::new(payer_pk, true),
            AccountMeta::new_readonly(Pubkey::system_program(), false),
            AccountMeta::new_readonly(mint_pk, false),
            AccountMeta::new(metadata_pda, false),
            AccountMeta::new_readonly(auth_a_pk, true),
        ];
        let metadata_ix = Instruction {
            program_id,
            accounts,
            data,
        };

        let recent = ctx.get_recent_blockhash().await?;
        let message = ArchMessage::new(
            &[create_mint_ix, init_mint_ix, rotate_to_b_ix, metadata_ix],
            Some(payer_pk),
            recent.parse()?,
        );
        let tx = ctx
            .build_and_sign_transaction(message, vec![payer_kp, mint_kp, auth_a_kp])
            .await?;
        let txid = ctx.send_transaction(tx).await?;
        let res = ctx.wait_for_transaction(&txid).await?;
        assert!(matches!(res.status, Status::Failed(_)));
        Ok(())
    })
    .await
}

// Mint authority rotation: after rotating A -> B, new B must succeed
#[tokio::test]
#[serial]
async fn create_metadata_mint_authority_rotation_new_succeeds() {
    TestRunner::run(|ctx| async move {
        let program_id = deploy_token_metadata_program(&ctx).await?;

        let (payer_kp, payer_pk, _) = ctx.generate_new_keypair();
        ctx.fund_keypair_with_faucet(&payer_kp).await?;

        // old authority A and new authority B
        let (auth_a_kp, auth_a_pk, _) = ctx.generate_new_keypair();
        ctx.fund_keypair_with_faucet(&auth_a_kp).await?;

        let (auth_b_kp, auth_b_pk, _) = ctx.generate_new_keypair();
        ctx.fund_keypair_with_faucet(&auth_b_kp).await?;

        // Create mint with A as mint authority
        let (mint_kp, mint_pk, _) = ctx.generate_new_keypair();
        let (metadata_pda, _bump) = find_metadata_pda_with_program(&program_id, &mint_pk);

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
            &auth_a_pk,
            None,
            9,
        )?;

        // Rotate mint authority A -> B, signed by A
        let rotate_to_b_ix = apl_token::instruction::set_authority(
            &apl_token::id(),
            &mint_pk,
            Some(&auth_b_pk),
            apl_token::instruction::AuthorityType::MintTokens,
            &auth_a_pk,
            &[],
        )?;

        // Now create metadata signed by NEW authority B (should succeed)
        let ix_data = MetadataInstruction::CreateMetadata {
            name: "RotateNewOk".to_string(),
            symbol: "RNO".to_string(),
            image: "https://example.com/rno.png".to_string(),
            description: "new ok".to_string(),
            immutable: false,
        };
        let data = ix_data.pack();
        let accounts = vec![
            AccountMeta::new(payer_pk, true),
            AccountMeta::new_readonly(Pubkey::system_program(), false),
            AccountMeta::new_readonly(mint_pk, false),
            AccountMeta::new(metadata_pda, false),
            AccountMeta::new_readonly(auth_b_pk, true),
        ];
        let metadata_ix = Instruction {
            program_id,
            accounts,
            data,
        };

        let recent = ctx.get_recent_blockhash().await?;
        let message = ArchMessage::new(
            &[create_mint_ix, init_mint_ix, rotate_to_b_ix, metadata_ix],
            Some(payer_pk),
            recent.parse()?,
        );
        let tx = ctx
            .build_and_sign_transaction(message, vec![payer_kp, mint_kp, auth_a_kp, auth_b_kp])
            .await?;
        let txid = ctx.send_transaction(tx).await?;
        let res = ctx.wait_for_transaction(&txid).await?;
        assert_eq!(res.status, Status::Processed);

        let acct = ctx.read_account_info(metadata_pda).await?;
        let md = TokenMetadata::unpack(&acct.data).expect("unpack metadata");
        assert!(md.is_initialized);
        assert_eq!(md.mint, mint_pk);
        assert_eq!(md.update_authority, Some(auth_b_pk));
        Ok(())
    })
    .await
}

// If mint_authority exists, freeze authority cannot be used to create metadata
#[tokio::test]
#[serial]
async fn create_metadata_freeze_auth_rejected_when_mint_auth_present() {
    TestRunner::run(|ctx| async move {
        let program_id = deploy_token_metadata_program(&ctx).await?;
        let (payer_kp, payer_pk, _) = ctx.generate_new_keypair();
        ctx.fund_keypair_with_faucet(&payer_kp).await?;
        let (freeze_kp, freeze_pk, _) = ctx.generate_new_keypair();
        ctx.fund_keypair_with_faucet(&freeze_kp).await?;

        let (mint_kp, mint_pk, _) = ctx.generate_new_keypair();
        let (metadata_pda, _bump) = find_metadata_pda_with_program(&program_id, &mint_pk);

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
            &payer_pk,        // mint authority present
            Some(&freeze_pk), // freeze authority set
            9,
        )?;

        let ix_data = MetadataInstruction::CreateMetadata {
            name: "FreezeRejected".to_string(),
            symbol: "FR".to_string(),
            image: "https://example.com/fr.png".to_string(),
            description: "fr".to_string(),
            immutable: false,
        };
        let data = ix_data.pack();
        let accounts = vec![
            AccountMeta::new(payer_pk, true),
            AccountMeta::new_readonly(Pubkey::system_program(), false),
            AccountMeta::new_readonly(mint_pk, false),
            AccountMeta::new(metadata_pda, false),
            AccountMeta::new_readonly(freeze_pk, true), // attempt with freeze signer while mint auth exists
        ];
        let metadata_ix = Instruction {
            program_id,
            accounts,
            data,
        };

        let recent = ctx.get_recent_blockhash().await?;
        let message = ArchMessage::new(
            &[create_mint_ix, init_mint_ix, metadata_ix],
            Some(payer_pk),
            recent.parse()?,
        );
        let tx = ctx
            .build_and_sign_transaction(message, vec![payer_kp, mint_kp, freeze_kp])
            .await?;
        let txid = ctx.send_transaction(tx).await?;
        let res = ctx.wait_for_transaction(&txid).await?;
        assert!(matches!(res.status, Status::Failed(_)));
        Ok(())
    })
    .await
}

// When both mint and freeze authorities are None, creation must fail
#[tokio::test]
#[serial]
async fn create_metadata_no_authority_fails() {
    TestRunner::run(|ctx| async move {
        let program_id = deploy_token_metadata_program(&ctx).await?;
        let (payer_kp, payer_pk, _) = ctx.generate_new_keypair();
        ctx.fund_keypair_with_faucet(&payer_kp).await?;

        let (mint_kp, mint_pk, _) = ctx.generate_new_keypair();
        let (metadata_pda, _bump) = find_metadata_pda_with_program(&program_id, &mint_pk);

        let create_mint_ix = system_instruction::create_account(
            &payer_pk,
            &mint_pk,
            MIN_ACCOUNT_LAMPORTS,
            apl_token::state::Mint::LEN as u64,
            &apl_token::id(),
        );
        // Initialize with payer as mint authority, then remove both mint and freeze
        let init_mint_ix = apl_token::instruction::initialize_mint2(
            &apl_token::id(),
            &mint_pk,
            &payer_pk,
            None,
            9,
        )?;
        let clear_mint_auth = apl_token::instruction::set_authority(
            &apl_token::id(),
            &mint_pk,
            None,
            apl_token::instruction::AuthorityType::MintTokens,
            &payer_pk,
            &[],
        )?;

        let ix_data = MetadataInstruction::CreateMetadata {
            name: "NoAuth".to_string(),
            symbol: "NA".to_string(),
            image: "https://example.com/na.png".to_string(),
            description: "no auth".to_string(),
            immutable: false,
        };
        let data = ix_data.pack();
        // Attempt with payer as signer, but mint has no authorities
        let accounts = vec![
            AccountMeta::new(payer_pk, true),
            AccountMeta::new_readonly(Pubkey::system_program(), false),
            AccountMeta::new_readonly(mint_pk, false),
            AccountMeta::new(metadata_pda, false),
            AccountMeta::new_readonly(payer_pk, true),
        ];
        let metadata_ix = Instruction {
            program_id,
            accounts,
            data,
        };

        let recent = ctx.get_recent_blockhash().await?;
        let message = ArchMessage::new(
            &[create_mint_ix, init_mint_ix, clear_mint_auth, metadata_ix],
            Some(payer_pk),
            recent.parse()?,
        );
        let tx = ctx
            .build_and_sign_transaction(message, vec![payer_kp, mint_kp])
            .await?;
        let txid = ctx.send_transaction(tx).await?;
        let res = ctx.wait_for_transaction(&txid).await?;
        assert!(matches!(res.status, Status::Failed(_)));
        Ok(())
    })
    .await
}

// Success when mint_authority is None and freeze_authority signs
#[tokio::test]
#[serial]
async fn create_metadata_mint_authority_success() {
    TestRunner::run(|ctx| async move {
        let program_id = deploy_token_metadata_program(&ctx).await?;

        let (payer_kp, payer_pk, _) = ctx.generate_new_keypair();
        let (mint_kp, mint_pk, _) = ctx.generate_new_keypair();
        let (metadata_pda, _bump) = find_metadata_pda_with_program(&program_id, &mint_pk);

        // Fund payer
        ctx.fund_keypair_with_faucet(&payer_kp).await?;

        // Create and initialize a real mint owned by the token program
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
            &payer_pk, // mint authority
            None,      // no freeze authority
            9,         // decimals
        )
        .expect("init mint ix");

        // Program will create the metadata PDA via CPI; we just pass payer + system_program
        // Build CreateMetadata instruction data via Borsh
        let ix_data = MetadataInstruction::CreateMetadata {
            name: "Arch Pioneer Token".to_string(),
            symbol: "APT".to_string(),
            image: "https://arweave.net/abc123.png".to_string(),
            description: "The first token launched on Arch Network".to_string(),
            immutable: false,
        };
        let data = ix_data.pack();

        // Accounts: [
        //   payer (writable, signer),
        //   system_program,
        //   mint,
        //   metadata_pda (writable),
        //   mint_authority (signer)
        // ]
        let accounts = vec![
            AccountMeta::new(payer_pk, true),
            AccountMeta::new_readonly(Pubkey::system_program(), false),
            AccountMeta::new_readonly(mint_pk, false),
            AccountMeta::new(metadata_pda, false),
            AccountMeta::new_readonly(payer_pk, true),
        ];

        let metadata_ix = Instruction {
            program_id,
            accounts,
            data,
        };

        let recent = ctx.get_recent_blockhash().await?;

        let message = ArchMessage::new(
            &[create_mint_ix, init_mint_ix, metadata_ix], //
            Some(payer_pk),
            recent.parse()?,
        );

        let tx = ctx
            .build_and_sign_transaction(message, vec![payer_kp, mint_kp])
            .await?;

        let txid = ctx.send_transaction(tx).await?;
        let res = ctx.wait_for_transaction(&txid).await?;
        assert_eq!(res.status, Status::Processed);

        // Read back metadata and assert fields
        let acct = ctx.read_account_info(metadata_pda).await?;
        let md = TokenMetadata::unpack(&acct.data).expect("unpack metadata");
        assert!(md.is_initialized);
        assert_eq!(md.mint, mint_pk);
        assert_eq!(md.name, "Arch Pioneer Token");
        assert_eq!(md.symbol, "APT");
        assert_eq!(md.image, "https://arweave.net/abc123.png");
        assert_eq!(md.description, "The first token launched on Arch Network");
        assert_eq!(md.update_authority, Some(payer_pk));

        Ok(())
    })
    .await
}

// Success when mint_authority is None and freeze_authority signs
#[tokio::test]
#[serial]
async fn create_metadata_freeze_authority_success() {
    TestRunner::run(|ctx| async move {
        let program_id = deploy_token_metadata_program(&ctx).await?;

        let (payer_kp, payer_pk, _) = ctx.generate_new_keypair();
        ctx.fund_keypair_with_faucet(&payer_kp).await?;

        // Freeze authority will be a different key
        let (freeze_kp, freeze_pk, _) = ctx.generate_new_keypair();
        ctx.fund_keypair_with_faucet(&freeze_kp).await?;

        // Create mint
        let (mint_kp, mint_pk, _) = ctx.generate_new_keypair();

        let (metadata_pda, _bump) = find_metadata_pda_with_program(&program_id, &mint_pk);

        let create_mint_ix = system_instruction::create_account(
            &payer_pk,
            &mint_pk,
            MIN_ACCOUNT_LAMPORTS,
            apl_token::state::Mint::LEN as u64,
            &apl_token::id(),
        );

        // initialize mint with payer as mint authority and freeze authority set
        let init_mint_ix = apl_token::instruction::initialize_mint2(
            &apl_token::id(),
            &mint_pk,
            &payer_pk, // initial mint authority is payer
            Some(&freeze_pk),
            9,
        )
        .expect("init mint ix");

        // Now set mint authority to None using token program, signed by current mint authority (payer)
        let set_mint_auth_none_ix = apl_token::instruction::set_authority(
            &apl_token::id(),
            &mint_pk,
            None,
            apl_token::instruction::AuthorityType::MintTokens,
            &payer_pk,
            &[],
        )
        .expect("set mint auth none");

        let ix_data = MetadataInstruction::CreateMetadata {
            name: "Freeze Authority Token".to_string(),
            symbol: "FZ".to_string(),
            image: "https://example.com/fz.png".to_string(),
            description: "freeze auth creates".to_string(),
            immutable: false,
        };
        let data = ix_data.pack();

        let accounts = vec![
            AccountMeta::new(payer_pk, true),
            AccountMeta::new_readonly(Pubkey::system_program(), false),
            AccountMeta::new_readonly(mint_pk, false),
            AccountMeta::new(metadata_pda, false),
            AccountMeta::new_readonly(freeze_pk, true),
        ];
        let metadata_ix = Instruction {
            program_id,
            accounts,
            data,
        };

        let recent = ctx.get_recent_blockhash().await?;
        let message = ArchMessage::new(
            &[
                create_mint_ix,
                init_mint_ix,
                set_mint_auth_none_ix,
                metadata_ix,
            ],
            Some(payer_pk),
            recent.parse()?,
        );
        let tx = ctx
            .build_and_sign_transaction(message, vec![payer_kp, mint_kp, freeze_kp])
            .await?;
        let txid = ctx.send_transaction(tx).await?;
        let res = ctx.wait_for_transaction(&txid).await?;
        assert_eq!(res.status, arch_sdk::Status::Processed);

        let acct = ctx.read_account_info(metadata_pda).await?;
        let md = TokenMetadata::unpack(&acct.data).expect("unpack metadata");
        assert!(md.is_initialized);
        assert_eq!(md.mint, mint_pk);
        assert_eq!(md.update_authority, Some(freeze_pk));
        Ok(())
    })
    .await
}

// Failure when wrong signer tries to create metadata
#[tokio::test]
#[serial]
async fn create_metadata_wrong_signer_fails() {
    TestRunner::run(|ctx| async move {
        let program_id = deploy_token_metadata_program(&ctx).await?;

        // Failure when wrong signer tries to create metadata
        let (payer_kp, payer_pk, _) = ctx.generate_new_keypair();
        ctx.fund_keypair_with_faucet(&payer_kp).await?;
        let (wrong_kp, wrong_pk, _) = ctx.generate_new_keypair();
        ctx.fund_keypair_with_faucet(&wrong_kp).await?;

        let (mint_kp, mint_pk, _) = ctx.generate_new_keypair();
        let (metadata_pda, _bump) = find_metadata_pda_with_program(&program_id, &mint_pk);

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
            &payer_pk,
            None,
            9,
        )
        .expect("init mint ix");

        let ix_data = MetadataInstruction::CreateMetadata {
            name: "Wrong Signer".to_string(),
            symbol: "WS".to_string(),
            image: "https://example.com/ws.png".to_string(),
            description: "should fail".to_string(),
            immutable: false,
        };
        let data = ix_data.pack();
        let accounts = vec![
            AccountMeta::new(payer_pk, true),
            AccountMeta::new_readonly(Pubkey::system_program(), false),
            AccountMeta::new_readonly(mint_pk, false),
            AccountMeta::new(metadata_pda, false),
            AccountMeta::new_readonly(wrong_pk, true),
        ];
        let metadata_ix = Instruction {
            program_id,
            accounts,
            data,
        };

        let recent = ctx.get_recent_blockhash().await?;
        let message = ArchMessage::new(
            &[create_mint_ix, init_mint_ix, metadata_ix],
            Some(payer_pk),
            recent.parse()?,
        );
        let tx = ctx
            .build_and_sign_transaction(message, vec![payer_kp, mint_kp, wrong_kp])
            .await?;
        let txid = ctx.send_transaction(tx).await?;
        let res = ctx.wait_for_transaction(&txid).await?;
        assert!(matches!(res.status, arch_sdk::Status::Failed(_)));
        Ok(())
    })
    .await
}

// Failure on duplicate create
#[tokio::test]
#[serial]
async fn create_metadata_duplicate_fails() {
    TestRunner::run(|ctx| async move {
        let program_id = deploy_token_metadata_program(&ctx).await?;
        // Failure on duplicate create
        let (payer_kp, payer_pk, _) = ctx.generate_new_keypair();
        ctx.fund_keypair_with_faucet(&payer_kp).await?;
        let (mint_kp, mint_pk, _) = ctx.generate_new_keypair();
        let (metadata_pda, _bump) = find_metadata_pda_with_program(&program_id, &mint_pk);

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
            &payer_pk,
            None,
            9,
        )
        .expect("init mint ix");

        let ix_data = MetadataInstruction::CreateMetadata {
            name: "Dup".to_string(),
            symbol: "DUP".to_string(),
            image: "https://example.com/dup.png".to_string(),
            description: "dup test".to_string(),
            immutable: false,
        };
        let data = ix_data.pack();
        let accounts = vec![
            AccountMeta::new(payer_pk, true),
            AccountMeta::new_readonly(Pubkey::system_program(), false),
            AccountMeta::new_readonly(mint_pk, false),
            AccountMeta::new(metadata_pda, false),
            AccountMeta::new_readonly(payer_pk, true),
        ];
        let metadata_ix = Instruction {
            program_id,
            accounts,
            data: data.clone(),
        };

        let recent = ctx.get_recent_blockhash().await?;
        let message1 = ArchMessage::new(
            &[
                create_mint_ix.clone(),
                init_mint_ix.clone(),
                metadata_ix.clone(),
            ],
            Some(payer_pk),
            recent.parse()?,
        );
        let tx1 = ctx
            .build_and_sign_transaction(message1, vec![payer_kp.clone(), mint_kp.clone()])
            .await?;
        let txid1 = ctx.send_transaction(tx1).await?;
        let res1 = ctx.wait_for_transaction(&txid1).await?;
        assert_eq!(res1.status, arch_sdk::Status::Processed);

        // Send duplicate create
        let recent2 = ctx.get_recent_blockhash().await?;
        let message2 = ArchMessage::new(&[metadata_ix], Some(payer_pk), recent2.parse()?);
        let tx2 = ctx
            .build_and_sign_transaction(message2, vec![payer_kp])
            .await?;
        let txid2 = ctx.send_transaction(tx2).await?;
        let res2 = ctx.wait_for_transaction(&txid2).await?;
        assert!(matches!(res2.status, arch_sdk::Status::Failed(_)));
        Ok(())
    })
    .await
}

// Success when immutable is true
#[tokio::test]
#[serial]
async fn create_metadata_immutable_success() {
    TestRunner::run(|ctx| async move {
        let program_id = deploy_token_metadata_program(&ctx).await?;

        // Immutable success: update_authority must be None
        let (payer_kp, payer_pk, _) = ctx.generate_new_keypair();
        ctx.fund_keypair_with_faucet(&payer_kp).await?;
        let (mint_kp, mint_pk, _) = ctx.generate_new_keypair();
        let (metadata_pda, _bump) = find_metadata_pda_with_program(&program_id, &mint_pk);

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
            &payer_pk,
            None,
            9,
        )?;

        let ix_data = MetadataInstruction::CreateMetadata {
            name: "Immutable Token".to_string(),
            symbol: "IMM".to_string(),
            image: "https://example.com/imm.png".to_string(),
            description: "immutable".to_string(),
            immutable: true,
        };
        let data = ix_data.pack();
        let accounts = vec![
            AccountMeta::new(payer_pk, true),
            AccountMeta::new_readonly(Pubkey::system_program(), false),
            AccountMeta::new_readonly(mint_pk, false),
            AccountMeta::new(metadata_pda, false),
            AccountMeta::new_readonly(payer_pk, true),
        ];
        let metadata_ix = Instruction {
            program_id,
            accounts,
            data,
        };

        let recent = ctx.get_recent_blockhash().await?;
        let message = ArchMessage::new(
            &[create_mint_ix, init_mint_ix, metadata_ix],
            Some(payer_pk),
            recent.parse()?,
        );
        let tx = ctx
            .build_and_sign_transaction(message, vec![payer_kp, mint_kp])
            .await?;
        let txid = ctx.send_transaction(tx).await?;
        let res = ctx.wait_for_transaction(&txid).await?;
        assert_eq!(res.status, Status::Processed);

        let acct = ctx.read_account_info(metadata_pda).await?;
        let md = TokenMetadata::unpack(&acct.data).expect("unpack metadata");
        assert!(md.is_initialized);
        assert_eq!(md.update_authority, None);
        Ok(())
    })
    .await
}

// Failure when wrong system program tries to create metadata
#[tokio::test]
#[serial]
async fn create_metadata_wrong_system_program_fails() {
    TestRunner::run(|ctx| async move {
        let program_id = deploy_token_metadata_program(&ctx).await?;
        // Wrong system program fails
        let (payer_kp, payer_pk, _) = ctx.generate_new_keypair();
        ctx.fund_keypair_with_faucet(&payer_kp).await?;
        let (mint_kp, mint_pk, _) = ctx.generate_new_keypair();
        let (metadata_pda, _bump) = find_metadata_pda_with_program(&program_id, &mint_pk);

        // Proper mint init
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
            &payer_pk,
            None,
            9,
        )?;

        let ix_data = MetadataInstruction::CreateMetadata {
            name: "BadSys".to_string(),
            symbol: "BS".to_string(),
            image: "https://example.com/bs.png".to_string(),
            description: "bad sys".to_string(),
            immutable: false,
        };
        let data = ix_data.pack();
        let bogus_sys = Pubkey::from_slice(&[9u8; 32]);
        let accounts = vec![
            AccountMeta::new(payer_pk, true),
            AccountMeta::new_readonly(bogus_sys, false), // wrong system program
            AccountMeta::new_readonly(mint_pk, false),
            AccountMeta::new(metadata_pda, false),
            AccountMeta::new_readonly(payer_pk, true),
        ];
        let metadata_ix = Instruction {
            program_id,
            accounts,
            data,
        };

        let recent = ctx.get_recent_blockhash().await?;
        let message = ArchMessage::new(
            &[create_mint_ix, init_mint_ix, metadata_ix],
            Some(payer_pk),
            recent.parse()?,
        );
        let tx = ctx
            .build_and_sign_transaction(message, vec![payer_kp, mint_kp])
            .await?;
        let txid = ctx.send_transaction(tx).await?;
        let res = ctx.wait_for_transaction(&txid).await?;
        assert!(matches!(res.status, Status::Failed(_)));
        Ok(())
    })
    .await
}

// Mint wrong owner fails (owner != token program)
#[tokio::test]
#[serial]
async fn create_metadata_mint_wrong_owner_fails() {
    TestRunner::run(|ctx| async move {
        let program_id = deploy_token_metadata_program(&ctx).await?;

        let (payer_kp, payer_pk, _) = ctx.generate_new_keypair();
        ctx.fund_keypair_with_faucet(&payer_kp).await?;
        let (mint_kp, mint_pk, _) = ctx.generate_new_keypair();
        let (metadata_pda, _bump) = find_metadata_pda_with_program(&program_id, &mint_pk);

        // Create mint as a system-owned account (not token program)
        let create_mint_ix = system_instruction::create_account(
            &payer_pk,
            &mint_pk,
            MIN_ACCOUNT_LAMPORTS,
            0,
            &Pubkey::system_program(),
        );

        let ix_data = MetadataInstruction::CreateMetadata {
            name: "WrongOwner".to_string(),
            symbol: "WO".to_string(),
            image: "https://example.com/wo.png".to_string(),
            description: "wo".to_string(),
            immutable: false,
        };
        let data = ix_data.pack();
        let accounts = vec![
            AccountMeta::new(payer_pk, true),
            AccountMeta::new_readonly(Pubkey::system_program(), false),
            AccountMeta::new_readonly(mint_pk, false),
            AccountMeta::new(metadata_pda, false),
            AccountMeta::new_readonly(payer_pk, true),
        ];
        let metadata_ix = Instruction {
            program_id,
            accounts,
            data,
        };

        let recent = ctx.get_recent_blockhash().await?;
        let message = ArchMessage::new(
            &[create_mint_ix, metadata_ix],
            Some(payer_pk),
            recent.parse()?,
        );
        let tx = ctx
            .build_and_sign_transaction(message, vec![payer_kp, mint_kp])
            .await?;
        let txid = ctx.send_transaction(tx).await?;
        let res = ctx.wait_for_transaction(&txid).await?;
        assert!(matches!(res.status, Status::Failed(_)));
        Ok(())
    })
    .await
}

// Uninitialized mint fails (owned by token program, but not initialized)
#[tokio::test]
#[serial]
async fn create_metadata_uninitialized_mint_fails() {
    TestRunner::run(|ctx| async move {
        let program_id = deploy_token_metadata_program(&ctx).await?;
        let (payer_kp, payer_pk, _) = ctx.generate_new_keypair();
        ctx.fund_keypair_with_faucet(&payer_kp).await?;
        let (mint_kp, mint_pk, _) = ctx.generate_new_keypair();
        let (metadata_pda, _bump) = find_metadata_pda_with_program(&program_id, &mint_pk);

        // Create mint owned by token program but do NOT initialize
        let create_mint_ix = system_instruction::create_account(
            &payer_pk,
            &mint_pk,
            MIN_ACCOUNT_LAMPORTS,
            apl_token::state::Mint::LEN as u64,
            &apl_token::id(),
        );

        let ix_data = MetadataInstruction::CreateMetadata {
            name: "Uninit".to_string(),
            symbol: "UI".to_string(),
            image: "https://example.com/ui.png".to_string(),
            description: "ui".to_string(),
            immutable: false,
        };
        let data = ix_data.pack();
        let accounts = vec![
            AccountMeta::new(payer_pk, true),
            AccountMeta::new_readonly(Pubkey::system_program(), false),
            AccountMeta::new_readonly(mint_pk, false),
            AccountMeta::new(metadata_pda, false),
            AccountMeta::new_readonly(payer_pk, true),
        ];
        let metadata_ix = Instruction {
            program_id,
            accounts,
            data,
        };

        let recent = ctx.get_recent_blockhash().await?;
        let message = ArchMessage::new(
            &[create_mint_ix, metadata_ix],
            Some(payer_pk),
            recent.parse()?,
        );
        let tx = ctx
            .build_and_sign_transaction(message, vec![payer_kp, mint_kp])
            .await?;
        let txid = ctx.send_transaction(tx).await?;
        let res = ctx.wait_for_transaction(&txid).await?;
        assert!(matches!(res.status, Status::Failed(_)));
        Ok(())
    })
    .await
}

// PDA mismatch fails
#[tokio::test]
#[serial]
async fn create_metadata_pda_mismatch_fails() {
    TestRunner::run(|ctx| async move {
        let program_id = deploy_token_metadata_program(&ctx).await?;

        let (payer_kp, payer_pk, _) = ctx.generate_new_keypair();
        ctx.fund_keypair_with_faucet(&payer_kp).await?;
        let (mint_kp, mint_pk, _) = ctx.generate_new_keypair();
        let (_metadata_pda, _bump) = find_metadata_pda_with_program(&program_id, &mint_pk);
        let (wrong_meta_kp, wrong_meta_pk, _) = ctx.generate_new_keypair();

        // Proper mint
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
            &payer_pk,
            None,
            9,
        )?;

        // Create a normal system-owned account to act as wrong metadata address
        let create_wrong_meta_ix = system_instruction::create_account(
            &payer_pk,
            &wrong_meta_pk,
            MIN_ACCOUNT_LAMPORTS,
            0,
            &Pubkey::system_program(),
        );

        let ix_data = MetadataInstruction::CreateMetadata {
            name: "PDABad".to_string(),
            symbol: "PB".to_string(),
            image: "https://example.com/pb.png".to_string(),
            description: "pda bad".to_string(),
            immutable: false,
        };
        let data = ix_data.pack();
        let accounts = vec![
            AccountMeta::new(payer_pk, true),
            AccountMeta::new_readonly(Pubkey::system_program(), false),
            AccountMeta::new_readonly(mint_pk, false),
            AccountMeta::new(wrong_meta_pk, false), // not the correct PDA
            AccountMeta::new_readonly(payer_pk, true),
        ];
        let metadata_ix = Instruction {
            program_id,
            accounts,
            data,
        };

        let recent = ctx.get_recent_blockhash().await?;
        let message = ArchMessage::new(
            &[
                create_mint_ix,
                init_mint_ix,
                create_wrong_meta_ix,
                metadata_ix,
            ],
            Some(payer_pk),
            recent.parse()?,
        );
        let tx = ctx
            .build_and_sign_transaction(message, vec![payer_kp, mint_kp, wrong_meta_kp])
            .await?;
        let txid = ctx.send_transaction(tx).await?;
        let res = ctx.wait_for_transaction(&txid).await?;
        assert!(matches!(res.status, Status::Failed(_)));
        Ok(())
    })
    .await
}

// Field caps exceeded fails for name and symbol (sample); extend similarly for image/description
#[tokio::test]
#[serial]
async fn create_metadata_field_caps_exceeded_fails() {
    TestRunner::run(|ctx| async move {
        let program_id = deploy_token_metadata_program(&ctx).await?;

        // Field caps exceeded fails for name and symbol (sample); extend similarly for image/description
        let (payer_kp, payer_pk, _) = ctx.generate_new_keypair();
        ctx.fund_keypair_with_faucet(&payer_kp).await?;
        let (mint_kp, mint_pk, _) = ctx.generate_new_keypair();
        let (metadata_pda, _bump) = find_metadata_pda_with_program(&program_id, &mint_pk);

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
            &payer_pk,
            None,
            9,
        )?;

        // Oversized name
        let big_name = "N".repeat(257);
        let ix_data = MetadataInstruction::CreateMetadata {
            name: big_name,
            symbol: "OK".to_string(),
            image: "https://example.com/i.png".to_string(),
            description: "d".to_string(),
            immutable: false,
        };
        let data = ix_data.pack();
        let accounts = vec![
            AccountMeta::new(payer_pk, true),
            AccountMeta::new_readonly(Pubkey::system_program(), false),
            AccountMeta::new_readonly(mint_pk, false),
            AccountMeta::new(metadata_pda, false),
            AccountMeta::new_readonly(payer_pk, true),
        ];
        let bad_name_ix = Instruction {
            program_id,
            accounts,
            data,
        };

        // Oversized symbol
        let (metadata_pda2, _bump2) = find_metadata_pda_with_program(&program_id, &mint_pk);
        let ix_data2 = MetadataInstruction::CreateMetadata {
            name: "OK".to_string(),
            symbol: "S".repeat(17),
            image: "https://example.com/i.png".to_string(),
            description: "d".to_string(),
            immutable: false,
        };
        let data2 = ix_data2.pack();
        let accounts2 = vec![
            AccountMeta::new(payer_pk, true),
            AccountMeta::new_readonly(Pubkey::system_program(), false),
            AccountMeta::new_readonly(mint_pk, false),
            AccountMeta::new(metadata_pda2, false),
            AccountMeta::new_readonly(payer_pk, true),
        ];
        let bad_symbol_ix = Instruction {
            program_id,
            accounts: accounts2,
            data: data2,
        };

        let recent = ctx.get_recent_blockhash().await?;
        let message = ArchMessage::new(
            &[create_mint_ix, init_mint_ix, bad_name_ix, bad_symbol_ix],
            Some(payer_pk),
            recent.parse()?,
        );
        let tx = ctx
            .build_and_sign_transaction(message, vec![payer_kp, mint_kp])
            .await?;
        let txid = ctx.send_transaction(tx).await?;
        let res = ctx.wait_for_transaction(&txid).await?;
        assert!(matches!(res.status, Status::Failed(_)));
        Ok(())
    })
    .await
}

// Image length > IMAGE_MAX_LEN fails
#[tokio::test]
#[serial]
async fn create_metadata_image_length_exceeded_fails() {
    TestRunner::run(|ctx| async move {
        let program_id = deploy_token_metadata_program(&ctx).await?;

        let (payer_kp, payer_pk, _) = ctx.generate_new_keypair();
        ctx.fund_keypair_with_faucet(&payer_kp).await?;
        let (mint_kp, mint_pk, _) = ctx.generate_new_keypair();
        let (metadata_pda, _bump) = find_metadata_pda_with_program(&program_id, &mint_pk);

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
            &payer_pk,
            None,
            9,
        )?;

        let big_image = format!("https://example.com/{}.png", "i".repeat(513));
        let ix_data = MetadataInstruction::CreateMetadata {
            name: "OK".to_string(),
            symbol: "OK".to_string(),
            image: big_image,
            description: "d".to_string(),
            immutable: false,
        };
        let data = ix_data.pack();
        let accounts = vec![
            AccountMeta::new(payer_pk, true),
            AccountMeta::new_readonly(Pubkey::system_program(), false),
            AccountMeta::new_readonly(mint_pk, false),
            AccountMeta::new(metadata_pda, false),
            AccountMeta::new_readonly(payer_pk, true),
        ];
        let metadata_ix = Instruction {
            program_id,
            accounts,
            data,
        };

        let recent = ctx.get_recent_blockhash().await?;
        let message = ArchMessage::new(
            &[create_mint_ix, init_mint_ix, metadata_ix],
            Some(payer_pk),
            recent.parse()?,
        );
        let tx = ctx
            .build_and_sign_transaction(message, vec![payer_kp, mint_kp])
            .await?;
        let txid = ctx.send_transaction(tx).await?;
        let res = ctx.wait_for_transaction(&txid).await?;
        assert!(matches!(res.status, Status::Failed(_)));
        Ok(())
    })
    .await
}

// Description length > DESCRIPTION_MAX_LEN fails
#[tokio::test]
#[serial]
async fn create_metadata_description_length_exceeded_fails() {
    TestRunner::run(|ctx| async move {
        let program_id = deploy_token_metadata_program(&ctx).await?;

        let (payer_kp, payer_pk, _) = ctx.generate_new_keypair();
        ctx.fund_keypair_with_faucet(&payer_kp).await?;
        let (mint_kp, mint_pk, _) = ctx.generate_new_keypair();
        let (metadata_pda, _bump) = find_metadata_pda_with_program(&program_id, &mint_pk);

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
            &payer_pk,
            None,
            9,
        )?;

        let big_desc = "d".repeat(513);
        let ix_data = MetadataInstruction::CreateMetadata {
            name: "OK".to_string(),
            symbol: "OK".to_string(),
            image: "https://example.com/i.png".to_string(),
            description: big_desc,
            immutable: false,
        };
        let data = ix_data.pack();
        let accounts = vec![
            AccountMeta::new(payer_pk, true),
            AccountMeta::new_readonly(Pubkey::system_program(), false),
            AccountMeta::new_readonly(mint_pk, false),
            AccountMeta::new(metadata_pda, false),
            AccountMeta::new_readonly(payer_pk, true),
        ];
        let metadata_ix = Instruction {
            program_id,
            accounts,
            data,
        };

        let recent = ctx.get_recent_blockhash().await?;
        let message = ArchMessage::new(
            &[create_mint_ix, init_mint_ix, metadata_ix],
            Some(payer_pk),
            recent.parse()?,
        );
        let tx = ctx
            .build_and_sign_transaction(message, vec![payer_kp, mint_kp])
            .await?;
        let txid = ctx.send_transaction(tx).await?;
        let res = ctx.wait_for_transaction(&txid).await?;
        assert!(matches!(res.status, Status::Failed(_)));
        Ok(())
    })
    .await
}
