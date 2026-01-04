#!/bin/bash
set -e

echo "🚀 Starting full project initialization and infrastructure reset..."

# ---------------------------------------------------------
# 1. Physical Infrastructure Purge
# Ensures a clean slate by removing all build artifacts and 
# dependency links using native OS commands.
# ---------------------------------------------------------
echo "🗑️  Purging all physical artifacts (node_modules, dist, cache)..."
find . -name "node_modules" -type d -prune -exec rm -rf '{}' +
find . -name "dist" -type d -prune -exec rm -rf '{}' +
find . -name "build" -type d -prune -exec rm -rf '{}' +
find . -name ".cache" -type d -prune -exec rm -rf '{}' +
find . -name "*.tsbuildinfo" -type f -delete

# # ---------------------------------------------------------
# # 2. Source-Level Artifact Clean (Shared Package Focus)
# # Removes generated .js and .d.ts files sitting next to .ts sources
# # especially in the core package which uses composite mode.
# # ---------------------------------------------------------
# echo "🧹 Cleaning stray build artifacts from source directories..."
# # src 폴더 내부에 생성된 .js, .d.ts, .js.map, .d.ts.map 파일만 타겟팅
# find ./packages/core/src -name "*.js" -type f -delete
# find ./packages/core/src -name "*.d.ts" -type f -delete
# find ./packages/core/src -name "*.map" -type f -delete

# ---------------------------------------------------------
# 3. Environment Reconstruction
# Reinstalls dependencies based on the lockfile.
# ---------------------------------------------------------
echo "📦 Installing dependencies (pnpm install)..."
pnpm store prune
pnpm install --frozen-lockfile

# ---------------------------------------------------------
# 4. Post-Installation Sync
# ---------------------------------------------------------
echo "✅ Checking workspace health..."
pnpm recursive run typecheck || echo "⚠️  Warning: Initial typecheck failed."

echo "✨ Project initialization completed successfully. System is pristine."
