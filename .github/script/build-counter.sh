#!/bin/sh
# Takes the next official build number off the counter and prints it. The
# counter is one object in the bucket, so no history rewrite renumbers it,
# and it is never reset. The take is read-add-write; the number job's
# concurrency group (.github/workflows/deploy.yml) keeps two runs apart.
#
#   ./.github/script/build-counter.sh

set -eu

KEY=pixelet-tiles/_meta/build-counter
# wrangler at the version ts/deno.lock holds for the edge's deploy.
wrangler() { deno run -A --node-modules-dir=auto npm:wrangler@4.123.0 "$@"; }
die() { echo "[build-counter] $1" >&2; exit 1; }

# A count that restarts is worse than no build; start one explicitly:
#   echo N | wrangler r2 object put pixelet-tiles/_meta/build-counter --remote --pipe --content-type text/plain
current=$(wrangler r2 object get "$KEY" --remote --pipe) || die "no counter at $KEY; no number was taken"
current=$(printf '%s' "$current" | tr -d '[:space:]')
case $current in '' | *[!0-9]*) die "the counter holds \"$(printf '%.40s' "$current")\", not a count; no number was taken" ;; esac

next=$((current + 1))
printf '%s\n' "$next" | wrangler r2 object put "$KEY" --remote --pipe --content-type text/plain >/dev/null \
  || die "could not write the counter back; #$next was not taken and the next build would reuse it"

echo "$next"
