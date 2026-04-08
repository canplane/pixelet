#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")" && pwd)"
ENGINE="$ROOT/packages/pxl/engine/rust"

echo "==> pxl-engine (client, web)"
wasm-pack build "$ENGINE" \
  --target web \
  --out-dir "$ROOT/apps/web/public/wasm/pxl-engine" \
  --dev \
  -- --features client

echo "==> pxl-engine (server, api)"
wasm-pack build "$ENGINE" \
  --target web \
  --out-dir "$ROOT/apps/api/public/wasm/pxl-engine" \
  --dev \
  -- --features server

echo "==> done"
