# Arch Token Metadata CLI

Secure, ergonomic CLI for interacting with the Arch Token Metadata program.

- No secrets in shell history by default.
- Mirrors the Rust SDK instruction builders and reader helpers.

## Install

From workspace root:

```bash
cargo run -p arch-token-metadata-cli -- --help | cat
```

## Global options

- `--rpc` (defaults to `ARCH_RPC` or `http://localhost:9002`)
- `--program-id` hex32 (defaults to `PROGRAM_ID` else baked id)
- `--cu-units` u32, `--heap-bytes` u32 (optional)
- `--json`

## Secure signer sources

Provide signers as sources rather than inline keys:

- `prompt` – hidden TTY prompt (default)
- `stdin` – read once from stdin (pipe from a secret manager)
- `file:/path` – read from file (enforce 0600 on your own)
- `env:VAR` – read from an env var (allowed, warned)

Secrets are never logged; in-memory buffers are zeroized after use.

If a role-specific signer is omitted, it defaults to the payer signer.

## Subcommands

- `create-metadata --mint HEX --name NAME --symbol SYM --image URI --description DESC [--immutable] --payer SOURCE [--mint-authority SOURCE]`
- `update-metadata --mint HEX [--name ...] [--symbol ...] [--image ...] [--description ...] --payer SOURCE [--update-authority SOURCE]`
- `create-attributes --mint HEX --kv k=v --kv k=v ... --payer SOURCE [--update-authority SOURCE]`
- `replace-attributes --mint HEX --kv k=v ... --payer SOURCE [--update-authority SOURCE]`
- `transfer-authority --mint HEX --new-authority HEX --payer SOURCE [--current-update-authority SOURCE]`
- `make-immutable --mint HEX --payer SOURCE [--current-update-authority SOURCE]`
- Readers: `get-metadata --mint HEX`, `get-attributes --mint HEX`, `get-details --mint HEX`

## Examples

Prompt for secrets (nothing stored):

```bash
arch-metadata create-metadata \
  --rpc http://localhost:9002 \
  --mint cf95c8...d3a4f7 \
  --name "My Token" \
  --symbol "MTK" \
  --image "https://example.com/logo.png" \
  --description "hello" \
  --payer prompt \
  --mint-authority prompt
```

Pipe from a secret manager:

```bash
op read "op://vault/arch/payer-privkey" | \
arch-metadata update-metadata \
  --mint cf95c8...d3a4f7 \
  --symbol NEW \
  --payer stdin \
  --update-authority stdin
```

Read and verify:

```bash
arch-metadata get-details --mint cf95c8...d3a4f7 --json
```

## Notes

- Length limits: NAME<=256, SYMBOL<=16, IMAGE<=512, DESCRIPTION<=512.
- Create requires mint or freeze authority. Update/attributes require update authority.
- `--cu-units` and `--heap-bytes` are accepted and forwarded, but current runtime may not enforce them yet.
