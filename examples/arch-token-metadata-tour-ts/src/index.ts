import {
  TokenMetadataClient,
  metadataProgramId,
  tokenProgramId,
  type Instruction as MetaInstruction,
  TokenMetadataReader,
} from "arch-token-metadata-sdk";
import {
  RpcConnection,
  ArchConnection,
  SanitizedMessageUtil,
  SignatureUtil,
  type Instruction as ArchInstruction,
  type RuntimeTransaction,
} from "@saturnbtcio/arch-sdk";
import { deriveP2trAddress, signBip322P2tr } from "arch-signer-ts";
import { secp256k1 } from "@noble/curves/secp256k1";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

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
      "Missing env. Run examples/launch.sh first to generate examples/.env",
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
  const payer = payerPubHex
    ? hexToBytes(payerPubHex)
    : secp256k1.getPublicKey(Buffer.from(payerPrivHex, "hex"), true).slice(1);

  const metadataPda = client.metadataPda(mint as any);
  console.log("Using PROGRAM_ID:", Buffer.from(programId).toString("hex"));
  console.log("Mint (new this run):", Buffer.from(mint).toString("hex"));
  console.log("Metadata PDA:", Buffer.from(metadataPda).toString("hex"));
  // Log payer balance
  console.log("Payer:", Buffer.from(payer).toString("hex"));
  try {
    const ai = await provider.readAccountInfo(payer as any);
    console.log("Payer lamports:", ai.lamports);
  } catch (e) {
    console.log("Payer account not found or unreadable:", e);
  }

  // Compose instructions
  const aplTokenProgramId = tokenProgramId();
  // Provide lamports for mint account allocation (align with Rust's MIN_ACCOUNT_LAMPORTS scale)
  const minLamports = 3_000_000n;
  // Optional compute budget from env (no defaults; align with Rust example usage)
  const cuUnits = process.env.CU_UNITS
    ? parseInt(process.env.CU_UNITS, 10)
    : undefined;
  const heapBytes = process.env.HEAP_BYTES
    ? parseInt(process.env.HEAP_BYTES, 10)
    : undefined;
  const computeBudgetIxs: MetaInstruction[] = [];
  if (typeof cuUnits === "number") {
    computeBudgetIxs.push(
      (client as any).setComputeUnitLimitIx(cuUnits) as any,
    );
  }
  if (typeof heapBytes === "number") {
    computeBudgetIxs.push((client as any).requestHeapFrameIx(heapBytes) as any);
  }
  if (computeBudgetIxs.length > 0) {
    console.log(
      `Compute budget: units=${cuUnits ?? "(none)"} heapBytes=${
        heapBytes ?? "(none)"
      }`,
    );
  }
  const createMint: MetaInstruction = client.createMintAccountIx(
    payer as any,
    mint as any,
    aplTokenProgramId,
    minLamports,
  );
  const initMint: MetaInstruction = client.tokenInitializeMint2Ix(
    aplTokenProgramId,
    mint as any,
    payer as any,
    undefined,
    9,
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

  // Single-transaction flow aligned with Rust example
  const recentBlockhashHex = await arch.getBestBlockHash();
  const recentBlockhash = hexToBytes(recentBlockhashHex);
  const budget =
    computeBudgetIxs.length > 0 ? { units: cuUnits, heapBytes } : undefined;
  const metaInstructions: MetaInstruction[] = (
    client as any
  ).createTokenWithMetadataAndAttributesTxWithBudget(
    {
      payer: payer as any,
      mint: mint as any,
      mintAuthority: payer as any,
      freezeAuthority: undefined,
      decimals: 9,
      name: "Demo Token",
      symbol: "DT",
      image: "https://example.com/i.png",
      description: "demo",
      immutable: false,
      attributes: [
        ["rarity", "common"],
        ["series", "alpha"],
      ],
      mintInitializeInstructions: [createMint, initMint],
    },
    budget,
  ) as MetaInstruction[];
  const stepsLabel = `[${
    budget ? "compute_budget, " : ""
  }create_mint_account, initialize_mint2(decimals=9), create_metadata, create_attributes]`;
  console.log("Building instructions:", stepsLabel);
  const instructions: ArchInstruction[] = metaInstructions.map((ix) => ({
    program_id: ix.programId,
    accounts: ix.accounts.map((a) => ({
      pubkey: a.pubkey,
      is_signer: a.isSigner,
      is_writable: a.isWritable,
    })),
    data: ix.data,
  }));
  const message = SanitizedMessageUtil.createSanitizedMessage(
    instructions,
    payer,
    recentBlockhash,
  );
  if (typeof (message as any).header === "undefined") {
    throw new Error("Failed to compile message");
  }
  const msgHash = SanitizedMessageUtil.hash(message as any);
  const payerSigRaw = signBip322P2tr(payerPrivHex, msgHash, "regtest");
  const mintSigRaw = signBip322P2tr(
    Buffer.from(mintPriv).toString("hex"),
    msgHash,
    "regtest",
  );
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
  {
    const processedDeadline = Date.now() + 60_000;
    let processedOk = false;
    while (Date.now() < processedDeadline) {
      const pt = await provider.getProcessedTransaction(txid);
      if (pt) {
        if ((pt.status as any).type === "processed") {
          processedOk = true;
          break;
        }
        if ((pt.status as any).type === "failed") {
          console.error(
            "tx failed:",
            (pt.status as any).message,
            "logs:\n\t",
            pt.logs.join("\n\t"),
          );
          throw new Error("single tx failed");
        }
      }
      await new Promise((r) => setTimeout(r, 500));
    }
    if (!processedOk) throw new Error("single tx not confirmed within timeout");
  }

  // Post-submit: verify accounts via reader using RpcConnection as a simple adapter
  const reader = new TokenMetadataReader(
    programId as any,
    {
      async getMultipleAccounts(pubkeys: Uint8Array[]) {
        const res = await provider.getMultipleAccounts(pubkeys as any);
        return res.map((r: any) => {
          if (!r) return { data: null, owner: null };
          const data: any = r.data;
          const ownerAny: any = r.owner;
          const dataBytes: Uint8Array | null =
            data == null
              ? null
              : typeof data === "string"
                ? hexToBytes(data)
                : Array.isArray(data)
                  ? new Uint8Array(data)
                  : data instanceof Uint8Array
                    ? data
                    : null;
          const ownerBytes: Uint8Array | null =
            ownerAny == null
              ? null
              : typeof ownerAny === "string"
                ? hexToBytes(ownerAny)
                : Array.isArray(ownerAny)
                  ? new Uint8Array(ownerAny)
                  : ownerAny instanceof Uint8Array
                    ? ownerAny
                    : null;
          return { data: dataBytes, owner: ownerBytes } as any;
        });
      },
    } as any,
  );
  const details = await reader.getTokenDetails(mint as any);
  console.log("metadata:", details.metadata);
  console.log("attributes:", details.attributes);
}

// Helper function to convert hex string to Uint8Array
function hexToBytes(hex: string): Uint8Array {
  const clean = hex.startsWith("0x") ? hex.slice(2) : hex;
  const out = new Uint8Array(clean.length / 2);
  for (let i = 0; i < clean.length; i += 2)
    out[i / 2] = parseInt(clean.slice(i, i + 2), 16);
  return out;
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
