## Examples

Prerequisites:

- Docker and Docker Compose
- Rust toolchain (nightly not required)
- Node.js (for future TS example)

Setup and launch:

1. Start the local stack and generate funded keys:
   - `./examples/launch.sh`
   - This starts `bitcoind`, `titan`, and `local_validator` on ports 18443/8080/9002.
   - It waits for the validator RPC to be ready, then runs wallet-setup to write `examples/.env`.

2. Run the Rust metadata CLI end-to-end:
   - `cargo run -p arch_token_metadata_example_rust_cli`
   - This loads `examples/.env`, builds `[create_mint, initialize_mint2, create_metadata]`, signs, submits, waits for Processed, and prints PDAs + txid.

Environment:

- `examples/.env` contains `PAYER_PRIVKEY`, `PAYER_PUBKEY`, `MINT_PRIVKEY`, `MINT_PUBKEY`, and `ARCH_RPC`.

Troubleshooting:

- If `launch.sh` stalls, check Docker logs for `bitcoind`, `titan`, and `local_validator`.
- Ensure `http://localhost:9002` responds to JSON-RPC. The script polls `get_block_count` for readiness.
- If faucet funding fails, rerun `./examples/launch.sh` to retry.

Compute budget note:

- Both Rust and TS tours optionally prepend compute budget instructions when `CU_UNITS`/`HEAP_BYTES` env vars are set. As of the current runtime, these instructions are not enforced, so effective limits remain the defaults and adding them will not raise the cap. Keep the env flags and helpers for forward-compatibility; behavior will change once runtime enforcement is enabled.
