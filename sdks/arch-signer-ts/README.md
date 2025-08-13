Arch Signer (TypeScript)

Lightweight BIP-322 signing utilities for Arch transactions in TypeScript.

What this is

- A small helper library to produce valid BIP-322 signatures for Arch transactions.
- Focused on P2TR (taproot, single-key spend) on regtest/testnet/mainnet.
- Designed to pair with `@saturnbtcio/arch-sdk` (message/hash/submit) and higher-level SDKs that construct instructions.

What this is not

- A wallet or key manager. You must manage private keys securely.
- A transport client. Use `@saturnbtcio/arch-sdk` for RPC and message utilities.

Install

- In this monorepo, it is a workspace package named `arch-signer`.
- External consumers (when published) will be able to `npm install arch-signer`.

API

- `deriveP2trAddress(pubkeyXOnly: Uint8Array, network: 'regtest'|'testnet'|'mainnet'): string`
  - Derives a bech32m P2TR address (single-key spend) from a 32-byte x-only pubkey.

- `toWif(privHex: string, network: 'regtest'|'testnet'|'mainnet'): string`
  - Converts a hex-encoded secp256k1 private key to a compressed WIF for the given network.

- `signBip322P2tr(privHex: string, message: Uint8Array, network: 'regtest'|'testnet'|'mainnet'): Uint8Array`
  - Produces a 64-byte signature over `message` using BIP-322 (simple) for a P2TR single-key address derived from `privHex`.
  - Output is ready to pass through `SignatureUtil.adjustSignature` from `@saturnbtcio/arch-sdk` if needed by your flow.

Usage example (with Arch SDKs)

```ts
import {
  RpcConnection,
  ArchConnection,
  SanitizedMessageUtil,
  SignatureUtil,
  type Instruction as ArchInstruction,
  type RuntimeTransaction,
} from "@saturnbtcio/arch-sdk";
import {
  TokenMetadataClient,
  metadataProgramId,
  tokenProgramId,
} from "arch-token-metadata-sdk";
import { signBip322P2tr, deriveP2trAddress } from "arch-signer";

const provider = new RpcConnection("http://localhost:9002");
const arch = ArchConnection(provider);

const programId = metadataProgramId();
const client = new TokenMetadataClient(programId);

// payer/mint keys (32B x-only pubkeys) and payerPrivHex from your wallet/env
const payerPrivHex = process.env.PAYER_PRIVKEY!;
const payer = /* derive x-only pubkey from priv */ new Uint8Array(32);
const mint = /* new x-only pubkey */ new Uint8Array(32);

// Build instructions (example)
const aplTokenProgramId = tokenProgramId();
const createMint = client.createMintAccountIx(
  payer as any,
  mint as any,
  aplTokenProgramId,
  2_000_000_000n,
);
const initMint = client.tokenInitializeMint2Ix(
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
  image: "https://example/i.png",
  description: "demo",
  immutable: false,
});
const instructions: ArchInstruction[] = [createMint, initMint, createMd].map(
  (ix) => ({
    program_id: ix.programId,
    accounts: ix.accounts.map((a) => ({
      pubkey: a.pubkey,
      is_signer: a.isSigner,
      is_writable: a.isWritable,
    })),
    data: ix.data,
  }),
);

// Build + hash message
const recent = await arch.getBestBlockHash();
const msg = SanitizedMessageUtil.createSanitizedMessage(
  instructions,
  payer,
  Buffer.from(recent, "hex"),
);
if (!(msg as any).header) throw new Error("message compile failed");
const msgHash = SanitizedMessageUtil.hash(msg as any);

// BIP-322 sign (P2TR)
const payerSig = signBip322P2tr(payerPrivHex, msgHash, "regtest");
const mintPrivHex = "...";
const mintSig = signBip322P2tr(mintPrivHex, msgHash, "regtest");

const tx: RuntimeTransaction = {
  version: 0,
  signatures: [
    SignatureUtil.adjustSignature(payerSig),
    SignatureUtil.adjustSignature(mintSig),
  ],
  message: msg as any,
};

const txid = await arch.sendTransaction(tx);
console.log("submitted txid=", txid);
```

Notes

- Currently supports P2TR (taproot single-key spend). P2WPKH support can be added if needed.
- Network must match your node: `regtest` (localnet), `testnet`, or `mainnet`.
- Keep your private keys secure (dotenv, key vaults). Never commit secrets.

License

- MIT (same as the rest of the repo unless stated otherwise).
