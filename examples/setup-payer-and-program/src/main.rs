use std::{fs, path::Path};

use anyhow::{bail, Context};
use arch_program::bpf_loader::LoaderState;
use arch_sdk::{
    arch_program::pubkey::Pubkey, generate_new_keypair, ArchRpcClient, ProgramDeployer,
};
use bitcoin::{key::Keypair, secp256k1::Secp256k1, Address, Network};

fn main() -> anyhow::Result<()> {
    // Connect to local Arch RPC (provided by docker-compose)
    let rpc_url = std::env::var("ARCH_RPC").unwrap_or_else(|_| "http://localhost:9002".to_string());
    let client = ArchRpcClient::new(&rpc_url);

    // Generate payer keypair
    let network = Network::Regtest;
    let (payer_ut_kp, payer_pk, _): (_, Pubkey, Address) = generate_new_keypair(network);
    let secp = Secp256k1::new();
    let payer_kp = Keypair::from_secret_key(&secp, &payer_ut_kp.secret_key());

    // Fund the payer using faucet (blocking call under the hood)
    client
        .create_and_fund_account_with_faucet(&payer_kp, network)
        .context("faucet funding failed")?;

    // Verify balance & top up if needed for program deployment rent
    let mut acct = client
        .read_account_info(payer_pk)
        .context("read_account_info payer")?;
    if acct.lamports == 0 {
        bail!("Faucet funding produced zero lamports for payer");
    }
    // If deploying a program, estimate required rent and top up
    if let Ok(elf_path) = std::env::var("ARCH_METADATA_ELF") {
        if !elf_path.is_empty() {
            use std::fs;
            let elf = fs::read(&elf_path).context("read ARCH_METADATA_ELF for rent estimate")?;
            let needed =
                arch_program::rent::minimum_rent(LoaderState::program_data_offset() + elf.len());
            let target = needed + 2_000_000_000; // buffer for tx fees
            while acct.lamports < target {
                let _ = client.request_airdrop(payer_pk).context("airdrop top-up")?;
                acct = client
                    .read_account_info(payer_pk)
                    .context("read_account_info payer")?;
            }
        }
    }

    // Optionally deploy the Arch Token Metadata program if ARCH_METADATA_ELF is provided
    let mut program_id_hex: Option<String> = None;
    if let Ok(elf_path) = std::env::var("ARCH_METADATA_ELF") {
        if !elf_path.is_empty() {
            // Generate a fresh program keypair and deploy using payer as authority
            let (program_ut_kp, _program_pk, _): (_, Pubkey, Address) =
                generate_new_keypair(network);
            let program_kp = Keypair::from_secret_key(&secp, &program_ut_kp.secret_key());

            let deployed_id = ProgramDeployer::new(&rpc_url, network)
                .try_deploy_program(
                    "arch-token-metadata".to_string(),
                    program_kp,
                    payer_kp.clone(),
                    &elf_path,
                )
                .context("deploy program")?;
            program_id_hex = Some(hex::encode(deployed_id));
        }
    }

    // Emit .env entries that examples will use
    let env_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join(".env");

    let contents = format!(
        "PAYER_PRIVKEY={}\nPAYER_PUBKEY={}\nARCH_RPC={}\n{}",
        hex::encode(payer_kp.secret_key().secret_bytes()),
        hex::encode(payer_pk),
        rpc_url,
        program_id_hex
            .as_ref()
            .map(|id| format!("PROGRAM_ID={}\n", id))
            .unwrap_or_default()
    );
    fs::write(&env_path, contents).context("write examples/.env")?;
    println!("Wrote {}", env_path.display());

    Ok(())
}
