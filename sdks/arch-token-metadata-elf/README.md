# arch_token_metadata_elf

Prebuilt ELF binary for the Arch Token Metadata on-chain program.

- This crate embeds the compiled program binary at compile time using `include_bytes!`.
- The build script prefers a packaged `elf/arch_token_metadata.so` (included in the crate),
  and falls back to a workspace-built artifact during local development.

Usage:

```rust
use arch_token_metadata_elf::ARCH_TOKEN_METADATA_ELF;

// ARCH_TOKEN_METADATA_ELF is a `&'static [u8]` of the ELF contents
let bytes: &[u8] = ARCH_TOKEN_METADATA_ELF;
```

Publishing notes:

- Ensure `elf/arch_token_metadata.so` exists before publishing. In this workspace, build the program with:
  `cargo build-sbf --manifest-path programs/arch-token-metadata/Cargo.toml`
  and copy the output to `sdks/arch-token-metadata-elf/elf/arch_token_metadata.so`.
