import {
  TokenMetadataClient,
  metadataProgramId,
  tokenProgramId,
  type Instruction as MetaInstruction,
} from "arch-token-metadata-sdk";
import {
  RpcConnection,
  ArchConnection,
  SanitizedMessageUtil,
  SignatureUtil,
  type Instruction as ArchInstruction,
  type RuntimeTransaction,
  PubkeyUtil,
} from "@saturnbtcio/arch-sdk";
import { Signer as Bip322Signer } from "bip322-js";
import * as bitcoin from "bitcoinjs-lib";
import wif from "wif";
import { secp256k1 } from "@noble/curves/secp256k1";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

function hexToBytes(hex: string): Uint8Array {
  const clean = hex.startsWith("0x") ? hex.slice(2) : hex;
  const out = new Uint8Array(clean.length / 2);
  for (let i = 0; i < clean.length; i += 2)
    out[i / 2] = parseInt(clean.slice(i, i + 2), 16);
  return out;
}

function loadExamplesEnv(): void {
  const localDirname = path.dirname(fileURLToPath(import.meta.url));
  const candidatePaths = [
    path.resolve(localDirname, "..", "..", ".env"),
    path.resolve(process.cwd(), "..", ".env"),
  ];
  for (const p of candidatePaths) {
    if (fs.existsSync(p)) {
      const text = fs.readFileSync(p, "utf8");
      for (const rawLine of text.split(/\r?\n/)) {
        const line = rawLine.trim();
        if (!line || line.startsWith("#")) continue;
        const idx = line.indexOf("=");
        if (idx === -1) continue;
        const key = line.slice(0, idx).trim();
        const value = line.slice(idx + 1).trim();
        if (!(key in process.env)) process.env[key] = value;
      }
      break;
    }
  }
}

async function main() {
  loadExamplesEnv();
  const rpcUrl = process.env.ARCH_RPC;
  const payerPubHex = process.env.PAYER_PUBKEY;
  const payerPrivHex = process.env.PAYER_PRIVKEY;
  if (!rpcUrl || !payerPrivHex) {
    throw new Error(
      "Missing env. Run examples/launch.sh first to generate examples/.env"
    );
  }

  const defaultProgramId = metadataProgramId();
  const programIdHex = process.env.PROGRAM_ID;
  const programId = (
    programIdHex ? hexToBytes(programIdHex) : defaultProgramId
  ) as Uint8Array;
  const client = new TokenMetadataClient(programId);

  // Provider
  const provider = new RpcConnection(rpcUrl);
  const arch = ArchConnection(provider);

  // Generate a fresh mint keypair for this run (Taproot single-key spend)
  const mintPriv = secp256k1.utils.randomPrivateKey();
  const mintFull = secp256k1.getPublicKey(mintPriv, true); // 33B compressed
  const mint = mintFull.slice(1); // 32B x-only pubkey
  const payer = payerPubHex ? hexToBytes(payerPubHex) : secp256k1.getPublicKey(Buffer.from(payerPrivHex, "hex"), true).slice(1);

  const metadataPda = client.metadataPda(mint as any);
  console.log("Using PROGRAM_ID:", Buffer.from(programId).toString("hex"));
  console.log("Mint (new this run):", Buffer.from(mint).toString("hex"));
  console.log("Metadata PDA:", Buffer.from(metadataPda).toString("hex"));

  // Compose instructions
  const aplTokenProgramId = tokenProgramId();
  // Provide ample lamports to avoid rent issues on localnet
  const minLamports = 2_000_000_000n;
  const createMint: MetaInstruction = client.createMintAccountIx(
    payer as any,
    mint as any,
    aplTokenProgramId,
    minLamports
  );
  const initMint: MetaInstruction = client.tokenInitializeMint2Ix(
    aplTokenProgramId,
    mint as any,
    payer as any,
    undefined,
    9
  );
  const createMd = client.createMetadataIx({
    payer: payer as any,
    mint: mint as any,
    mintOrFreezeAuthority: payer as any,
    name: "Demo Token",
    symbol: "DT",
    image: "https://example.com/i.png",
    description: "demo",
    immutable: false,
  });
  const createAttrs = client.createAttributesIx({
    payer: payer as any,
    mint: mint as any,
    updateAuthority: payer as any,
    data: [
      ["rarity", "common"],
      ["series", "alpha"],
    ],
  });

  const recentBlockhashHex = await arch.getBestBlockHash();
  const recentBlockhash = hexToBytes(recentBlockhashHex);
  const metaInstructions: MetaInstruction[] = [
    createMint,
    initMint,
    createMd,
    createAttrs,
  ];
  const instructions: ArchInstruction[] = metaInstructions.map((ix) => ({
    program_id: ix.programId,
    accounts: ix.accounts.map((a) => ({
      pubkey: a.pubkey,
      is_signer: a.isSigner,
      is_writable: a.isWritable,
    })),
    data: ix.data,
  }));
  console.log(
    "Building instructions: [create_mint_account, initialize_mint2(decimals=9), create_metadata, create_attributes]"
  );

  // Build and sign message (payer + mint)
  const message = SanitizedMessageUtil.createSanitizedMessage(
    instructions,
    payer,
    recentBlockhash
  );
  if (typeof (message as any).header === "undefined") {
    throw new Error("Failed to compile message");
  }
  const msgHash = SanitizedMessageUtil.hash(message as any);
  // Produce BIP-322 signatures over the message hash for P2TR (single-key spend)
  // Derive P2TR single-key addresses locally for payer and mint on regtest
  const payerAddr = bitcoin.payments.p2tr({
    internalPubkey: Buffer.from(payer),
    network: bitcoin.networks.regtest,
  }).address!;
  const mintAddr = bitcoin.payments.p2tr({
    internalPubkey: Buffer.from(mint),
    network: bitcoin.networks.regtest,
  }).address!;
  // Convert raw privkeys (hex) to WIF for bip322-js. Use testnet/regtest WIF (0xEF) for tb1/bcrt1
  const wifVersionForAddress = (address: string): number =>
    address.startsWith("tb1") || address.startsWith("bcrt1") ? 0xef : 0x80;
  const payerPrivWif = wif.encode(
    wifVersionForAddress(payerAddr),
    Buffer.from(payerPrivHex, "hex"),
    true
  );
  const mintPrivHex = Buffer.from(mintPriv).toString("hex");
  const mintPrivWif = wif.encode(
    wifVersionForAddress(mintAddr),
    Buffer.from(mintPrivHex, "hex"),
    true
  );
  const payerSigB64 = Bip322Signer.sign(payerPrivWif, payerAddr, Buffer.from(msgHash));
  const mintSigB64 = Bip322Signer.sign(mintPrivWif, mintAddr, Buffer.from(msgHash));
  const payerSigRaw = Buffer.from(payerSigB64, "base64");
  const mintSigRaw = Buffer.from(mintSigB64, "base64");
  const tx: RuntimeTransaction = {
    version: 0,
    signatures: [
      SignatureUtil.adjustSignature(payerSigRaw),
      SignatureUtil.adjustSignature(mintSigRaw),
    ],
    message: message as any,
  };
  console.log("Submitting transaction...");
  const txid = await arch.sendTransaction(tx);
  console.log("submitted txid=", txid);
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
