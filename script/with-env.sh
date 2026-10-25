#!/bin/sh
# Runs its arguments with the local databases' addresses (infra/local/.env)
# and the servers' own numbers (rust/server/.env) set, where each exists.
for f in infra/local/.env rust/server/.env; do
  [ -f "$f" ] && set -a && . "./$f" && set +a
done
exec "$@"
