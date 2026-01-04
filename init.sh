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
find . -name "*.tsbuildinfo" -type f -delete
find . -name ".cache" -type d -prune -exec rm -rf '{}' +

# ---------------------------------------------------------
# 2. Environment Reconstruction
# Reinstalls dependencies based on the lockfile.
# ---------------------------------------------------------
echo "🧹 Pruning pnpm store..."
pnpm store prune

echo "📦 Installing dependencies (pnpm install)..."
# --frozen-lockfile: Ensures that the pnpm-lock.yaml is not 
# updated during installation. Critical for consistent builds.
pnpm install --frozen-lockfile

# ---------------------------------------------------------
# 3. Post-Installation Sync
# Optional: Verifies the integrity of the workspace.
# ---------------------------------------------------------
echo "✅ Checking workspace health..."
pnpm recursive run typecheck || echo "⚠️  Warning: Initial typecheck failed."

echo "✨ Project initialization completed successfully. System is pristine."