use arch_testing::TestRunner;
use arch_token_metadata_tests::ARCH_TOKEN_METADATA_ELF;

#[tokio::test]
async fn deploy_test() {
    TestRunner::run(|ctx| async move {
        // Generate keypairs
        let (authority_kp, _authority_pubkey, _) = ctx.generate_new_keypair();
        ctx.fund_keypair_with_faucet(&authority_kp).await?;

        // Deploy the program ELF
        let (program_kp, program_id, _) = ctx.generate_new_keypair();
        ctx.deploy_program(program_kp, authority_kp, ARCH_TOKEN_METADATA_ELF)
            .await?;

        println!("Deployed program id: {}", program_id);

        // Sanity: program account exists and is executable
        let info = ctx.read_account_info(program_id).await?;
        println!(
            "Program Account: exec={}, owner={:x}",
            info.is_executable, info.owner
        );
        assert!(info.is_executable, "program account should be executable");

        Ok(())
    })
    .await
}
