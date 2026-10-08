#!/bin/sh
# Runs its arguments with ./.env's variables set, where there is one: the
# servers and the bakes read the databases' addresses from it.
[ -f .env ] && set -a && . ./.env && set +a
exec "$@"
