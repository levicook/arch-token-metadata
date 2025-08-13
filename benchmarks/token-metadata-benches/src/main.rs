use std::{str::FromStr, time::Duration};

use anyhow::Context;
use arch_program::{hash::Hash, pubkey::Pubkey, sanitized::ArchMessage};
use arch_sdk::AsyncArchRpcClient;
use arch_token_metadata::id as metadata_program_id;
use arch_token_metadata_sdk as tmsdk;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Config
    let rpc_url = std::env::var("ARCH_RPC").unwrap_or_else(|_| "http://localhost:9002".into());
    let warmup_iters: usize = std::env::var("WARMUP_ITERS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(2);
    let iters: usize = std::env::var("ITERS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(10);

    let rpc = AsyncArchRpcClient::new(&rpc_url);

    // Payer keypair from examples env
    dotenvy::from_path("examples/.env").ok();
    let payer_pub_hex = std::env::var("PAYER_PUBKEY").context("PAYER_PUBKEY")?;
    let payer_priv_hex = std::env::var("PAYER_PRIVKEY").context("PAYER_PRIVKEY")?;
    let payer = parse_hex32(&payer_pub_hex);

    let program_id: Pubkey = if let Ok(p) = std::env::var("PROGRAM_ID") {
        Pubkey::from_slice(&hex::decode(p)?)
    } else {
        metadata_program_id()
    };

    let client = tmsdk::TokenMetadataClient::new(program_id);

    // Suite: measure multiple operations and print combined JSON
    let r1 = bench_create_metadata(&rpc, &client, payer, &payer_priv_hex, warmup_iters, iters)
        .await
        .context("bench_create_metadata")?;

    let r2 = bench_create_metadata_and_attributes(
        &rpc,
        &client,
        payer,
        &payer_priv_hex,
        warmup_iters,
        iters,
    )
    .await
    .context("bench_create_metadata_and_attributes")?;

    let r3 = bench_update_metadata(&rpc, &client, payer, &payer_priv_hex, warmup_iters, iters)
        .await
        .context("bench_update_metadata")?;

    let r4 = bench_replace_attributes(&rpc, &client, payer, &payer_priv_hex, warmup_iters, iters)
        .await
        .context("bench_replace_attributes")?;

    let r5 = bench_transfer_authority(&rpc, &client, payer, &payer_priv_hex, warmup_iters, iters)
        .await
        .context("bench_transfer_authority")?;

    let r6 = bench_make_immutable(&rpc, &client, payer, &payer_priv_hex, warmup_iters, iters)
        .await
        .context("bench_make_immutable")?;

    // Full-flow benchmarks (builders including mint + init + metadata ops)
    let full_flows = bench_full_flows(&rpc, &client, payer, &payer_priv_hex, warmup_iters, iters)
        .await
        .context("bench_full_flows")?;

    let report = serde_json::json!({
        "create_metadata": r1,
        "create_metadata_and_attributes": r2,
        "update_metadata": r3,
        "replace_attributes": r4,
        "transfer_authority": r5,
        "make_immutable": r6,
        "full_flows": full_flows,
    });

    println!("{}", serde_json::to_string_pretty(&report)?);

    Ok(())
}

async fn bench_create_metadata(
    rpc: &AsyncArchRpcClient,
    client: &tmsdk::TokenMetadataClient,
    payer: Pubkey,
    payer_priv_hex: &str,
    warmup_iters: usize,
    iters: usize,
) -> anyhow::Result<serde_json::Value> {
    // We will build: [create_mint, initialize_mint2, create_metadata]
    // For deterministic/independent runs, we generate a fresh mint per iteration
    let mut cu_values: Vec<u64> = Vec::with_capacity(iters);

    // Warmups
    for _ in 0..warmup_iters {
        let _ = one_create_metadata_tx(rpc, client, payer, payer_priv_hex).await;
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    // Measured
    for _ in 0..iters {
        let cu = one_create_metadata_tx(rpc, client, payer, payer_priv_hex).await?;
        cu_values.push(cu);
        tokio::time::sleep(Duration::from_millis(25)).await;
    }

    cu_values.sort_unstable();
    let median = cu_values[cu_values.len() / 2];
    let p90_idx = ((cu_values.len() as f64 * 0.9).floor() as usize).min(cu_values.len() - 1);
    let p90 = cu_values[p90_idx];

    Ok(serde_json::json!({
        "iters": iters,
        "warmup_iters": warmup_iters,
        "median_cu": median,
        "p90_cu": p90,
        "all": cu_values,
    }))
}

async fn one_create_metadata_tx(
    rpc: &AsyncArchRpcClient,
    client: &tmsdk::TokenMetadataClient,
    payer: Pubkey,
    payer_priv_hex: &str,
) -> anyhow::Result<u64> {
    use bitcoin::{key::Keypair, Network};

    // Fresh mint keypair
    let (sk, mint, _) = arch_sdk::generate_new_keypair(Network::Regtest);
    let secp = bitcoin::secp256k1::Secp256k1::new();
    let mint_kp = Keypair::from_secret_key(&secp, &sk.secret_key());
    let payer_kp = keypair_from_priv_hex(payer_priv_hex)?;

    // Build instructions
    let create_md_params = tmsdk::CreateMetadataParams {
        payer,
        mint,
        mint_or_freeze_authority: payer,
        name: "Bench Token".into(),
        symbol: "BT".into(),
        image: "https://example.com/i.png".into(),
        description: "bench".into(),
        immutable: false,
    };

    let create_mint_ix = client.create_mint_account_ix(payer, mint);
    let init_mint_ix = client.initialize_mint2_ix(mint, payer, None, 9)?;
    let create_md_ix = client.create_metadata_ix(create_md_params)?;

    let recent_blockhash = Hash::from_str(&rpc.get_best_block_hash().await?)?;
    let msg = ArchMessage::new(
        &[create_mint_ix, init_mint_ix, create_md_ix],
        Some(payer),
        recent_blockhash,
    );
    let tx = arch_sdk::build_and_sign_transaction(msg, vec![payer_kp, mint_kp], Network::Regtest)?;

    let txid = rpc.send_transaction(tx).await?;
    let processed = rpc.wait_for_processed_transaction(&txid).await?;
    let cu_opt = processed.compute_units_consumed();
    let cu = cu_opt
        .and_then(|s| s.parse::<u64>().ok())
        .context("compute units not found in logs")?;
    Ok(cu)
}

fn parse_hex32(s: &str) -> Pubkey {
    let bytes = hex::decode(s).expect("hex");
    assert_eq!(bytes.len(), 32, "expected 32 bytes hex");
    Pubkey::from_slice(&bytes)
}

fn keypair_from_priv_hex(priv_hex: &str) -> anyhow::Result<bitcoin::key::Keypair> {
    use bitcoin::{key::Keypair, secp256k1::SecretKey};
    let secp = bitcoin::secp256k1::Secp256k1::new();
    let sk_bytes = hex::decode(priv_hex)?;
    let sk = SecretKey::from_slice(&sk_bytes)?;
    let kp = Keypair::from_secret_key(&secp, &sk);
    Ok(kp)
}

// ---- Additional Benches ----

// Full-flow suite using Rust SDK transaction builders (includes mint+init+metadata ops)
async fn bench_full_flows(
    rpc: &AsyncArchRpcClient,
    client: &tmsdk::TokenMetadataClient,
    payer: Pubkey,
    payer_priv_hex: &str,
    warmup_iters: usize,
    iters: usize,
) -> anyhow::Result<serde_json::Value> {
    let a = bench_full_create_token_with_metadata(
        rpc,
        client,
        payer,
        payer_priv_hex,
        warmup_iters,
        iters,
    )
    .await?;
    let b = bench_full_create_token_with_metadata_and_attributes(
        rpc,
        client,
        payer,
        payer_priv_hex,
        warmup_iters,
        iters,
    )
    .await?;
    let c = bench_full_create_token_with_freeze_auth_metadata(
        rpc,
        client,
        payer,
        payer_priv_hex,
        warmup_iters,
        iters,
    )
    .await?;
    Ok(serde_json::json!({
        "full_create_token_with_metadata": a,
        "full_create_token_with_metadata_and_attributes": b,
        "full_create_token_with_freeze_auth_metadata": c,
    }))
}

async fn bench_full_create_token_with_metadata(
    rpc: &AsyncArchRpcClient,
    client: &tmsdk::TokenMetadataClient,
    payer: Pubkey,
    payer_priv_hex: &str,
    warmup_iters: usize,
    iters: usize,
) -> anyhow::Result<serde_json::Value> {
    let mut cu_values: Vec<u64> = Vec::with_capacity(iters);
    for _ in 0..warmup_iters {
        let _ = one_full_create_token_with_metadata(rpc, client, payer, payer_priv_hex).await;
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    for _ in 0..iters {
        let cu = one_full_create_token_with_metadata(rpc, client, payer, payer_priv_hex).await?;
        cu_values.push(cu);
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    cu_values.sort_unstable();
    let median = cu_values[cu_values.len() / 2];
    let p90_idx = ((cu_values.len() as f64 * 0.9).floor() as usize).min(cu_values.len() - 1);
    let p90 = cu_values[p90_idx];

    Ok(serde_json::json!({
        "iters": iters,
        "warmup_iters": warmup_iters,
        "median_cu": median,
        "p90_cu": p90,
        "all": cu_values,
    }))
}

async fn one_full_create_token_with_metadata(
    rpc: &AsyncArchRpcClient,
    client: &tmsdk::TokenMetadataClient,
    payer: Pubkey,
    payer_priv_hex: &str,
) -> anyhow::Result<u64> {
    use bitcoin::{key::Keypair, Network};
    let (sk, mint, _) = arch_sdk::generate_new_keypair(Network::Regtest);
    let secp = bitcoin::secp256k1::Secp256k1::new();
    let mint_kp = Keypair::from_secret_key(&secp, &sk.secret_key());
    let payer_kp = keypair_from_priv_hex(payer_priv_hex)?;

    let tx_ixs = client.create_token_with_metadata_tx(tmsdk::TxCreateTokenWithMetadataParams {
        payer,
        mint,
        mint_authority: payer,
        freeze_authority: None,
        decimals: 9,
        name: "Bench Token".into(),
        symbol: "BT".into(),
        image: "https://example.com/i.png".into(),
        description: "bench".into(),
        immutable: false,
    })?;
    let bh = Hash::from_str(&rpc.get_best_block_hash().await?)?;
    let msg = ArchMessage::new(&tx_ixs, Some(payer), bh);
    let tx = arch_sdk::build_and_sign_transaction(msg, vec![payer_kp, mint_kp], Network::Regtest)?;
    let txid = rpc.send_transaction(tx).await?;
    let processed = rpc.wait_for_processed_transaction(&txid).await?;
    let cu = processed
        .compute_units_consumed()
        .and_then(|s| s.parse::<u64>().ok())
        .context("compute units not found in logs")?;
    Ok(cu)
}

async fn bench_full_create_token_with_metadata_and_attributes(
    rpc: &AsyncArchRpcClient,
    client: &tmsdk::TokenMetadataClient,
    payer: Pubkey,
    payer_priv_hex: &str,
    warmup_iters: usize,
    iters: usize,
) -> anyhow::Result<serde_json::Value> {
    let mut cu_values: Vec<u64> = Vec::with_capacity(iters);
    for _ in 0..warmup_iters {
        let _ =
            one_full_create_token_with_metadata_and_attributes(rpc, client, payer, payer_priv_hex)
                .await;
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    for _ in 0..iters {
        let cu =
            one_full_create_token_with_metadata_and_attributes(rpc, client, payer, payer_priv_hex)
                .await?;
        cu_values.push(cu);
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    cu_values.sort_unstable();
    let median = cu_values[cu_values.len() / 2];
    let p90_idx = ((cu_values.len() as f64 * 0.9).floor() as usize).min(cu_values.len() - 1);
    let p90 = cu_values[p90_idx];
    Ok(
        serde_json::json!({"iters": iters, "warmup_iters": warmup_iters, "median_cu": median, "p90_cu": p90, "all": cu_values}),
    )
}

async fn one_full_create_token_with_metadata_and_attributes(
    rpc: &AsyncArchRpcClient,
    client: &tmsdk::TokenMetadataClient,
    payer: Pubkey,
    payer_priv_hex: &str,
) -> anyhow::Result<u64> {
    use bitcoin::{key::Keypair, Network};
    let (sk, mint, _) = arch_sdk::generate_new_keypair(Network::Regtest);
    let secp = bitcoin::secp256k1::Secp256k1::new();
    let mint_kp = Keypair::from_secret_key(&secp, &sk.secret_key());
    let payer_kp = keypair_from_priv_hex(payer_priv_hex)?;

    let tx_ixs = client.create_token_with_metadata_and_attributes_tx(
        tmsdk::TxCreateTokenWithMetadataAndAttributesParams {
            payer,
            mint,
            mint_authority: payer,
            freeze_authority: None,
            decimals: 9,
            name: "Bench Token".into(),
            symbol: "BT".into(),
            image: "https://example.com/i.png".into(),
            description: "bench".into(),
            immutable: false,
            attributes: vec![("k1".into(), "v1".into()), ("k2".into(), "v2".into())],
        },
    )?;
    let bh = Hash::from_str(&rpc.get_best_block_hash().await?)?;
    let msg = ArchMessage::new(&tx_ixs, Some(payer), bh);
    let tx = arch_sdk::build_and_sign_transaction(msg, vec![payer_kp, mint_kp], Network::Regtest)?;
    let txid = rpc.send_transaction(tx).await?;
    let processed = rpc.wait_for_processed_transaction(&txid).await?;
    let cu = processed
        .compute_units_consumed()
        .and_then(|s| s.parse::<u64>().ok())
        .context("compute units not found in logs")?;
    Ok(cu)
}

async fn bench_full_create_token_with_freeze_auth_metadata(
    rpc: &AsyncArchRpcClient,
    client: &tmsdk::TokenMetadataClient,
    payer: Pubkey,
    payer_priv_hex: &str,
    warmup_iters: usize,
    iters: usize,
) -> anyhow::Result<serde_json::Value> {
    let mut cu_values: Vec<u64> = Vec::with_capacity(iters);
    for _ in 0..warmup_iters {
        let _ = one_full_create_token_with_freeze_auth_metadata(rpc, client, payer, payer_priv_hex)
            .await;
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    for _ in 0..iters {
        let cu =
            one_full_create_token_with_freeze_auth_metadata(rpc, client, payer, payer_priv_hex)
                .await?;
        cu_values.push(cu);
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    cu_values.sort_unstable();
    let median = cu_values[cu_values.len() / 2];
    let p90_idx = ((cu_values.len() as f64 * 0.9).floor() as usize).min(cu_values.len() - 1);
    let p90 = cu_values[p90_idx];
    Ok(
        serde_json::json!({"iters": iters, "warmup_iters": warmup_iters, "median_cu": median, "p90_cu": p90, "all": cu_values}),
    )
}

async fn one_full_create_token_with_freeze_auth_metadata(
    rpc: &AsyncArchRpcClient,
    client: &tmsdk::TokenMetadataClient,
    payer: Pubkey,
    payer_priv_hex: &str,
) -> anyhow::Result<u64> {
    use bitcoin::{key::Keypair, Network};
    let (sk, mint, _) = arch_sdk::generate_new_keypair(Network::Regtest);
    let secp = bitcoin::secp256k1::Secp256k1::new();
    let mint_kp = Keypair::from_secret_key(&secp, &sk.secret_key());
    let payer_kp = keypair_from_priv_hex(payer_priv_hex)?;

    let tx_ixs = client.create_token_with_freeze_auth_metadata_tx(
        tmsdk::TxCreateTokenWithFreezeAuthMetadataParams {
            payer,
            mint,
            initial_mint_authority: payer,
            freeze_authority: payer,
            decimals: 9,
            name: "Bench Token".into(),
            symbol: "BT".into(),
            image: "https://example.com/i.png".into(),
            description: "bench".into(),
            immutable: false,
        },
    )?;
    let bh = Hash::from_str(&rpc.get_best_block_hash().await?)?;
    let msg = ArchMessage::new(&tx_ixs, Some(payer), bh);
    let tx = arch_sdk::build_and_sign_transaction(msg, vec![payer_kp, mint_kp], Network::Regtest)?;
    let txid = rpc.send_transaction(tx).await?;
    let processed = rpc.wait_for_processed_transaction(&txid).await?;
    let cu = processed
        .compute_units_consumed()
        .and_then(|s| s.parse::<u64>().ok())
        .context("compute units not found in logs")?;
    Ok(cu)
}

async fn bench_create_metadata_and_attributes(
    rpc: &AsyncArchRpcClient,
    client: &tmsdk::TokenMetadataClient,
    payer: Pubkey,
    payer_priv_hex: &str,
    warmup_iters: usize,
    iters: usize,
) -> anyhow::Result<serde_json::Value> {
    let mut cu_values: Vec<u64> = Vec::with_capacity(iters);
    for _ in 0..warmup_iters {
        let _ = one_create_md_and_attrs_tx(rpc, client, payer, payer_priv_hex).await;
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    for _ in 0..iters {
        let cu = one_create_md_and_attrs_tx(rpc, client, payer, payer_priv_hex).await?;
        cu_values.push(cu);
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    cu_values.sort_unstable();
    let median = cu_values[cu_values.len() / 2];
    let p90_idx = ((cu_values.len() as f64 * 0.9).floor() as usize).min(cu_values.len() - 1);
    let p90 = cu_values[p90_idx];
    Ok(
        serde_json::json!({"iters": iters, "warmup_iters": warmup_iters, "median_cu": median, "p90_cu": p90, "all": cu_values}),
    )
}

async fn one_create_md_and_attrs_tx(
    rpc: &AsyncArchRpcClient,
    client: &tmsdk::TokenMetadataClient,
    payer: Pubkey,
    payer_priv_hex: &str,
) -> anyhow::Result<u64> {
    use bitcoin::{key::Keypair, Network};
    let (sk, mint, _) = arch_sdk::generate_new_keypair(Network::Regtest);
    let secp = bitcoin::secp256k1::Secp256k1::new();
    let mint_kp = Keypair::from_secret_key(&secp, &sk.secret_key());
    let payer_kp = keypair_from_priv_hex(payer_priv_hex)?;

    let create_md = client.create_metadata_ix(tmsdk::CreateMetadataParams {
        payer,
        mint,
        mint_or_freeze_authority: payer,
        name: "Bench Token".into(),
        symbol: "BT".into(),
        image: "https://example.com/i.png".into(),
        description: "bench".into(),
        immutable: false,
    })?;
    let create_attrs = client.create_attributes_ix(tmsdk::CreateAttributesParams {
        payer,
        mint,
        update_authority: payer,
        data: vec![("k1".into(), "v1".into()), ("k2".into(), "v2".into())],
    })?;
    let create_mint = client.create_mint_account_ix(payer, mint);
    let init_mint = client.initialize_mint2_ix(mint, payer, None, 9)?;
    let bh = Hash::from_str(&rpc.get_best_block_hash().await?)?;
    let msg = ArchMessage::new(
        &[create_mint, init_mint, create_md, create_attrs],
        Some(payer),
        bh,
    );
    let tx = arch_sdk::build_and_sign_transaction(msg, vec![payer_kp, mint_kp], Network::Regtest)?;
    let txid = rpc.send_transaction(tx).await?;
    let processed = rpc.wait_for_processed_transaction(&txid).await?;
    let cu = processed
        .compute_units_consumed()
        .and_then(|s| s.parse::<u64>().ok())
        .context("compute units not found in logs")?;
    Ok(cu)
}

async fn bench_update_metadata(
    rpc: &AsyncArchRpcClient,
    client: &tmsdk::TokenMetadataClient,
    payer: Pubkey,
    payer_priv_hex: &str,
    warmup_iters: usize,
    iters: usize,
) -> anyhow::Result<serde_json::Value> {
    let mut cu_values: Vec<u64> = Vec::with_capacity(iters);
    for _ in 0..warmup_iters {
        let _ = one_update_metadata_tx(rpc, client, payer, payer_priv_hex).await;
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    for _ in 0..iters {
        let cu = one_update_metadata_tx(rpc, client, payer, payer_priv_hex).await?;
        cu_values.push(cu);
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    cu_values.sort_unstable();
    let median = cu_values[cu_values.len() / 2];
    let p90_idx = ((cu_values.len() as f64 * 0.9).floor() as usize).min(cu_values.len() - 1);
    let p90 = cu_values[p90_idx];
    Ok(
        serde_json::json!({"iters": iters, "warmup_iters": warmup_iters, "median_cu": median, "p90_cu": p90, "all": cu_values}),
    )
}

async fn setup_metadata_once(
    rpc: &AsyncArchRpcClient,
    client: &tmsdk::TokenMetadataClient,
    payer: Pubkey,
    payer_priv_hex: &str,
) -> anyhow::Result<(bitcoin::key::Keypair, Pubkey)> {
    use bitcoin::{key::Keypair, Network};
    let (sk, mint, _) = arch_sdk::generate_new_keypair(Network::Regtest);
    let secp = bitcoin::secp256k1::Secp256k1::new();
    let mint_kp = Keypair::from_secret_key(&secp, &sk.secret_key());
    let payer_kp = keypair_from_priv_hex(payer_priv_hex)?;
    // submit create mint + init + create md (attributes not required)
    let create_mint = client.create_mint_account_ix(payer, mint);
    let init_mint = client.initialize_mint2_ix(mint, payer, None, 9)?;
    let create_md = client.create_metadata_ix(tmsdk::CreateMetadataParams {
        payer,
        mint,
        mint_or_freeze_authority: payer,
        name: "Bench Token".into(),
        symbol: "BT".into(),
        image: "https://example.com/i.png".into(),
        description: "bench".into(),
        immutable: false,
    })?;
    let bh = Hash::from_str(&rpc.get_best_block_hash().await?)?;
    let msg = ArchMessage::new(&[create_mint, init_mint, create_md], Some(payer), bh);
    let tx = arch_sdk::build_and_sign_transaction(
        msg,
        vec![payer_kp, mint_kp.clone()],
        Network::Regtest,
    )?;
    let txid = rpc.send_transaction(tx).await?;
    let _ = rpc.wait_for_processed_transaction(&txid).await?;
    Ok((mint_kp, mint))
}

async fn one_update_metadata_tx(
    rpc: &AsyncArchRpcClient,
    client: &tmsdk::TokenMetadataClient,
    payer: Pubkey,
    payer_priv_hex: &str,
) -> anyhow::Result<u64> {
    use bitcoin::Network;
    let (mint_kp, mint) = setup_metadata_once(rpc, client, payer, payer_priv_hex).await?;
    let payer_kp = keypair_from_priv_hex(payer_priv_hex)?;
    let update = client.update_metadata_ix(tmsdk::UpdateMetadataParams {
        mint,
        update_authority: payer,
        name: Some("New".into()),
        symbol: None,
        image: None,
        description: None,
    })?;
    let bh = Hash::from_str(&rpc.get_best_block_hash().await?)?;
    let msg = ArchMessage::new(&[update], Some(payer), bh);
    let tx = arch_sdk::build_and_sign_transaction(msg, vec![payer_kp, mint_kp], Network::Regtest)?;
    let txid = rpc.send_transaction(tx).await?;
    let processed = rpc.wait_for_processed_transaction(&txid).await?;
    let cu = processed
        .compute_units_consumed()
        .and_then(|s| s.parse::<u64>().ok())
        .context("compute units not found in logs")?;
    Ok(cu)
}

async fn bench_replace_attributes(
    rpc: &AsyncArchRpcClient,
    client: &tmsdk::TokenMetadataClient,
    payer: Pubkey,
    payer_priv_hex: &str,
    warmup_iters: usize,
    iters: usize,
) -> anyhow::Result<serde_json::Value> {
    let mut cu_values: Vec<u64> = Vec::with_capacity(iters);
    for _ in 0..warmup_iters {
        let _ = one_replace_attributes_tx(rpc, client, payer, payer_priv_hex).await;
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    for _ in 0..iters {
        let cu = one_replace_attributes_tx(rpc, client, payer, payer_priv_hex).await?;
        cu_values.push(cu);
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    cu_values.sort_unstable();
    let median = cu_values[cu_values.len() / 2];
    let p90_idx = ((cu_values.len() as f64 * 0.9).floor() as usize).min(cu_values.len() - 1);
    let p90 = cu_values[p90_idx];
    Ok(
        serde_json::json!({"iters": iters, "warmup_iters": warmup_iters, "median_cu": median, "p90_cu": p90, "all": cu_values}),
    )
}

async fn one_replace_attributes_tx(
    rpc: &AsyncArchRpcClient,
    client: &tmsdk::TokenMetadataClient,
    payer: Pubkey,
    payer_priv_hex: &str,
) -> anyhow::Result<u64> {
    use bitcoin::Network;
    // Setup metadata + attributes first
    let (mint_kp, mint) = setup_metadata_once(rpc, client, payer, payer_priv_hex).await?;
    // create attributes
    let payer_kp = keypair_from_priv_hex(payer_priv_hex)?;
    let create_attrs = client.create_attributes_ix(tmsdk::CreateAttributesParams {
        payer,
        mint,
        update_authority: payer,
        data: vec![("a".into(), "1".into())],
    })?;
    let bh = Hash::from_str(&rpc.get_best_block_hash().await?)?;
    let msg1 = ArchMessage::new(&[create_attrs], Some(payer), bh);
    let tx1 = arch_sdk::build_and_sign_transaction(
        msg1,
        vec![payer_kp.clone(), mint_kp.clone()],
        Network::Regtest,
    )?;
    let txid1 = rpc.send_transaction(tx1).await?;
    let _ = rpc.wait_for_processed_transaction(&txid1).await?;

    // replace attributes
    let replace = client.replace_attributes_ix(tmsdk::ReplaceAttributesParams {
        mint,
        update_authority: payer,
        data: vec![("a".into(), "2".into()), ("b".into(), "3".into())],
    })?;
    let bh2 = Hash::from_str(&rpc.get_best_block_hash().await?)?;
    let msg2 = ArchMessage::new(&[replace], Some(payer), bh2);
    let tx2 =
        arch_sdk::build_and_sign_transaction(msg2, vec![payer_kp, mint_kp], Network::Regtest)?;
    let txid2 = rpc.send_transaction(tx2).await?;
    let processed = rpc.wait_for_processed_transaction(&txid2).await?;
    let cu = processed
        .compute_units_consumed()
        .and_then(|s| s.parse::<u64>().ok())
        .context("compute units not found in logs")?;
    Ok(cu)
}

async fn bench_transfer_authority(
    rpc: &AsyncArchRpcClient,
    client: &tmsdk::TokenMetadataClient,
    payer: Pubkey,
    payer_priv_hex: &str,
    warmup_iters: usize,
    iters: usize,
) -> anyhow::Result<serde_json::Value> {
    let mut cu_values: Vec<u64> = Vec::with_capacity(iters);
    for _ in 0..warmup_iters {
        let _ = one_transfer_authority_tx(rpc, client, payer, payer_priv_hex).await;
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    for _ in 0..iters {
        let cu = one_transfer_authority_tx(rpc, client, payer, payer_priv_hex).await?;
        cu_values.push(cu);
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    cu_values.sort_unstable();
    let median = cu_values[cu_values.len() / 2];
    let p90_idx = ((cu_values.len() as f64 * 0.9).floor() as usize).min(cu_values.len() - 1);
    let p90 = cu_values[p90_idx];
    Ok(
        serde_json::json!({"iters": iters, "warmup_iters": warmup_iters, "median_cu": median, "p90_cu": p90, "all": cu_values}),
    )
}

async fn one_transfer_authority_tx(
    rpc: &AsyncArchRpcClient,
    client: &tmsdk::TokenMetadataClient,
    payer: Pubkey,
    payer_priv_hex: &str,
) -> anyhow::Result<u64> {
    use bitcoin::Network;
    let (mint_kp, mint) = setup_metadata_once(rpc, client, payer, payer_priv_hex).await?;
    let payer_kp = keypair_from_priv_hex(payer_priv_hex)?;
    // random new authority
    let (new_sk, new_auth, _) = arch_sdk::generate_new_keypair(Network::Regtest);
    let _new_kp = bitcoin::key::Keypair::from_secret_key(
        &bitcoin::secp256k1::Secp256k1::new(),
        &new_sk.secret_key(),
    );
    let transfer = client.transfer_authority_ix(tmsdk::TransferAuthorityParams {
        mint,
        current_update_authority: payer,
        new_authority: new_auth,
    })?;
    let bh = Hash::from_str(&rpc.get_best_block_hash().await?)?;
    let msg = ArchMessage::new(&[transfer], Some(payer), bh);
    let tx = arch_sdk::build_and_sign_transaction(msg, vec![payer_kp, mint_kp], Network::Regtest)?;
    let txid = rpc.send_transaction(tx).await?;
    let processed = rpc.wait_for_processed_transaction(&txid).await?;
    let cu = processed
        .compute_units_consumed()
        .and_then(|s| s.parse::<u64>().ok())
        .context("compute units not found in logs")?;
    Ok(cu)
}

async fn bench_make_immutable(
    rpc: &AsyncArchRpcClient,
    client: &tmsdk::TokenMetadataClient,
    payer: Pubkey,
    payer_priv_hex: &str,
    warmup_iters: usize,
    iters: usize,
) -> anyhow::Result<serde_json::Value> {
    let mut cu_values: Vec<u64> = Vec::with_capacity(iters);
    for _ in 0..warmup_iters {
        let _ = one_make_immutable_tx(rpc, client, payer, payer_priv_hex).await;
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    for _ in 0..iters {
        let cu = one_make_immutable_tx(rpc, client, payer, payer_priv_hex).await?;
        cu_values.push(cu);
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    cu_values.sort_unstable();
    let median = cu_values[cu_values.len() / 2];
    let p90_idx = ((cu_values.len() as f64 * 0.9).floor() as usize).min(cu_values.len() - 1);
    let p90 = cu_values[p90_idx];
    Ok(
        serde_json::json!({"iters": iters, "warmup_iters": warmup_iters, "median_cu": median, "p90_cu": p90, "all": cu_values}),
    )
}

async fn one_make_immutable_tx(
    rpc: &AsyncArchRpcClient,
    client: &tmsdk::TokenMetadataClient,
    payer: Pubkey,
    payer_priv_hex: &str,
) -> anyhow::Result<u64> {
    use bitcoin::Network;
    let (mint_kp, mint) = setup_metadata_once(rpc, client, payer, payer_priv_hex).await?;
    let payer_kp = keypair_from_priv_hex(payer_priv_hex)?;
    let make_imm = client.make_immutable_ix(tmsdk::MakeImmutableParams {
        mint,
        current_update_authority: payer,
    })?;
    let bh = Hash::from_str(&rpc.get_best_block_hash().await?)?;
    let msg = ArchMessage::new(&[make_imm], Some(payer), bh);
    let tx = arch_sdk::build_and_sign_transaction(msg, vec![payer_kp, mint_kp], Network::Regtest)?;
    let txid = rpc.send_transaction(tx).await?;
    let processed = rpc.wait_for_processed_transaction(&txid).await?;
    let cu = processed
        .compute_units_consumed()
        .and_then(|s| s.parse::<u64>().ok())
        .context("compute units not found in logs")?;
    Ok(cu)
}
