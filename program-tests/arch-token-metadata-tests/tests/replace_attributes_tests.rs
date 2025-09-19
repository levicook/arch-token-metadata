use arch_program::{
    account::AccountMeta, instruction::Instruction, program_pack::Pack, pubkey::Pubkey,
    rent::minimum_rent, sanitized::ArchMessage, system_instruction,
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
async fn replace_attributes_success() {
    TestRunner::run(|ctx| async move {
        let program_id = deploy_token_metadata_program(&ctx).await?;
        let (payer_kp, payer_pk, _) = ctx.generate_new_keypair();
        ctx.fund_keypair_with_faucet(&payer_kp).await?;
        let (mint_kp, mint_pk, _) = ctx.generate_new_keypair();
        let (metadata_pda, _bmd) = find_metadata_pda_with_program(&program_id, &mint_pk);
        let (attrs_pda, _ba) = find_attributes_pda_with_program(&program_id, &mint_pk);

        let create_mint_ix = system_instruction::create_account(
            &payer_pk,
            &mint_pk,
            minimum_rent(apl_token::state::Mint::LEN),
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

        // initial create attributes
        let initial = vec![("a".into(), "1".into())];
        let create_attrs = MetadataInstruction::CreateAttributes {
            data: initial.clone(),
        }
        .pack();
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

        // replace
        let replacement = vec![("b".into(), "2".into())];
        let replace = MetadataInstruction::ReplaceAttributes {
            data: replacement.clone(),
        }
        .pack();
        let replace_ix = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(attrs_pda, false),
                AccountMeta::new_readonly(payer_pk, true),
                AccountMeta::new_readonly(metadata_pda, false),
            ],
            data: replace,
        };

        let recent = ctx.get_recent_blockhash().await?;
        let msg = ArchMessage::new(
            &[
                create_mint_ix,
                init_mint_ix,
                create_md_ix,
                create_attrs_ix,
                replace_ix,
            ],
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
        assert_eq!(attrs.data, replacement);
        Ok(())
    })
    .await
}

#[tokio::test]
#[serial]
async fn replace_attributes_wrong_signer_fails() {
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
            minimum_rent(apl_token::state::Mint::LEN),
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

        let initial = vec![("a".into(), "1".into())];
        let create_attrs = MetadataInstruction::CreateAttributes { data: initial }.pack();
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

        let replacement = vec![("b".into(), "2".into())];
        let replace = MetadataInstruction::ReplaceAttributes { data: replacement }.pack();
        let replace_ix = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(attrs_pda, false),
                AccountMeta::new_readonly(wrong_pk, true),
                AccountMeta::new_readonly(metadata_pda, false),
            ],
            data: replace,
        };

        let recent = ctx.get_recent_blockhash().await?;
        let msg = ArchMessage::new(
            &[
                create_mint_ix,
                init_mint_ix,
                create_md_ix,
                create_attrs_ix,
                replace_ix,
            ],
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
async fn replace_attributes_empty_key_or_value_fails() {
    TestRunner::run(|ctx| async move {
        let program_id = deploy_token_metadata_program(&ctx).await?;
        let (payer_kp, payer_pk, _) = ctx.generate_new_keypair();
        ctx.fund_keypair_with_faucet(&payer_kp).await?;
        let (mint_kp, mint_pk, _) = ctx.generate_new_keypair();
        let (metadata_pda, _bmd) = find_metadata_pda_with_program(&program_id, &mint_pk);
        let (attrs_pda, _ba) = find_attributes_pda_with_program(&program_id, &mint_pk);

        // create mint + metadata + initial attributes
        let create_mint_ix = system_instruction::create_account(
            &payer_pk,
            &mint_pk,
            minimum_rent(apl_token::state::Mint::LEN),
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

        let initial = vec![("a".into(), "1".into())];
        let create_attrs = MetadataInstruction::CreateAttributes { data: initial }.pack();
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

        // now attempt replace with empty key/value entries
        let replacement = vec![("".into(), "2".into()), ("b".into(), "".into())];
        let replace = MetadataInstruction::ReplaceAttributes { data: replacement }.pack();
        let replace_ix = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(attrs_pda, false),
                AccountMeta::new_readonly(payer_pk, true),
                AccountMeta::new_readonly(metadata_pda, false),
            ],
            data: replace,
        };

        let recent = ctx.get_recent_blockhash().await?;
        let msg = ArchMessage::new(
            &[
                create_mint_ix,
                init_mint_ix,
                create_md_ix,
                create_attrs_ix,
                replace_ix,
            ],
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
