#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")"

echo "Starting docker-compose stack..."
docker compose -f docker-compose.yml up -d

echo "Waiting for Arch RPC to be ready on http://localhost:9002 ..."
until curl -sS -H 'content-type: application/json' --data '{"jsonrpc":"2.0","id":"1","method":"get_block_count"}' http://localhost:9002 >/dev/null 2>&1; do
  sleep 2
  echo -n "."
done
echo ""

echo "Generating and funding wallet via wallet-setup example..."
pushd .. >/dev/null

cargo run -p setup-payer-and-program
popd >/dev/null

echo "Wrote examples/.env with funded PAYER."