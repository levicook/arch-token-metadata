Arch Token Metadata Roadmap

Phase 1 – Core (current)
- Add constants, PDA helpers, and exact size calculations
- Implement CreateMetadata (PDA via CPI only; allow idempotent write if PDA exists and is zero-initialized)
- Add e2e tests for CreateMetadata success and failures (authority mismatch, duplicate)

Phase 2 – Core updates
- Implement UpdateMetadata with authority enforcement
- Implement CreateAttributes and ReplaceAttributes
- Implement TransferAuthority
- Expand tests to cover all invariants in SECURITY.md

Phase 3 – Convenience and DX
- Instruction builders with full AccountMeta sets
- Client PDA helpers and minimal SDK examples
- Test utilities for mint/account setup

Phase 4 – Docs and integration
- Add examples and integration guides
- Benchmarks and compute/cost sizing guidance
- Document e2e test coverage goals in tests


