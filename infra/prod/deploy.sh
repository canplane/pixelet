#!/usr/bin/env bash
# The server half of a deploy, run on the VM as root and read from the
# deploying machine's checkout rather than the VM's, which it resets:
#
#   ssh root@<vm> bash -eu -s -- <sha> < infra/prod/deploy.sh
#
# CI's deploy-server runs it so (.github/workflows/deploy.yml), and
# so does a deploy by hand (infra/README.md §3).
set -eu

sha="$1"
cd /opt/pixelet
# reset rather than pull: the VM is a deploy target and nothing is
# authored on it. infra/prod/.env is gitignored and survives this.
# The refspec spelled out: a clone made before deploy existed, or
# made single-branch, has no remote-tracking ref for it to update.
git fetch --prune origin +refs/heads/deploy:refs/remotes/origin/deploy
# The commit this run was started for, so the VM's half of a build
# is the half the runner published rather than whatever deploy has
# reached by the time the ssh lands. A rewritten history can leave
# that commit unreachable here, and a deploy that refused over it
# would leave production on the old code for something this project
# does on purpose — so it says so and takes the tip instead.
git fetch origin "$sha" 2>/dev/null || true
if git cat-file -e "${sha}^{commit}" 2>/dev/null; then
  git reset --hard "$sha"
else
  echo "::warning::$sha is unreachable on the VM; the servers are built from origin/deploy instead"
  git reset --hard origin/deploy
fi
cd infra/prod
# The disk's bounds are the tree's: a changed one is installed
# before the stack is recreated under it.
# Validated first: a config dockerd refuses would leave every
# container stopped.
if [ -f etc/daemon.json ] && ! cmp -s etc/daemon.json /etc/docker/daemon.json; then
  dockerd --validate --config-file etc/daemon.json
  install -m 644 etc/daemon.json /etc/docker/daemon.json
  systemctl restart docker
fi
if [ -f etc/journald.conf ] && ! cmp -s etc/journald.conf /etc/systemd/journald.conf.d/pixelet.conf; then
  install -D -m 644 etc/journald.conf /etc/systemd/journald.conf.d/pixelet.conf
  systemctl restart systemd-journald
fi
# --force-recreate because the Caddyfile is a bind mount, read once
# at start: an `up --build` alone leaves a stale relay behind a fresh
# server, which on 09-18 looked like /api/* answering 404 from the
# wrong process rather than like a failure.
docker compose up -d --build --force-recreate
docker image prune -f
docker compose ps
