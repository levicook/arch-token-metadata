Arch Token Metadata Roadmap

Phase 1 – Core (current)

- Add constants, PDA helpers, and exact size calculations
- Implement CreateMetadata (PDA via CPI only; allow idempotent write if PDA exists and is zero-initialized)
- Add e2e tests for CreateMetadata success and failures (authority mismatch, duplicate)

Phase 2 – Core updates

- Implement UpdateMetadata with authority enforcement (DONE)
  - Accounts: [metadata_pda (writable), update_authority (signer)]
  - Partial updates with cap revalidation; metadata must be initialized
- Implement CreateAttributes and ReplaceAttributes (DONE)
  - Limits: MAX_ATTRIBUTES=32, MAX_KEY_LENGTH=64, MAX_VALUE_LENGTH=240
  - PDA creation via CPI; allocate full max size upfront (<10KB growth)
  - ReplaceAttributes performs whole-vector replace; no realloc
- Implement TransferAuthority and MakeImmutable as separate instructions (DONE)
  - TransferAuthority requires explicit Pubkey (no Option)
  - MakeImmutable revokes update authority (irreversible)
- Expand tests to cover all invariants in SECURITY.md (DONE)

Phase 3 – Convenience and DX

- Instruction builders with full AccountMeta sets
- Client PDA helpers and minimal SDK examples
- Test utilities for mint/account setup (PARTIAL: helpers exist in tests)

Phase 4 – Docs and integration

- Add examples and integration guides
- Benchmarks and compute/cost sizing guidance
- Document e2e test coverage goals in tests
- Document PDA-only creation for metadata/attributes and attribute sizing rationale (10KB per-instruction growth) (DONE)
