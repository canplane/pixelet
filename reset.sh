#!/usr/bin/env bash
set -euo pipefail

echo "🚀 Starting full project reset for Deno workspace..."

echo "🗑️ Purging build artifacts and transient caches..."
find . -name "node_modules" -type d -prune -exec rm -rf '{}' +
find . -name "dist" -type d -prune -exec rm -rf '{}' +
find . -name "build" -type d -prune -exec rm -rf '{}' +
find . -name ".cache" -type d -prune -exec rm -rf '{}' +
find . -name ".vite" -type d -prune -exec rm -rf '{}' +

echo "✅ Running workspace checks..."
deno task check

echo "🔨 Running clean builds..."
deno task build:api
deno task build:web

echo "✨ Reset completed."