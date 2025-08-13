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

### Transaction builders

- createTokenWithMetadataTx
- createTokenWithMetadataAndAttributesTx
- createTokenWithFreezeAuthMetadataTx
- Convenience: createAttributesTx, replaceAttributesTx, makeImmutableTx, transferAuthorityThenUpdateTx
- Budget-aware variants: add compute budget instructions automatically
  - createTokenWithMetadataTxWithBudget
  - createTokenWithMetadataAndAttributesTxWithBudget
  - createTokenWithFreezeAuthMetadataTxWithBudget
  - createAttributesTxWithBudget, replaceAttributesTxWithBudget, makeImmutableTxWithBudget, transferAuthorityThenUpdateTxWithBudget

All builders return Instruction[]. You may add compute budget or other instructions before submission.

### Readers

- `TokenMetadataReader` provides:
  - getTokenMetadata(mint)
  - getTokenMetadataAttributes(mint)
  - getTokenDetails(mint) → { metadata, attributes }
  - Batch variants for both, with strict owner checks

### Validation

- Names, symbols, image URI, description length caps mirror on-chain limits
- Attributes count/key/value length caps mirror on-chain limits

### Testing / Fixtures

Rust binary tools/arch-token-metadata-fixtures emits golden fixtures used by this package’s tests.

```
cargo run -p arch_token_metadata_fixtures
npm install
npm run build -w sdks/arch-token-metadata-sdk-ts
npm run test -w sdks/arch-token-metadata-sdk-ts
```

### Benchmarks and guidance

- See `docs/benchmarks/README.md` for current median CU usage and how to set compute budgets.

### Known limitations

-

### Well-known attribute keys

Use the following standard keys when adding attributes:

- twitter: "@handle" or full URL
- telegram: full invite URL
- website: canonical HTTPS URL
- discord: full invite URL
- coingecko: asset URL
- whitepaper: HTTPS URL to PDF or page
- audit: HTTPS URL to audit report
- category: short lowercase category
- tags: comma-separated list, e.g. "defi,yield"

Constants are exported on `TokenMetadataClient`:

```
TokenMetadataClient.ATTR_TWITTER
TokenMetadataClient.ATTR_TELEGRAM
TokenMetadataClient.ATTR_WEBSITE
TokenMetadataClient.ATTR_DISCORD
TokenMetadataClient.ATTR_COINGECKO
TokenMetadataClient.ATTR_WHITEPAPER
TokenMetadataClient.ATTR_AUDIT
TokenMetadataClient.ATTR_CATEGORY
TokenMetadataClient.ATTR_TAGS
```

Guidelines:

- Keys and values must be non-empty.
- Normalize URLs to https.
- Keep `tags` short; avoid spaces.

### Offline JSON metadata example

Client UIs and indexers may fetch additional JSON off-chain and merge with on-chain fields. Example:

```json
{
  "name": "Arch Pioneer Token",
  "symbol": "APT",
  "image": "https://arweave.net/abc123.png",
  "description": "The first token launched on Arch Network",
  "attributes": {
    "twitter": "@arch",
    "website": "https://arch.network",
    "tags": "defi,governance"
  }
}
```

Consumers should always prefer on-chain fields when present.

### Compatibility with Token Lists

For list-level aggregation, align with Uniswap Token Lists where possible:

- Use `logoURI` as an alias of `image` in list items
- Include `tags` (array or comma-separated string) per list conventions
- Schema references: `https://uniswap.org/tokenlist.schema.json`, `https://github.com/solana-labs/token-list`

- Compute budget instructions (SetComputeUnitLimit, RequestHeapFrame) are built and can be prepended via the `...WithBudget` builders. However, the current runtime does not enforce these requests; effective CU limits remain at defaults. Keep the helpers in your flow for forward-compatibility, but do not rely on them changing limits until runtime support is enabled.
