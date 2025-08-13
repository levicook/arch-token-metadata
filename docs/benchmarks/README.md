### Arch Token Metadata – Benchmarks and Compute Budget Guidance

This page summarizes compute unit (CU) usage measured on local regtest using the Rust bench harness. Results are medians over repeated runs; your mileage may vary by runtime version and surrounding instructions.

Bench source: `benchmarks/token-metadata-benches` → prints JSON and updates `docs/benchmarks/report.json`.

#### Current medians (CU)

- create_metadata: 7456
- create_metadata_and_attributes: 8477
- update_metadata: 2412
- replace_attributes: 5149
- transfer_authority: 2111
- make_immutable: 2065

Full-flow (builders including mint + initialize + metadata ops)

- full_create_token_with_metadata: 7456
- full_create_token_with_metadata_and_attributes: 8477
- full_create_token_with_freeze_auth_metadata: 7455

Note: The “full” flows above are currently dominated by the metadata operations in this program. System/Token instructions may change in overhead as the runtime evolves.

#### How to run benchmarks

```bash
cargo run -p token-metadata-benches
# Output is printed to stdout and were redirected to create `docs/benchmarks/report.json`
```

Required env (created by examples/ scripts): `ARCH_RPC`, `PAYER_PUBKEY`, `PAYER_PRIVKEY`, optional `PROGRAM_ID`, and optional `WARMUP_ITERS`, `ITERS`.

#### Using compute budgets (Rust)

You can prepend compute-budget instructions via the SDK’s “with_budget” builders.

```rust
use arch_token_metadata_sdk::{
  TokenMetadataClient, ComputeBudgetOptions, TxCreateTokenWithMetadataAndAttributesParams
};

let budget = ComputeBudgetOptions { units: Some(12_000), heap_bytes: Some(64 * 1024) };
let ixs = client.create_token_with_metadata_and_attributes_tx_with_budget(
  TxCreateTokenWithMetadataAndAttributesParams { /* ... */ },
  budget,
)?;
```

If you only need budget instructions:

```rust
let cu = client.set_compute_unit_limit_ix(12_000);
let heap = client.request_heap_frame_ix(64 * 1024);
```

#### Using compute budgets (TypeScript)

The TS SDK mirrors the helpers and adds `...WithBudget` variants.

```ts
const budget = { units: 12000, heapBytes: 64 * 1024 };
const ixs = client.createTokenWithMetadataAndAttributesTxWithBudget(
  params,
  budget,
);
// or individual helpers
const cu = client.setComputeUnitLimitIx(12000);
const heap = client.requestHeapFrameIx(64 * 1024);
```

#### Guidance

- Do not hard-code CU limits in applications based on these exact medians. Leave buffer (e.g., +20–30%) when setting a limit for production.
- Re-run the bench harness after upgrading runtime or program versions.

#### Runtime note on Compute Budget

As of the current runtime, compute budget instructions are accepted but not enforced. The helpers in both SDKs produce correct bytes and are available behind `*_with_budget` builders, but measured CU and observed per-instruction caps reflect default node limits. This document will be updated when enforcement lands; until then, treat budgets as advisory/no-op at execution time.
