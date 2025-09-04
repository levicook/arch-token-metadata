use anyhow::Context as _;
use arch_sdk::{generate_new_keypair, AsyncArchRpcClient};
use arch_token_metadata_elf::ARCH_TOKEN_METADATA_ELF;
use arch_token_metadata_sdk::{TokenMetadataClient, TokenMetadataReader};
use clap::{Args, Parser, Subcommand, ValueEnum};
use std::{io::Write, str::FromStr};
use tempfile::NamedTempFile;

use arch_program::{
    compute_budget::ComputeBudgetInstruction, hash::Hash, program_pack::Pack, pubkey::Pubkey,
    sanitized::ArchMessage,
};
use bitcoin::{
    key::Keypair,
    secp256k1::{Secp256k1, SecretKey},
    Network,
};

fn parse_hex32(s: &str) -> anyhow::Result<Pubkey> {
    let bytes = hex::decode(s)?;
    anyhow::ensure!(bytes.len() == 32, "expected 32-byte hex");
    Ok(Pubkey::from_slice(&bytes))
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum NetworkArg {
    Regtest,
    Testnet4,
    Mainnet,
}

impl NetworkArg {
    fn to_bitcoin(self) -> Network {
        match self {
            NetworkArg::Regtest => Network::Regtest,
            NetworkArg::Testnet4 => Network::Testnet4,
            NetworkArg::Mainnet => Network::Bitcoin,
        }
    }
}

#[derive(Clone, Debug)]
enum SignerSourceKind {
    Prompt,
    Stdin,
    File,
    Env,
}

#[derive(Clone, Debug, Args)]
struct SignerArg {
    /// Signer source: prompt|stdin|file:/path|env:VAR
    #[arg(long = "payer", alias = "signer", default_value = "prompt")]
    signer: String,
}

fn keypair_from_source(spec: &str) -> anyhow::Result<Keypair> {
    use std::io::Read as _;
    let secp = Secp256k1::new();
    let (kind, rest) = if let Some(rest) = spec.strip_prefix("file:") {
        (SignerSourceKind::File, Some(rest.to_string()))
    } else if let Some(rest) = spec.strip_prefix("env:") {
        (SignerSourceKind::Env, Some(rest.to_string()))
    } else if spec == "stdin" {
        (SignerSourceKind::Stdin, None)
    } else {
        (SignerSourceKind::Prompt, None)
    };

    let secret: Vec<u8> = match kind {
        SignerSourceKind::Prompt => {
            let s = rpassword::prompt_password("enter signer private key hex: ")?;
            hex::decode(s.trim())?
        }
        SignerSourceKind::Stdin => {
            let mut buf = String::new();
            std::io::stdin().read_to_string(&mut buf)?;
            hex::decode(buf.trim())?
        }
        SignerSourceKind::File => {
            let path = rest.expect("file path");
            let s = std::fs::read_to_string(&path).with_context(|| format!("read {}", path))?;
            hex::decode(s.trim())?
        }
        SignerSourceKind::Env => {
            let var = rest.expect("env var");
            let s = std::env::var(&var).with_context(|| format!("env {} not set", var))?;
            hex::decode(s.trim())?
        }
    };
    let kp = Keypair::from_secret_key(&secp, &SecretKey::from_slice(&secret)?);
    Ok(kp)
}

fn pubkey_xonly(kp: &Keypair) -> Pubkey {
    use bitcoin::secp256k1::{Keypair as SecpKeypair, Secp256k1, XOnlyPublicKey};
    let secp = Secp256k1::new();
    let sk = kp.secret_key();
    let secp_kp = SecpKeypair::from_secret_key(&secp, &sk);
    let x = XOnlyPublicKey::from_keypair(&secp_kp).0;
    Pubkey::from_slice(&x.serialize())
}

#[derive(Parser, Debug)]
#[command(
    name = "arch-metadata",
    version,
    about = "Arch Token Metadata CLI",
    long_about = "Command-line interface for creating and managing APL mints and Arch Token Metadata.\nJSON is always printed to stdout; logs/status to stderr."
)]
struct Cli {
    /// RPC endpoint URL
    #[arg(
        default_value = "http://localhost:9002",
        env = "ARCH_RPC",
        global = true,
        long
    )]
    rpc: String,

    /// Metadata program id (hex32). Required for metadata operations (optional for token-only ops)
    #[arg(env = "ARCH_TOKEN_METADATA_PROGRAM_ID", global = true, long)]
    metadata_program_id: Option<String>,

    /// Optional compute unit limit
    #[arg(global = true, long)]
    cu_units: Option<u32>,

    /// Optional heap frame bytes (multiple of 1024)
    #[arg(global = true, long)]
    heap_bytes: Option<u32>,

    #[command(subcommand)]
    command: Commands,

    /// Bitcoin/Arch network (affects keygen/signing/deploy)
    #[arg(global = true, long, value_enum, default_value_t = NetworkArg::Regtest)]
    network: NetworkArg,

    /// Optional Bitcoin Core RPC endpoint (used by deploy)
    #[arg(global = true, long)]
    btc_endpoint: Option<String>,

    /// Optional Bitcoin RPC username (used by deploy)
    #[arg(global = true, long)]
    btc_user: Option<String>,

    /// Optional Bitcoin RPC password (used by deploy)
    #[arg(global = true, long)]
    btc_password: Option<String>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Show mint + metadata + attributes
    #[command(
        alias = "inspect",
        alias = "info",
        about = "Show mint, metadata, and attributes (if present)"
    )]
    Show {
        #[arg(long)]
        mint: String,
    },
    #[command(subcommand, alias = "m", about = "APL mint operations (create, show)")]
    Mint(MintCmd),
    #[command(subcommand, alias = "md", about = "Arch Token Metadata operations")]
    Metadata(MetadataCmd),
    #[command(subcommand, alias = "prog", about = "Program management operations")]
    Program(ProgramCmd),
}

#[derive(Subcommand, Debug)]
enum MintCmd {
    /// Create a new APL token mint (alloc + initialize)
    #[command(
        alias = "new",
        about = "Create a new APL token mint (alloc + initialize)"
    )]
    Create {
        /// Number of decimals for the mint
        #[arg(long, default_value_t = 9)]
        decimals: u8,

        /// Payer signer source
        #[command(flatten)]
        payer: SignerArg,

        /// Optional freeze authority hex32
        #[arg(long)]
        freeze_authority: Option<String>,

        /// Mint authority signer source (defaults to payer)
        #[arg(long)]
        mint_authority: Option<String>,
    },

    /// Show APL mint account
    #[command(alias = "get", about = "Show the APL mint account fields")]
    Show {
        #[arg(long)]
        mint: String,
    },
}

#[derive(Subcommand, Debug)]
enum MetadataCmd {
    /// Create metadata for an existing mint
    #[command(alias = "new", about = "Create token metadata for an existing mint")]
    Create {
        /// Mint address
        #[arg(long)]
        mint: String,

        /// Token name
        #[arg(long)]
        name: String,

        /// Token symbol
        #[arg(long)]
        symbol: String,

        /// Token image
        #[arg(long)]
        image: String,

        /// Token description
        #[arg(long)]
        description: String,

        /// If true, metadata is immutable (no update authority retained)
        #[arg(long, default_value_t = false)]
        immutable: bool,

        /// Payer signer source
        #[command(flatten)]
        payer: SignerArg,

        /// Mint or freeze authority signer source (defaults to payer)
        #[arg(long)]
        mint_authority: Option<String>,
    },

    /// Create attributes for a mint
    #[command(
        alias = "attrs-create",
        alias = "attrs-add",
        about = "Create attributes for a mint"
    )]
    CreateAttributes {
        /// Mint address
        #[arg(long)]
        mint: String,
        /// Repeatable key=value
        #[arg(long = "kv")]
        kvs: Vec<String>,
        /// Payer signer source
        #[command(flatten)]
        payer: SignerArg,
        /// Update authority signer (defaults to payer)
        #[arg(long)]
        update_authority: Option<String>,
    },

    /// Update metadata fields (any subset)
    #[command(alias = "set", about = "Update any subset of metadata fields")]
    Update {
        /// Mint address
        #[arg(long)]
        mint: String,
        /// New name
        #[arg(long)]
        name: Option<String>,
        /// New symbol
        #[arg(long)]
        symbol: Option<String>,
        /// New image URI
        #[arg(long)]
        image: Option<String>,
        /// New description
        #[arg(long)]
        description: Option<String>,
        /// Payer signer source
        #[command(flatten)]
        payer: SignerArg,
        /// Update authority signer (defaults to payer)
        #[arg(long)]
        update_authority: Option<String>,
    },

    /// Replace attributes entirely
    #[command(
        alias = "attrs-replace",
        alias = "attrs-set",
        about = "Replace full attributes vector"
    )]
    ReplaceAttributes {
        /// Mint address
        #[arg(long)]
        mint: String,
        /// Repeatable key=value
        #[arg(long = "kv")]
        kvs: Vec<String>,
        /// Payer signer source
        #[command(flatten)]
        payer: SignerArg,
        /// Update authority signer (defaults to payer)
        #[arg(long)]
        update_authority: Option<String>,
    },

    /// Transfer update authority
    #[command(
        alias = "authority-transfer",
        alias = "auth-transfer",
        about = "Transfer metadata update authority"
    )]
    TransferAuthority {
        /// Mint address
        #[arg(long)]
        mint: String,
        /// New authority (hex32)
        #[arg(long)]
        new_authority: String,
        /// Payer signer source
        #[command(flatten)]
        payer: SignerArg,
        /// Current update authority signer (defaults to payer)
        #[arg(long)]
        current_update_authority: Option<String>,
    },

    /// Make metadata immutable
    #[command(
        alias = "lock",
        about = "Make metadata immutable (clear update authority)"
    )]
    MakeImmutable {
        /// Mint address
        #[arg(long)]
        mint: String,
        /// Payer signer source
        #[command(flatten)]
        payer: SignerArg,
        /// Current update authority signer (defaults to payer)
        #[arg(long)]
        current_update_authority: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
enum ProgramCmd {
    /// Deploy the Arch Token Metadata program ELF embedded in the workspace
    #[command(
        alias = "dep",
        about = "Deploy the Arch Token Metadata program from embedded ELF"
    )]
    Deploy {
        /// Deployer (payer) signer source
        #[command(flatten)]
        deployer: SignerArg,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv().ok();
    let args = Cli::parse();
    let rpc = AsyncArchRpcClient::new(&args.rpc);

    match args.command {
        Commands::Show { mint } => {
            // Unified show
            let mint_pk = parse_hex32(&mint)?;
            let info_opt = rpc.read_account_info(mint_pk).await.ok();
            let mint_json = info_opt.as_ref().and_then(|info| {
                if info.owner != apl_token::id() {
                    None
                } else {
                    apl_token::state::Mint::unpack_from_slice(&info.data)
                        .ok()
                        .map(|m| {
                            serde_json::json!({
                                "owner": hex::encode(info.owner),
                                "is_initialized": m.is_initialized,
                                "decimals": m.decimals,
                            })
                        })
                }
            });
            let program_id_hex = args
                .metadata_program_id
                .as_ref()
                .context("--metadata-program-id or ARCH_TOKEN_METADATA_PROGRAM_ID required")?
                .clone();
            let reader = TokenMetadataReader::new(
                parse_hex32(&program_id_hex)?,
                AsyncArchRpcClient::new(&args.rpc),
            );
            let (md_opt, at_opt) = reader.get_token_details(mint_pk).await?;
            let md_json = md_opt.as_ref().map(|m| {
                serde_json::json!({
                    "is_initialized": m.is_initialized,
                    "mint": hex::encode(m.mint),
                    "name": m.name,
                    "symbol": m.symbol,
                    "image": m.image,
                    "description": m.description,
                    "update_authority": m.update_authority.map(|p| hex::encode(p)),
                })
            });
            let attrs_json = at_opt.as_ref().map(|a| {
                serde_json::json!({
                    "is_initialized": a.is_initialized,
                    "mint": hex::encode(a.mint),
                    "data": a.data,
                })
            });
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "mint": mint_json,
                    "metadata": md_json,
                    "attributes": attrs_json,
                }))?
            );
        }
        Commands::Mint(MintCmd::Create {
            decimals,
            freeze_authority,
            payer,
            mint_authority,
        }) => {
            let payer_kp = keypair_from_source(&payer.signer)?;
            let payer_pk = pubkey_xonly(&payer_kp);

            let auth_kp = if let Some(spec) = mint_authority.as_ref() {
                keypair_from_source(spec)?
            } else {
                payer_kp.clone()
            };

            let auth_pk = pubkey_xonly(&auth_kp);
            let freeze_pk = match freeze_authority.as_ref() {
                Some(s) => Some(parse_hex32(s)?),
                None => None,
            };

            // Generate fresh mint keypair
            let secp = Secp256k1::new();
            let (mint_seed_kp, mint_pk, _) = generate_new_keypair(args.network.to_bitcoin());
            let mint_kp = Keypair::from_secret_key(&secp, &mint_seed_kp.secret_key());

            // Compose APL token ixs
            let create_mint_ix = arch_program::system_instruction::create_account(
                &payer_pk,
                &mint_pk,
                arch_program::account::MIN_ACCOUNT_LAMPORTS,
                apl_token::state::Mint::LEN as u64,
                &apl_token::id(),
            );

            let init_mint_ix = apl_token::instruction::initialize_mint2(
                &apl_token::id(),
                &mint_pk,
                &auth_pk,
                freeze_pk.as_ref(),
                decimals,
            )?;

            let mut ixs = vec![create_mint_ix, init_mint_ix];
            if let Some(units) = args.cu_units {
                ixs.insert(0, ComputeBudgetInstruction::set_compute_unit_limit(units));
            }
            if let Some(bytes) = args.heap_bytes {
                ixs.insert(0, ComputeBudgetInstruction::request_heap_frame(bytes));
            }

            let recent = Hash::from_str(&rpc.get_best_block_hash().await?)?;

            let tx = arch_sdk::build_and_sign_transaction(
                ArchMessage::new(&ixs, Some(payer_pk), recent),
                vec![payer_kp, mint_kp],
                args.network.to_bitcoin(),
            )?;

            let txid = rpc.send_transaction(tx).await?;

            let processed = rpc.wait_for_processed_transaction(&txid).await?;

            eprintln!("create-mint: txid={} status={:?}", txid, processed.status);
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "txid": txid,
                    "status": format!("{:?}", processed.status),
                    "logs": processed.logs,
                    "mint": hex::encode(mint_pk),
                }))?
            );
        }
        Commands::Mint(MintCmd::Show { mint }) => {
            let mint_pk = parse_hex32(&mint)?;
            let info_opt = rpc.read_account_info(mint_pk).await.ok();
            let mint_json = info_opt.as_ref().and_then(|info| {
                if info.owner != apl_token::id() {
                    None
                } else {
                    apl_token::state::Mint::unpack_from_slice(&info.data)
                        .ok()
                        .map(|m| {
                            serde_json::json!({
                                "owner": hex::encode(info.owner),
                                "is_initialized": m.is_initialized,
                                "decimals": m.decimals,
                            })
                        })
                }
            });
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({ "mint": mint_json }))?
            );
        }
        Commands::Metadata(MetadataCmd::Create {
            mint,
            name,
            symbol,
            image,
            description,
            immutable,
            payer,
            mint_authority,
        }) => {
            let program_id =
                parse_hex32(args.metadata_program_id.as_ref().context(
                    "--metadata-program-id or ARCH_TOKEN_METADATA_PROGRAM_ID required",
                )?)?;
            let client = TokenMetadataClient::new(program_id);

            let payer_kp = keypair_from_source(&payer.signer)?;
            let payer_pk = pubkey_xonly(&payer_kp);
            let auth_kp = if let Some(spec) = mint_authority.as_ref() {
                keypair_from_source(spec)?
            } else {
                payer_kp.clone()
            };
            let auth_pk = pubkey_xonly(&auth_kp);
            let mint_pk = parse_hex32(&mint)?;

            let mut ixs = Vec::new();
            if let Some(units) = args.cu_units {
                ixs.push(client.set_compute_unit_limit_ix(units));
            }
            if let Some(bytes) = args.heap_bytes {
                ixs.push(client.request_heap_frame_ix(bytes));
            }
            ixs.push(
                client.create_metadata_ix(arch_token_metadata_sdk::CreateMetadataParams {
                    payer: payer_pk,
                    mint: mint_pk,
                    mint_or_freeze_authority: auth_pk,
                    name,
                    symbol,
                    image,
                    description,
                    immutable,
                })?,
            );

            let recent = Hash::from_str(&rpc.get_best_block_hash().await?)?;
            let tx = arch_sdk::build_and_sign_transaction(
                ArchMessage::new(&ixs, Some(payer_pk), recent),
                vec![payer_kp, auth_kp],
                args.network.to_bitcoin(),
            )?;
            let txid = rpc.send_transaction(tx).await?;
            let processed = rpc.wait_for_processed_transaction(&txid).await?;
            eprintln!(
                "metadata.create: txid={} status={:?}",
                txid, processed.status
            );
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "txid": txid,
                    "status": format!("{:?}", processed.status),
                    "logs": processed.logs,
                }))?
            );
        }

        Commands::Metadata(MetadataCmd::CreateAttributes {
            mint,
            kvs,
            payer,
            update_authority,
        }) => {
            fn parse_kvs(kvs: &[String]) -> anyhow::Result<Vec<(String, String)>> {
                let mut out = Vec::with_capacity(kvs.len());
                for kv in kvs {
                    let Some((k, v)) = kv.split_once('=') else {
                        anyhow::bail!("invalid --kv, expected key=value");
                    };
                    anyhow::ensure!(!k.is_empty() && !v.is_empty(), "empty key or value");
                    out.push((k.to_string(), v.to_string()));
                }
                Ok(out)
            }
            let program_id =
                parse_hex32(args.metadata_program_id.as_ref().context(
                    "--metadata-program-id or ARCH_TOKEN_METADATA_PROGRAM_ID required",
                )?)?;
            let client = TokenMetadataClient::new(program_id);
            let payer_kp = keypair_from_source(&payer.signer)?;
            let payer_pk = pubkey_xonly(&payer_kp);
            let auth_kp = if let Some(spec) = update_authority.as_ref() {
                keypair_from_source(spec)?
            } else {
                payer_kp.clone()
            };
            let auth_pk = pubkey_xonly(&auth_kp);
            let mint_pk = parse_hex32(&mint)?;
            let data = parse_kvs(&kvs)?;

            let ix =
                client.create_attributes_ix(arch_token_metadata_sdk::CreateAttributesParams {
                    payer: payer_pk,
                    mint: mint_pk,
                    update_authority: auth_pk,
                    data,
                })?;
            let recent = Hash::from_str(
                &AsyncArchRpcClient::new(&args.rpc)
                    .get_best_block_hash()
                    .await?,
            )?;
            let tx = arch_sdk::build_and_sign_transaction(
                ArchMessage::new(&[ix], Some(payer_pk), recent),
                vec![payer_kp, auth_kp],
                args.network.to_bitcoin(),
            )?;
            let rpc2 = AsyncArchRpcClient::new(&args.rpc);
            let txid = rpc2.send_transaction(tx).await?;
            let processed = rpc2.wait_for_processed_transaction(&txid).await?;
            eprintln!(
                "metadata.create-attributes: txid={} status={:?}",
                txid, processed.status
            );
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "txid": txid,
                    "status": format!("{:?}", processed.status),
                    "logs": processed.logs,
                }))?
            );
        }

        Commands::Metadata(MetadataCmd::Update {
            mint,
            name,
            symbol,
            image,
            description,
            payer,
            update_authority,
        }) => {
            let program_id =
                parse_hex32(args.metadata_program_id.as_ref().context(
                    "--metadata-program-id or ARCH_TOKEN_METADATA_PROGRAM_ID required",
                )?)?;
            let client = TokenMetadataClient::new(program_id);
            let payer_kp = keypair_from_source(&payer.signer)?;
            let payer_pk = pubkey_xonly(&payer_kp);
            let auth_kp = if let Some(spec) = update_authority.as_ref() {
                keypair_from_source(spec)?
            } else {
                payer_kp.clone()
            };
            let auth_pk = pubkey_xonly(&auth_kp);
            let mint_pk = parse_hex32(&mint)?;

            let ix = client.update_metadata_ix(arch_token_metadata_sdk::UpdateMetadataParams {
                mint: mint_pk,
                update_authority: auth_pk,
                name,
                symbol,
                image,
                description,
            })?;
            let recent = Hash::from_str(
                &AsyncArchRpcClient::new(&args.rpc)
                    .get_best_block_hash()
                    .await?,
            )?;
            let tx = arch_sdk::build_and_sign_transaction(
                ArchMessage::new(&[ix], Some(payer_pk), recent),
                vec![payer_kp, auth_kp],
                args.network.to_bitcoin(),
            )?;
            let rpc2 = AsyncArchRpcClient::new(&args.rpc);
            let txid = rpc2.send_transaction(tx).await?;
            let processed = rpc2.wait_for_processed_transaction(&txid).await?;
            eprintln!(
                "metadata.update: txid={} status={:?}",
                txid, processed.status
            );
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "txid": txid,
                    "status": format!("{:?}", processed.status),
                    "logs": processed.logs,
                }))?
            );
        }

        Commands::Metadata(MetadataCmd::ReplaceAttributes {
            mint,
            kvs,
            payer,
            update_authority,
        }) => {
            fn parse_kvs(kvs: &[String]) -> anyhow::Result<Vec<(String, String)>> {
                let mut out = Vec::with_capacity(kvs.len());
                for kv in kvs {
                    let Some((k, v)) = kv.split_once('=') else {
                        anyhow::bail!("invalid --kv, expected key=value");
                    };
                    anyhow::ensure!(!k.is_empty() && !v.is_empty(), "empty key or value");
                    out.push((k.to_string(), v.to_string()));
                }
                Ok(out)
            }
            let program_id =
                parse_hex32(args.metadata_program_id.as_ref().context(
                    "--metadata-program-id or ARCH_TOKEN_METADATA_PROGRAM_ID required",
                )?)?;
            let client = TokenMetadataClient::new(program_id);
            let payer_kp = keypair_from_source(&payer.signer)?;
            let payer_pk = pubkey_xonly(&payer_kp);
            let auth_kp = if let Some(spec) = update_authority.as_ref() {
                keypair_from_source(spec)?
            } else {
                payer_kp.clone()
            };
            let auth_pk = pubkey_xonly(&auth_kp);
            let mint_pk = parse_hex32(&mint)?;
            let data = parse_kvs(&kvs)?;

            let ix =
                client.replace_attributes_ix(arch_token_metadata_sdk::ReplaceAttributesParams {
                    mint: mint_pk,
                    update_authority: auth_pk,
                    data,
                })?;
            let recent = Hash::from_str(
                &AsyncArchRpcClient::new(&args.rpc)
                    .get_best_block_hash()
                    .await?,
            )?;
            let tx = arch_sdk::build_and_sign_transaction(
                ArchMessage::new(&[ix], Some(payer_pk), recent),
                vec![payer_kp, auth_kp],
                args.network.to_bitcoin(),
            )?;
            let rpc2 = AsyncArchRpcClient::new(&args.rpc);
            let txid = rpc2.send_transaction(tx).await?;
            let processed = rpc2.wait_for_processed_transaction(&txid).await?;
            eprintln!(
                "metadata.replace-attributes: txid={} status={:?}",
                txid, processed.status
            );
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "txid": txid,
                    "status": format!("{:?}", processed.status),
                    "logs": processed.logs,
                }))?
            );
        }

        Commands::Metadata(MetadataCmd::TransferAuthority {
            mint,
            new_authority,
            payer,
            current_update_authority,
        }) => {
            let program_id =
                parse_hex32(args.metadata_program_id.as_ref().context(
                    "--metadata-program-id or ARCH_TOKEN_METADATA_PROGRAM_ID required",
                )?)?;
            let client = TokenMetadataClient::new(program_id);
            let payer_kp = keypair_from_source(&payer.signer)?;
            let payer_pk = pubkey_xonly(&payer_kp);
            let current_kp = if let Some(spec) = current_update_authority.as_ref() {
                keypair_from_source(spec)?
            } else {
                payer_kp.clone()
            };
            let current_pk = pubkey_xonly(&current_kp);
            let mint_pk = parse_hex32(&mint)?;
            let new_pk = parse_hex32(&new_authority)?;

            let ix =
                client.transfer_authority_ix(arch_token_metadata_sdk::TransferAuthorityParams {
                    mint: mint_pk,
                    current_update_authority: current_pk,
                    new_authority: new_pk,
                })?;
            let recent = Hash::from_str(
                &AsyncArchRpcClient::new(&args.rpc)
                    .get_best_block_hash()
                    .await?,
            )?;
            let tx = arch_sdk::build_and_sign_transaction(
                ArchMessage::new(&[ix], Some(payer_pk), recent),
                vec![payer_kp, current_kp],
                args.network.to_bitcoin(),
            )?;
            let rpc2 = AsyncArchRpcClient::new(&args.rpc);
            let txid = rpc2.send_transaction(tx).await?;
            let processed = rpc2.wait_for_processed_transaction(&txid).await?;
            eprintln!(
                "metadata.transfer-authority: txid={} status={:?}",
                txid, processed.status
            );
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "txid": txid,
                    "status": format!("{:?}", processed.status),
                    "logs": processed.logs,
                }))?
            );
        }

        Commands::Metadata(MetadataCmd::MakeImmutable {
            mint,
            payer,
            current_update_authority,
        }) => {
            let program_id =
                parse_hex32(args.metadata_program_id.as_ref().context(
                    "--metadata-program-id or ARCH_TOKEN_METADATA_PROGRAM_ID required",
                )?)?;
            let client = TokenMetadataClient::new(program_id);
            let payer_kp = keypair_from_source(&payer.signer)?;
            let payer_pk = pubkey_xonly(&payer_kp);
            let current_kp = if let Some(spec) = current_update_authority.as_ref() {
                keypair_from_source(spec)?
            } else {
                payer_kp.clone()
            };
            let current_pk = pubkey_xonly(&current_kp);
            let mint_pk = parse_hex32(&mint)?;

            let ix = client.make_immutable_ix(arch_token_metadata_sdk::MakeImmutableParams {
                mint: mint_pk,
                current_update_authority: current_pk,
            })?;
            let recent = Hash::from_str(
                &AsyncArchRpcClient::new(&args.rpc)
                    .get_best_block_hash()
                    .await?,
            )?;
            let tx = arch_sdk::build_and_sign_transaction(
                ArchMessage::new(&[ix], Some(payer_pk), recent),
                vec![payer_kp, current_kp],
                args.network.to_bitcoin(),
            )?;
            let rpc2 = AsyncArchRpcClient::new(&args.rpc);
            let txid = rpc2.send_transaction(tx).await?;
            let processed = rpc2.wait_for_processed_transaction(&txid).await?;
            eprintln!(
                "metadata.make-immutable: txid={} status={:?}",
                txid, processed.status
            );
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "txid": txid,
                    "status": format!("{:?}", processed.status),
                    "logs": processed.logs,
                }))?
            );
        }

        Commands::Program(ProgramCmd::Deploy { deployer }) => {
            let mut temp_file = NamedTempFile::new()?;
            temp_file.write_all(ARCH_TOKEN_METADATA_ELF)?;
            temp_file.flush()?;

            let elf_path = temp_file.path().to_string_lossy().to_string();

            let deployer_kp = keypair_from_source(&deployer.signer)?;
            let (program_seed_kp, _program_id, _addr) =
                generate_new_keypair(args.network.to_bitcoin());
            let program_kp = bitcoin::key::Keypair::from_secret_key(
                &Secp256k1::new(),
                &program_seed_kp.secret_key(),
            );

            let mut config = arch_sdk::Config::localnet();
            // Override config via flags if provided
            if let Some(ep) = args.btc_endpoint.as_ref() {
                config.node_endpoint = ep.clone();
            }
            if let Some(u) = args.btc_user.as_ref() {
                config.node_username = u.clone();
            }
            if let Some(p) = args.btc_password.as_ref() {
                config.node_password = p.clone();
            }
            config.network = args.network.to_bitcoin();
            config.arch_node_url = args.rpc.clone();
            let program_name = "arch-token-metadata".to_string();
            let elf_path_owned = elf_path.clone();
            let deployed_id = tokio::task::spawn_blocking(move || {
                arch_sdk::ProgramDeployer::new(&config).try_deploy_program(
                    program_name,
                    program_kp,
                    deployer_kp,
                    &elf_path_owned,
                )
            })
            .await
            .context("deploy join error")?
            .with_context(|| "deploy program")?;

            eprintln!(
                "program.deploy: deployed program_id={}",
                hex::encode(deployed_id)
            );
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "program_id": hex::encode(deployed_id),
                }))?
            );
        }
    }

    Ok(())
}
