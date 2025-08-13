### Arch Token Metadata: Implementation Status vs Proposal

Reference: [proposal](https://gist.githubusercontent.com/levicook/c991a39a50543117b3fd557b605a53d9/raw/c7c7bb2c428d6ca4bafd3cc14059320d1537639d/arch-token-metadata.md)

#### Implemented (matches proposal)

- Core account: `TokenMetadata { mint, name, symbol, image, description, update_authority }`
- Optional attributes account: `TokenMetadataAttributes { mint, data: Vec<(String, String)> }`
- PDA seeds: `b"metadata"`, `b"attributes"`
- Authority model:
  - Create: mint authority, or freeze authority if mint authority is None
  - Unified update authority (controls both metadata and attributes)
  - Transfer authority, and immutable (revoke authority)
- Instructions: `CreateMetadata`, `UpdateMetadata`, `CreateAttributes`, `ReplaceAttributes`, `TransferAuthority`, `MakeImmutable`
- SDKs (Rust and TypeScript): instruction builders, PDA helpers, readers, and transaction composers

#### Deliberate deviations

- Attribute value cap: `MAX_VALUE_LENGTH = 240` (proposal said 512). Rationale: account size/runtime limits. SDKs mirror 240.

#### Recent changes

- Attributes validation tightened (on-chain and SDKs):
  - Reject empty keys/values
  - Return `TooManyAttributes` when `data.len() > MAX_ATTRIBUTES`
- SDKs export well-known attribute key constants; README guidelines added:
  - `twitter, telegram, website, discord, coingecko, whitepaper, audit, category, tags`
- Offline JSON example and guidance added in SDK READMEs
- Token Lists compatibility notes (use `logoURI` alias, tags guidance; see `https://uniswap.org/tokenlist.schema.json`, `https://github.com/solana-labs/token-list`)

#### Deferred

- "Touch" instruction to signal indexers:
  - Option A (breaking layout): add `last_touched_slot: u64` to `TokenMetadata`
  - Option B (non-breaking): separate PDA at seed `b"touch"` storing the slot
  - Recommendation: defer; prefer Option B if demand arises

#### Validation limits (current)

- `NAME_MAX_LEN=256`, `SYMBOL_MAX_LEN=16`, `IMAGE_MAX_LEN=512`, `DESCRIPTION_MAX_LEN=512`
- Attributes: `MAX_ATTRIBUTES=32`, `MAX_KEY_LENGTH=64`, `MAX_VALUE_LENGTH=240`

#### Notes for integrators

- Core fields are optimized for fast reads; attributes are optional and preallocated to max size for replacement without reallocation
- Prefer on-chain fields; use offline JSON only to enrich UI where applicable
