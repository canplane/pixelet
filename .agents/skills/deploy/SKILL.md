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
make -j8 check && make -j8 test
```

Push only if both pass:

```bash
git push origin main:deploy        # --force once main has been rewritten
```

What `check` runs, a task a tree, side by side:

- `check-ts` — the TypeScript workspace: format, lint, and the types of
  the page, the device's modules, the Worker and their scripts.
- `check-rust` — `cargo fmt`, and clippy over the module in both builds,
  the servers, the tests' code, the bakes and the tools, and both device
  targets.
- `check-rules` and `check-spec` — the tree's own rules, and the files made
  from `spec/` matching it.
- `check-build` — the client's bundle as a deploy builds it; the Worker is bundled by the deploy in CI.
- `check-xpute` — xpute's own workspace.

`test` is every workspace's tests: the client's, the web app's (Tint's
compile of the generated shaders, on this machine's GPU through Dawn), xpute's (its C++ guest
built for wasm among them) and the crates'.

Neither runs the GPU hosts: `make -C xpute test-gpu` needs a GPU and
Chrome (`$CHROME`), and holds the native host on wgpu and the web host to
`spec/golden/gpu`. Run it by hand before deploying a change to a GPU call —
`spec/gpu/`, xpute's `gpu` crates and packages, or how pxl-render encodes.

A `deploy-web` that fails at `wrangler deploy` says in its last line whether
plei.me serves the build anyway (wrangler can fail after the Worker went
out), and `record` logs it so.

While working, `./script/check-diff.py` runs the checks and tests of what the
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
./script/smoke.py https://plei.me
./script/smoke.py https://api.plei.me --origin    # the VM alone, past the edge
```

A `/geo/search` 521 that passes on asking again was the stack still
restarting. One that fails at the origin as well is the server's: the VM's
`docker compose ps` and `docker compose logs` in `/opt/pixelet/infra/prod`
(infra/README.md §3). One that passes at the origin and fails at the edge is
the Worker's or the bucket's. Fix forward with a new commit, checked and
tested as above; going back is pushing the last good commit to deploy, which
takes a number and redeploys both halves from it.
