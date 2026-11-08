#!/bin/sh
# Runs its arguments with the local databases' addresses (infra/local/.env)
# and the servers' own numbers (rust/server/.env) set, where each exists. The
# files are the checkout's, found from this script, so it runs from anywhere:
# the root's make runs cargo under it from rust/.
root=$(cd "$(dirname "$0")/.." && pwd)
for f in infra/local/.env rust/server/.env; do
  [ -f "$root/$f" ] && set -a && . "$root/$f" && set +a
done
exec "$@"
