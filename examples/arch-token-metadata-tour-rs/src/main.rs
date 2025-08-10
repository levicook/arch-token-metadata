use anyhow::Context;
use arch_program::{program_pack::Pack, pubkey::Pubkey, sanitized::ArchMessage};
use arch_sdk::AsyncArchRpcClient;
use arch_token_metadata::{
    find_metadata_pda_with_program, id as metadata_program_id, state::TokenMetadata,
};
use arch_token_metadata_sdk::{CreateAttributesParams, CreateMetadataParams, TokenMetadataClient};
use bitcoin::{
    key::{Keypair, Secp256k1},
    secp256k1::SecretKey,
};

fn parse_hex32(s: &str) -> Pubkey {
    let bytes = hex::decode(s).expect("hex");
    assert_eq!(bytes.len(), 32, "expected 32 bytes hex");
    Pubkey::from_slice(&bytes)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load env from examples/.env
    let _ = dotenvy::from_path("examples/.env").ok();
    let payer_hex = std::env::var("PAYER_PUBKEY").context("PAYER_PUBKEY")?;
    let payer = parse_hex32(&payer_hex);

    // Prefer PROGRAM_ID from env if present (deployed by wallet-setup), else use baked id()
    let (program_id, program_source): (Pubkey, &str) = if let Ok(p) = std::env::var("PROGRAM_ID") {
        let bytes = hex::decode(p)?;
        (Pubkey::from_slice(&bytes), "env")
    } else {
        (metadata_program_id(), "baked")
    };
    let client = TokenMetadataClient::new(program_id);
    let secp = Secp256k1::new();
    let payer_sk = hex::decode(std::env::var("PAYER_PRIVKEY").context("PAYER_PRIVKEY")?)?;
    let payer_kp = Keypair::from_secret_key(&secp, &SecretKey::from_slice(&payer_sk)?);
    let (mint_seed_kp, mint, _) = arch_sdk::generate_new_keypair(bitcoin::Network::Regtest);
    let mint_kp = Keypair::from_secret_key(&secp, &mint_seed_kp.secret_key());

    let (metadata_pda, _bump) = find_metadata_pda_with_program(&program_id, &mint);
    let mut tx_instructions = Vec::new();
    tx_instructions.push(client.create_mint_account_ix(payer, mint));
    tx_instructions.push(client.initialize_mint2_ix(mint, payer, None, 9)?);
    tx_instructions.push(client.create_metadata_ix(CreateMetadataParams {
        payer,
        mint,
        mint_or_freeze_authority: payer,
        name: "Demo Token".into(),
        symbol: "DT".into(),
        image: "https://example.com/i.png".into(),
        description: "demo".into(),
        immutable: false,
    })?);
    tx_instructions.push(client.create_attributes_ix(CreateAttributesParams {
        payer,
        mint,
        update_authority: payer,
        data: vec![
            ("rarity".into(), "common".into()),
            ("series".into(), "alpha".into()),
        ],
    })?);

    // Build message and submit using async arch_sdk client
    let rpc_url = std::env::var("ARCH_RPC").unwrap_or_else(|_| "http://localhost:9002".to_string());
    let async_client = AsyncArchRpcClient::new(&rpc_url);
    println!(
        "Using PROGRAM_ID ({}): {}",
        program_source,
        hex::encode(program_id)
    );
    println!("Mint (new this run): {}", hex::encode(mint));
    println!("Metadata PDA: {}", hex::encode(metadata_pda));
    println!(
        "Attributes PDA: {}",
        hex::encode(TokenMetadataClient::new(program_id).attributes_pda(&mint))
    );
    use std::str::FromStr;
    let recent_str = async_client.get_best_block_hash().await?;
    let recent = arch_program::hash::Hash::from_str(&recent_str)?;
    println!(
        "Building instructions: [create_mint_account, initialize_mint2(decimals=9), create_metadata, create_attributes]"
    );
    let msg = ArchMessage::new(&tx_instructions, Some(payer), recent);

    // Signers: payer and mint

    // Sign and send
    let runtime_tx = arch_sdk::build_and_sign_transaction(
        msg,
        vec![payer_kp, mint_kp],
        bitcoin::Network::Regtest,
    )?;
    println!("Submitting transaction...");
    let txid = async_client.send_transaction(runtime_tx).await?;
    println!("submitted txid={}", txid);

    let processed = async_client.wait_for_processed_transaction(&txid).await?;
    if processed.status != arch_sdk::Status::Processed {
        eprintln!(
            "tx failed: status={:?} logs:\n{}",
            processed.status,
            processed.logs.join("\n")
        );
        anyhow::bail!("tx not processed");
    }
    println!("Transaction confirmed.");

    // Verify metadata account exists and fields
    let md_account = async_client.read_account_info(metadata_pda).await?;

    let md = TokenMetadata::unpack_from_slice(&md_account.data)?;
    anyhow::ensure!(
        md.mint == mint,
        "mint mismatch; expected={} actual={}",
        mint,
        md.mint
    );
    anyhow::ensure!(
        md.name == "Demo Token",
        "name mismatch; expected=Demo Token actual={}",
        md.name
    );
    anyhow::ensure!(
        md.symbol == "DT",
        "symbol mismatch; expected=DT actual={}",
        md.symbol
    );
    anyhow::ensure!(
        md.image == "https://example.com/i.png",
        "image mismatch; expected=https://example.com/i.png actual={}",
        md.image
    );
    anyhow::ensure!(
        md.description == "demo",
        "description mismatch; expected=demo actual={}",
        md.description
    );

    println!(
        "ok metadata_pda={} mint={} txid={} md_owner={} md_lamports={}",
        hex::encode(metadata_pda),
        hex::encode(mint),
        txid,
        hex::encode(md_account.owner),
        md_account.lamports
    );

    Ok(())
}
