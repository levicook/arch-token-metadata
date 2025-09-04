use std::str::FromStr;

use anyhow::Context;
use arch_program::{hash::Hash, pubkey::Pubkey, sanitized::ArchMessage};
use arch_sdk::AsyncArchRpcClient;
use arch_token_metadata_sdk::{
    CreateAttributesParams, CreateMetadataParams, TokenMetadataClient, TokenMetadataReader,
};
use bitcoin::{
    key::{Keypair, Secp256k1},
    secp256k1::SecretKey,
    Network,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load env from examples/.env (created by setup-payer-and-program)
    let _ = dotenvy::from_path("examples/.env").ok();

    let payer_hex = std::env::var("PAYER_PUBKEY").context("PAYER_PUBKEY")?;
    let payer = parse_hex32(&payer_hex);

    // Require program id from env; no fallback to baked id
    let program_id_hex = std::env::var("ARCH_TOKEN_METADATA_PROGRAM_ID")
        .context("ARCH_TOKEN_METADATA_PROGRAM_ID env var is required")?;

    let program_id_bytes = hex::decode(&program_id_hex)
        .context("ARCH_TOKEN_METADATA_PROGRAM_ID must be hex-encoded")?;

    anyhow::ensure!(
        program_id_bytes.len() == 32,
        "ARCH_TOKEN_METADATA_PROGRAM_ID must be 32 bytes (64 hex chars)"
    );

    let program_id: Pubkey = Pubkey::from_slice(&program_id_bytes);

    let client = TokenMetadataClient::new(program_id);

    // Load payer keypair from env
    let secp = Secp256k1::new();
    let payer_sk = hex::decode(std::env::var("PAYER_PRIVKEY").context("PAYER_PRIVKEY")?)?;
    let payer_kp = Keypair::from_secret_key(&secp, &SecretKey::from_slice(&payer_sk)?);

    // Generate a fresh mint keypair for this run
    let (mint_seed_kp, mint, _) = arch_sdk::generate_new_keypair(Network::Regtest);
    let mint_kp = Keypair::from_secret_key(&secp, &mint_seed_kp.secret_key());

    let metadata_pda = client.metadata_pda(&mint);
    let attributes_pda = client.attributes_pda(&mint);

    let create_md_params = CreateMetadataParams {
        payer,
        mint,
        mint_or_freeze_authority: payer,
        name: "Demo Token".into(),
        symbol: "DT".into(),
        image: "https://example.com/i.png".into(),
        description: "demo".into(),
        immutable: false,
    };

    let create_attrs_params = CreateAttributesParams {
        payer,
        mint,
        update_authority: payer,
        data: vec![
            ("rarity".into(), "common".into()),
            ("series".into(), "alpha".into()),
        ],
    };

    println!("Building instructions: [compute_budget?], create_mint_account, initialize_mint2(decimals=9), create_metadata, create_attributes]");

    let mut tx_instructions = Vec::new();

    // Optional compute budget from env
    if let Some(units) = std::env::var("CU_UNITS")
        .ok()
        .and_then(|v| v.parse::<u32>().ok())
    {
        tx_instructions.push(client.set_compute_unit_limit_ix(units));
    }

    if let Some(bytes) = std::env::var("HEAP_BYTES")
        .ok()
        .and_then(|v| v.parse::<u32>().ok())
    {
        tx_instructions.push(client.request_heap_frame_ix(bytes));
    }

    tx_instructions.push(client.create_mint_account_ix(payer, mint));

    tx_instructions.push(client.initialize_mint2_ix(
        mint,  // token mint
        payer, // mint authority
        None,  // freeze authority (none)
        9,     // decimals
    )?);

    tx_instructions.push(client.create_metadata_ix(create_md_params.clone())?);
    tx_instructions.push(client.create_attributes_ix(create_attrs_params.clone())?);

    // Build message and submit using async arch_sdk client
    let rpc_url = std::env::var("ARCH_RPC").unwrap_or_else(|_| "http://localhost:9002".to_string());
    let rpc_client = AsyncArchRpcClient::new(&rpc_url);
    println!("Metadata program ID: {}", hex::encode(program_id));

    println!("Payer: {}", hex::encode(payer));
    match rpc_client.read_account_info(payer).await {
        Ok(ai) => println!("Payer lamports: {}", ai.lamports),
        Err(e) => println!("Payer account not found or unreadable: {}", e),
    }
    println!("Mint: {}", hex::encode(mint));
    println!("Metadata: {}", hex::encode(metadata_pda));
    println!("Attributes: {}", hex::encode(attributes_pda));

    let recent_blockhash = Hash::from_str(&rpc_client.get_best_block_hash().await?)?;

    // Sign and send
    let tx = arch_sdk::build_and_sign_transaction(
        ArchMessage::new(&tx_instructions, Some(payer), recent_blockhash), // message to sign
        vec![payer_kp, mint_kp], // signers: payer and mint
        Network::Regtest,
    )?;

    println!("Submitting transaction...");
    let txid = rpc_client.send_transaction(tx).await?;
    println!("Submitted txid={}", txid);

    let processed = rpc_client.wait_for_processed_transaction(&txid).await?;
    if processed.status != arch_sdk::Status::Processed {
        eprintln!(
            "tx failed: txid={} status={:?} logs:\n\t{}",
            txid,
            processed.status,
            processed.logs.join("\n\t")
        );
        anyhow::bail!("tx not processed");
    }
    println!("Transaction processed.");

    // Verify metadata account exists and fields

    let reader = TokenMetadataReader::new(program_id, rpc_client);

    let (metadata, attributes) = reader.get_token_details(mint).await?;
    let metadata = metadata.context("metadata not found")?;
    let attributes = attributes.context("attributes not found")?;

    anyhow::ensure!(
        metadata.mint == mint,
        "mint mismatch; expected={} actual={}",
        mint,
        metadata.mint
    );

    anyhow::ensure!(
        metadata.name == create_md_params.name,
        "name mismatch; expected={} actual={}",
        create_md_params.name,
        metadata.name
    );

    anyhow::ensure!(
        metadata.symbol == create_md_params.symbol,
        "symbol mismatch; expected={} actual={}",
        create_md_params.symbol,
        metadata.symbol
    );

    anyhow::ensure!(
        metadata.image == create_md_params.image,
        "image mismatch; expected={} actual={}",
        create_md_params.image,
        metadata.image
    );

    anyhow::ensure!(
        metadata.description == create_md_params.description,
        "description mismatch; expected={} actual={}",
        create_md_params.description,
        metadata.description
    );

    anyhow::ensure!(
        attributes.data == create_attrs_params.data,
        "attributes mismatch; expected={:?} actual={:?}",
        create_attrs_params.data,
        attributes.data
    );

    println!("Metadata: {:?}", metadata);
    println!("Attributes: {:?}", attributes);
    Ok(())
}

fn parse_hex32(s: &str) -> Pubkey {
    let bytes = hex::decode(s).expect("hex");
    assert_eq!(bytes.len(), 32, "expected 32 bytes hex");
    Pubkey::from_slice(&bytes)
}
