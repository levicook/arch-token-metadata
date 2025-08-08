Arch Token Metadata Security Invariants

This document lists the security requirements enforced by the Arch Token Metadata program.

Instruction invariants

- CreateMetadata

  - Accounts (strict order):
    - payer (writable, signer)
    - system_program (readonly, must equal System Program ID)
    - mint (readonly, owned by Token program; must be initialized)
    - metadata_pda (writable)
    - authority (readonly, signer)
  - PDA and account ownership:
    - metadata_pda must equal PDA(["metadata", mint], program_id)
    - Program creates the PDA via CPI to system program using invoke_signed with seeds ["metadata", mint, bump].
    - Clients cannot pre-create PDA accounts. The program may accept idempotent writes if the PDA already exists, is program-owned, and zero-initialized.
  - Authority model (anti-squatting):
    - If mint.mint_authority is Some(A): authority == A and is a signer
    - Else if mint.mint_authority is None and mint.freeze_authority is Some(F): authority == F and is a signer
    - Else: reject
  - Update authority storage and immutability:
    - Instruction carries immutable: bool
    - If immutable == true: store update_authority = None
    - Else: store update_authority = Some(matched_authority)
  - Field caps: name<=256, symbol<=16, image<=512, description<=512
  - Not already initialized: metadata account must be zero-initialized (first byte == 0)
  - Additional checks: payer must be a signer; system_program must match canonical ID

- UpdateMetadata (to be implemented)

  - Accounts: [mint, metadata_pda (writable), update_authority (signer)]
  - PDA checks as above
  - Stored update_authority must be Some and match signer
  - Field caps re-validated; partial updates only

- CreateAttributes (to be implemented)

  - Accounts: [mint, attributes_pda (writable), mint_authority (signer)]
  - PDA checks as above
  - Key/value caps: key<=64, value<=512, entries<=32; no empty keys/values
  - Not already initialized

- ReplaceAttributes (to be implemented)

  - Accounts: [mint, attributes_pda (writable), metadata_pda (readonly), update_authority (signer)]
  - PDA checks as above
  - Stored update_authority in metadata must be Some and match signer
  - Replace whole vector; caps re-validated

- TransferAuthority (to be implemented)
  - Accounts: [mint, metadata_pda (writable), current_update_authority (signer)]
  - PDA checks as above
  - Stored update_authority must be Some and match signer
  - Set to new authority (Some) or None (immutable)

Common

- All PDAs derived using seeds ["metadata"|"attributes", mint]
- Create PDAs via CPI only; client preallocation is not supported for PDAs. Allow idempotent writes when PDA already exists and is zero-initialized.
- Prevent re-initialization
- Cross-check mints for all related accounts (owned by Token program and initialized where applicable)
