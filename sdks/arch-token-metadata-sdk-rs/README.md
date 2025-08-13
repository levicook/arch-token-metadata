### Arch Token Metadata – Rust SDK

#### What this provides

- PDA helpers for metadata and attributes
- Instruction builders mirroring on-chain invariants
- Transaction builders for common flows
- Reader utilities for fetching and decoding accounts (with injected async RPC)

#### Quickstart

```rust
use arch_token_metadata_sdk::{TokenMetadataClient, CreateMetadataParams};

let client = TokenMetadataClient::new(program_id);
let ix = client.create_metadata_ix(CreateMetadataParams{
  payer,
  mint,
  mint_or_freeze_authority: payer,
  name: "Name".into(), symbol: "SYM".into(), image: "https://i".into(), description: "desc".into(),
  immutable: false,
})?;
```

#### Transaction builders

- create_token_with_metadata_tx
- create_token_with_metadata_and_attributes_tx
- create_token_with_freeze_auth_metadata_tx
- Convenience: create_attributes_tx, replace_attributes_tx, make_immutable_tx, transfer_authority_then_update_tx

Budget-aware variants (prepend compute budget instructions automatically):

- ...\_tx_with_budget(..., ComputeBudgetOptions)

```rust
use arch_token_metadata_sdk::{TokenMetadataClient, ComputeBudgetOptions};
let budget = ComputeBudgetOptions { units: Some(12_000), heap_bytes: Some(64 * 1024) };
let ixs = client.create_token_with_metadata_tx_with_budget(params, budget)?;
```

#### Readers

`TokenMetadataReader<Rpc>` exposes:

- get_token_metadata(mint)
- get_token_metadata_attributes(mint)
- get_token_details(mint)
- Batch variants for both

#### Validation limits

- NAME_MAX_LEN=256, SYMBOL_MAX_LEN=16, IMAGE_MAX_LEN=512, DESCRIPTION_MAX_LEN=512
- Attributes: MAX_ATTRIBUTES=32, MAX_KEY_LENGTH=64, MAX_VALUE_LENGTH=240

#### Benchmarks

- See `docs/benchmarks/README.md` for current medians and compute budget guidance.

#### Known limitations

- Compute budget instructions (SetComputeUnitLimit, RequestHeapFrame) are correctly encoded and can be prepended via the `*_with_budget` builders, but as of the current runtime they are not enforced. Transactions still run under the node’s default compute limits. This will be updated once runtime support is enabled; until then, treat budget helpers as no-ops at execution time.

#### Well-known attribute keys

This crate exports `well_known_attributes::*` constants:

- TWITTER, TELEGRAM, WEBSITE, DISCORD, COINGECKO, WHITEPAPER, AUDIT, CATEGORY, TAGS

Guidelines:

- Keys and values must be non-empty.
- Normalize URLs to https.
- Keep `tags` short and comma-separated.

#### Offline JSON metadata example

Indexers and UIs may fetch additional JSON off-chain and merge with on-chain fields. Example:

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

#### Compatibility with Token Lists

For list-level aggregation, align with Uniswap Token Lists where possible:

- Use `logoURI` as an alias of `image` in list items
- Include `tags` (array or comma-separated string) per list conventions
- Schema references: `https://uniswap.org/tokenlist.schema.json`, `https://github.com/solana-labs/token-list`
