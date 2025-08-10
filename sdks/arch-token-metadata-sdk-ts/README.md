Arch Token Metadata – TypeScript SDK

Lightweight client helpers for Arch Token Metadata:

- PDA helpers for metadata and attributes
- Instruction builders with client-side validation (mirror on-chain limits)
- Transaction builders that compose upstream system/token instructions with metadata flows
- Serialization tests against Rust golden fixtures

This SDK does not submit transactions. For submission, use `@saturnbtcio/arch-sdk` and pass the `Instruction[]` returned by these builders into your message/transaction flow.

Install

- Install this package and `@saturnbtcio/arch-sdk` in your app.

Usage

```ts
import {
  TokenMetadataClient,
  type Instruction,
  metadataProgramId,
  tokenProgramId,
} from "@arch/arch-token-metadata-sdk-ts";
import {
  ArchConnection,
  RpcConnection,
  SanitizedMessageUtil,
} from "@saturnbtcio/arch-sdk";

// Setup (you must supply real keys):
// - Use your wallet/provider to derive/generate payer & mint keypairs
// - Ensure payer is funded
// For simplicity this example assumes you already have Uint8Array(32) public keys.
const payer: Uint8Array = /* your payer pubkey */ new Uint8Array(32);
const mint: Uint8Array = /* your mint pubkey */ new Uint8Array(32);
const mintAuthority: Uint8Array = payer;

const client = new TokenMetadataClient(metadataProgramId());

const createMint: Instruction = client.createMintAccountIx(
  payer,
  mint,
  tokenProgramId(),
  1n,
);
const initMint: Instruction = client.tokenInitializeMint2Ix(
  tokenProgramId(),
  mint,
  mintAuthority,
  undefined,
  9,
);

const createMd: Instruction = client.createMetadataIx({
  payer,
  mint,
  mintOrFreezeAuthority: mintAuthority,
  name: "Name",
  symbol: "SYM",
  image: "https://i",
  description: "desc",
  immutable: false,
});

const ixs: Instruction[] = [createMint, initMint, createMd];

const provider = new RpcConnection({ nodeUrl: "http://localhost:8899" });
const arch = ArchConnection(provider);
const payerPubkey = payer; // Uint8Array(32)
const recentBlockhash = await arch.rpc.getBestBlockhash();

const message = SanitizedMessageUtil.createSanitizedMessage(
  ixs,
  payerPubkey,
  recentBlockhash,
);
const messageHash = SanitizedMessageUtil.hash(message);
// Sign messageHash with your wallet (BIP322), then:
// const signature = SignatureUtil.adjustSignature(yourSignature)
// const tx = { version: 1, signatures: [signature], message };
// const txid = await arch.sendTransaction(tx);
```

Transaction builders

- createTokenWithMetadataTx / createTokenWithMetadataTxWithPdas
- createTokenWithMetadataAndAttributesTx / ...WithPdas
- createTokenWithFreezeAuthMetadataTx
- Convenience: createAttributesTx, replaceAttributesTx, makeImmutableTx, transferAuthorityThenUpdateTx

All builders return Instruction[]. You may add compute budget or other instructions before submission.

Validation

- Names, symbols, image URI, description length caps mirror on-chain limits
- Attributes count/key/value length caps mirror on-chain limits

Testing / Fixtures

Rust binary tools/arch-token-metadata-fixtures emits golden fixtures used by this package’s tests.

```
cargo run -p arch_token_metadata_fixtures
npm install
npm run build -w sdks/arch-token-metadata-sdk-ts
npm run test -w sdks/arch-token-metadata-sdk-ts
```
