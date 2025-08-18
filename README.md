## Arch Token Metadata

Lightweight, fast, and standards-minded metadata program for the Arch Network with Rust and TypeScript SDKs. This README focuses on using the program and SDKs, with a concise maintenance section at the end.

### What you can do

- Create core token metadata: name, symbol, image URI, description
- Update metadata (when not immutable)
- Attach and replace extensible attributes (key/value pairs)
- Transfer update authority or make metadata immutable

Core account and PDA seeds:

- `TokenMetadata` PDA: seed `"metadata"`
- `TokenMetadataAttributes` PDA: seed `"attributes"`

Program ID (baked): returned by `arch_token_metadata::id()` and `metadataProgramId()` in SDKs. You can override with `PROGRAM_ID` in env for local deployments.

## Quickstart (localnet)

### Prerequisites

- Docker + Docker Compose
- Rust toolchain
- Node.js (for the TS SDK and TS tour)

### 1) Launch local stack and generate a funded payer

```bash
./examples/launch.sh
```

This starts `bitcoind`, `titan`, and `local_validator`, waits for RPC readiness, optionally builds the program ELF, deploys via the setup tool, and writes `examples/.env` with:

- `ARCH_RPC` (e.g. http://localhost:9002)
- `PAYER_PRIVKEY`, `PAYER_PUBKEY`
- `MINT_PRIVKEY`, `MINT_PUBKEY` (example)
- `PROGRAM_ID` (set when a local program is deployed by the setup tool)

### 2a) Run the Rust tour (end-to-end)

```bash
cargo run -p arch_token_metadata_tour
```

What it does:

- Builds `[optional compute_budget], create_mint_account, initialize_mint2(decimals=9), create_metadata, create_attributes`
- Signs and submits the transaction
- Waits for processed status and verifies on-chain accounts via the Reader

Environment flags (optional):

- `CU_UNITS` – compute unit limit to request
- `HEAP_BYTES` – requested heap frame bytes

Note: current runtime does not enforce compute budget requests; helpers are included for forward-compatibility.

### 2b) Run the TypeScript tour (end-to-end)

```bash
npm run build -w examples/arch-token-metadata-tour-ts
npm run start -w examples/arch-token-metadata-tour-ts
```

It performs an equivalent single-transaction flow and verifies metadata via the Reader.

## Using the SDKs

### Rust SDK

```rust
use arch_program::pubkey::Pubkey;
use arch_token_metadata_sdk::{TokenMetadataClient, CreateMetadataParams};

let program_id: Pubkey = arch_token_metadata::id();
let client = TokenMetadataClient::new(program_id);

let ix = client.create_metadata_ix(CreateMetadataParams {
    payer,
    mint,
    mint_or_freeze_authority: payer,
    name: "MyToken".into(),
    symbol: "MTK".into(),
    image: "https://example.com/logo.png".into(),
    description: "hello".into(),
    immutable: false,
})?;
```

Common transaction builders:

- `create_token_with_metadata_tx`
- `create_token_with_metadata_and_attributes_tx`
- `create_token_with_freeze_auth_metadata_tx`
- Budget-aware variants: `*_tx_with_budget`

Readers:

- `get_token_metadata(mint)`
- `get_token_metadata_attributes(mint)`
- `get_token_details(mint)`

Validation limits (mirrors on-chain):

- `NAME_MAX_LEN=256`, `SYMBOL_MAX_LEN=16`, `IMAGE_MAX_LEN=512`, `DESCRIPTION_MAX_LEN=512`
- Attributes: `MAX_ATTRIBUTES=32`, `MAX_KEY_LENGTH=64`, `MAX_VALUE_LENGTH=240`

### TypeScript SDK

Install in your app:

```bash
npm install arch-token-metadata-sdk @saturnbtcio/arch-sdk
```

Usage:

```ts
import {
  TokenMetadataClient,
  metadataProgramId,
} from "arch-token-metadata-sdk";

const client = new TokenMetadataClient(metadataProgramId());
const ix = client.createMetadataIx({
  payer,
  mint,
  mintOrFreezeAuthority: payer,
  name: "MyToken",
  symbol: "MTK",
  image: "https://example.com/logo.png",
  description: "hello",
  immutable: false,
});
// Compose with your token minting instructions, build a message with @saturnbtcio/arch-sdk, sign (e.g., BIP-322), and submit
```

Convenience builders (all return `Instruction[]`):

- `createTokenWithMetadataTx`
- `createTokenWithMetadataAndAttributesTx`
- `createTokenWithFreezeAuthMetadataTx`
- Budget-aware: `...WithBudget` variants

Reader helpers:

- `getTokenMetadata(mint)`
- `getTokenMetadataAttributes(mint)`
- `getTokenDetails(mint)`

## Program instruction set

- CreateMetadata { name, symbol, image, description, immutable }
- UpdateMetadata { name?, symbol?, image?, description? }
- CreateAttributes { data: Vec<(String, String)> }
- ReplaceAttributes { data: Vec<(String, String)> }
- TransferAuthority { new_authority }
- MakeImmutable

Authority model:

- Create: mint authority, or freeze authority if mint authority is None
- A unified update authority controls both metadata and attributes
- `MakeImmutable` revokes update authority permanently

## Repo layout

- `programs/arch-token-metadata` – on-chain program
- `sdks/arch-token-metadata-sdk-rs` – Rust SDK
- `sdks/arch-token-metadata-sdk-ts` – TypeScript SDK
- `examples/arch-token-metadata-tour-rs` – Rust end-to-end tour
- `examples/arch-token-metadata-tour-ts` – TypeScript end-to-end tour
- `program-tests/arch-token-metadata-tests` – integration tests
- `benchmarks/token-metadata-benches` – CU benchmarks
- `docs/` – status, roadmap, security, and benchmarks report

## Maintenance

### Build program (optional, local deploy)

`examples/launch.sh` attempts to build an ELF for local deploy. Manually:

```bash
cargo build-sbf --manifest-path programs/arch-token-metadata/Cargo.toml --sbf-out-dir examples/.tmp/.sbf-out
```

Set `PROGRAM_ID` in your env when submitting transactions against a locally deployed program.

### Tests

Rust integration tests:

```bash
cargo test -p arch-token-metadata-tests -- --nocapture
```

TypeScript SDK tests (will regenerate golden fixtures):

```bash
cargo run -p arch_token_metadata_fixtures
npm run test -w sdks/arch-token-metadata-sdk-ts
```

### Benchmarks

```bash
cargo run -p token-metadata-benches
```

See benchmark notes in `docs/benchmarks/README.md` and `docs/benchmarks/report.json`.

### Security, roadmap, implementation status

- Security invariants: see `docs/SECURITY.md`
- Implementation status vs proposal: see `docs/IMPLEMENTATION_STATUS.md`
- Roadmap: see `docs/ROADMAP.md`

### Notes on compute budget

The SDKs can prepend compute budget instructions (SetComputeUnitLimit, RequestHeapFrame). Current runtime does not enforce these requests yet; effective limits remain defaults. Keep helpers in flows for forward-compatibility.
