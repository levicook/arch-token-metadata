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

- UpdateMetadata
  - Accounts (strict order):
    - metadata_pda (writable)
    - update_authority (readonly, signer)
  - The metadata account must be initialized
  - Stored update_authority must be Some and match signer
  - Field caps re-validated; partial updates only

- CreateAttributes
  - Accounts (strict order):
    - payer (writable, signer)
    - system_program (readonly)
    - mint (readonly, owned by Token program; must match metadata.mint)
    - attributes_pda (writable)
    - update_authority (readonly, signer)
    - metadata_pda (readonly)
  - PDA checks as above
  - Key/value caps: key<=64, value<=240, entries<=32; no empty keys/values
  - Size/creation constraints:
    - Program creates attributes PDA via CPI using invoke_signed with seeds ["attributes", mint, bump]
    - Allocation respects per-instruction growth limits (~10KB). The program allocates the full maximum size upfront to avoid any future reallocation during replacements
  - Not already initialized

- ReplaceAttributes
  - Accounts (strict order):
    - attributes_pda (writable)
    - update_authority (readonly, signer)
    - metadata_pda (readonly)
  - PDA checks as above (attributes PDA is derived from metadata.mint)
  - Stored update_authority in metadata must be Some and match signer
  - Replace whole vector; caps re-validated (key<=64, value<=240, entries<=32)
  - No reallocation during update; account size must remain unchanged

- TransferAuthority
  - Accounts: [metadata_pda (writable), current_update_authority (signer)]
  - Stored update_authority must be Some and match signer
  - Set to new authority (Some)

- MakeImmutable
  - Accounts: [metadata_pda (writable), current_update_authority (signer)]
  - Stored update_authority must be Some and match signer
  - Set update_authority = None (irreversible)

Common

- All PDAs derived using seeds ["metadata"|"attributes", mint]
- Create PDAs via CPI only; client preallocation is not supported for PDAs. Allow idempotent writes when PDA already exists and is zero-initialized.
- Attribute limits profile (fits under 10KB growth per instruction): MAX_ATTRIBUTES=32, MAX_KEY_LENGTH=64, MAX_VALUE_LENGTH=240
- Prevent re-initialization
- Cross-check mints for all related accounts (owned by Token program and initialized where applicable)
