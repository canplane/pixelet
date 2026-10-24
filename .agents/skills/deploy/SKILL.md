---
name: deploy
description: Check, test and push a commit to deploy, then watch CI publish the client and rebuild the servers, and what to do when the smoke fails. Use before any `git push origin main:deploy`, and while a deploy runs.
---

# Deploying

CI checks nothing. `.github/workflows/deploy.yml` takes a build number,
rebuilds the servers on the VM, builds and publishes the client, smokes
plei.me and records the build. Every check and test runs here, before the
push, so a red check found after the push is a burnt build number. What the
bucket serves is [publish-bucket](../publish-bucket/SKILL.md), never a deploy.

## Before the push

From a tree whose HEAD is the commit to deploy and which holds no
uncommitted change in what ships — the checks read the working tree, and in
a shared tree another session's edits are in it:

```bash
deno task check && deno task test
```

Push only if both pass:

```bash
git push origin main:deploy        # --force once main has been rewritten
```

What `check` runs, each a task of its own:

- `check:style` — `deno fmt` and `cargo fmt`, check:rules, `deno lint`.
- `check:ship` — the web entry and the Worker type-check, the scripts
  type-check, the shipped crates pass clippy, the files made from `spec/`
  match, and the bundle and the Worker build as a deploy builds them.
- `check:native` — the device's modules and both device targets.
- `check:host` — clippy over the tests' code, the bakes and the tools.
- `check:xpute` — xpute's own workspace.

`test` is every workspace's tests: the client's, xpute's (its C++ guest
built for wasm among them) and the crates'.

Neither runs the GPU hosts: `deno task --cwd xpute test:gpu` needs a GPU and
Chrome (`$CHROME`), and holds the native host on wgpu and the web host to
`spec/golden/gpu`. Run it by hand before deploying a change to a GPU call —
`spec/gpu/`, xpute's `gpu` crates and packages, or how pxl-render encodes.

A `deploy-web` that fails at `wrangler deploy` says in its last line whether
plei.me serves the build anyway (wrangler can fail after the Worker went
out), and `record` logs it so.

While working, `deno task check:diff` runs the checks and tests of what the
working tree's changes reach. Before a deploy it is the whole `check` and
`test`, never the narrowed one.

## Watching it

```bash
gh run watch "$(gh run list --workflow deploy.yml --limit 1 --json databaseId --jq '.[0].databaseId')"
gh run view <id> --log-failed       # a failed job's steps
```

The jobs run `number` → `deploy-server` → `deploy-web` → `smoke` → `record`.
A failed `deploy-server` keeps the client back; a failed `deploy-web` leaves
the servers new and the client old, which is the safe direction.

## When the smoke fails

The client and the servers are already out, and `record` logs the build with
what the smoke found. Ask again from here:

```bash
deno task smoke https://plei.me
deno task smoke https://api.plei.me --origin    # the VM alone, past the edge
```

A `/geo/search` 521 that passes on asking again was the stack still
restarting. One that fails at the origin as well is the server's: the VM's
`docker compose ps` and `docker compose logs` in `/opt/pixelet/infra/prod`
(infra/README.md §3). One that passes at the origin and fails at the edge is
the Worker's or the bucket's. Fix forward with a new commit, checked and
tested as above; going back is pushing the last good commit to deploy, which
takes a number and redeploys both halves from it.
