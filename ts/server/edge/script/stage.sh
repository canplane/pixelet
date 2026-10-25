#!/bin/sh
# Builds the page (ts/app/web `build`) and stages its bundle beside the
# Worker, at dist/, which wrangler.jsonc `assets` serves: a fresh copy every
# time, so what is checked or deployed is never a bundle left from before.
# A build number in the environment (PXL_BUILD_NUMBER) is the build's.
#
#   ./script/stage.sh   (from ts/server/edge)

set -eu

edge=$(cd "$(dirname "$0")/.." && pwd)
# The TypeScript workspace over the Worker: the nearest directory up whose
# deno.jsonc lists members, as Deno finds it.
ws=$edge
until grep -Eq '"workspace": *\[ *("|$)' "$ws/deno.jsonc" 2>/dev/null; do
  [ "$ws" != / ] || { echo "[stage] no TypeScript workspace over $edge" >&2; exit 1; }
  ws=$(dirname "$ws")
done
web="$ws/app/web"

deno task --cwd "$web" build
rm -rf "$edge/dist"
cp -R "$web/dist/" "$edge/dist"
echo "[stage] $web/dist -> $edge/dist"
