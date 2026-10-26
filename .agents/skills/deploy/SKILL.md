---
name: deploy
description: Send a finished line out, in one order — check and test once, squash main into its archive and checkpoint, push to deploy and watch CI, export xpute last — and what to do when the smoke fails. Use before squashing main, before any `git push origin main:deploy`, before exporting xpute, and while a deploy runs.
---

# Deploying

A line of work goes out in one order, each step on what the last left, so no
step is done twice. A squash rewrites main's hash, and the deploy and xpute's
export name that hash; whatever is done before the squash is done again after
it.

1. **The tree is final.** Every change the line carries is committed. A fix
   found later belongs to the next round, which runs the whole order again.
2. **Check and test, once**, on that tree (below). The steps after it change
   no file, so its result holds for each of them.
3. **Squash** main into one commit, the originals kept whole in the archive,
   the checkpoint on the squash.
4. **Deploy**, where one is asked: push the squash, watch CI.
5. **Export xpute**, last and once, from the squash.

Any step may be left out; none is taken out of its order.

## Check and test

CI checks nothing. `.github/workflows/deploy.yml` takes a build number,
rebuilds the servers on the VM, builds and publishes the client, smokes
plei.me and records the build. Every check and test runs here, before the
push, so a red check found after the push is a burnt build number. What the
bucket serves is [publish-bucket](../publish-bucket/SKILL.md), never a deploy.

From a tree whose HEAD is the commit to send and which holds no uncommitted
change — the checks read the working tree, and in a shared tree another
session's edits are in it:

```bash
make -j8 check && make -j8 test
```

What `check` runs, a task a tree, side by side:

- `check-ts` — the TypeScript workspace: format, lint, and the types of the
  page, the device's modules, the Worker and their scripts.
- `check-rust` — `cargo fmt`, and clippy over the module in both builds, the
  servers, the tests' code, the bakes and the tools, and both device targets.
- `check-rules` and `check-spec` — the tree's own rules, and the files made
  from `spec/` matching it.
- `check-build` — the client's bundle as a deploy builds it. The Worker is
  bundled in CI, first of all, before a number is taken or anything goes out.
- `check-xpute` — xpute's own workspace.

`test` is every workspace's tests: the client's, the web app's (Tint's compile
of the generated shaders, on this machine's GPU through Dawn), xpute's (its C++
guest built for wasm among them) and the crates'.

Neither runs the GPU hosts: `make -C xpute test-gpu` needs a GPU and Chrome
(`$CHROME`), and holds the native host on wgpu and the web host to
`xpute/spec/golden/gpu`. Run it by hand before sending a change to a GPU call —
`xpute/spec/gpu/`, xpute's `gpu` crates and packages, or how pxl-render
encodes.

While working, `./script/check-diff.py` runs the checks and tests of what the
working tree's changes reach. Before sending it is the whole `check` and
`test`, never the narrowed one.

## Squash

main is the last squash and what came after it. A squash folds those commits
into one on the last squash's parent, keeps them whole in the line's archive,
`archive/<date>-<line>`, and sets the line's checkpoint,
`checkpoint/<date>-<line>`, on it. A line is named for what it began with
(`gpu-cast`), not for its last change.

- **One line of originals.** The archive holds the originals as one line from
  the last squash's parent. Where a squash was made earlier in the same round,
  the commits after it stand on that squash; each is laid again on the
  original before it with `git commit-tree`, its tree, author, date and
  message unchanged (each squash's tree is its originals' last tree).
- **The squash**: `git commit-tree <tip>^{tree} -p <base>`. Its message is the
  line's subject, then the line below, then what the line did, by part:

  ```
  N commits from <base> to <tip>, kept whole in archive/<name>.
  ```
- **The archive**: one merge on what the archive held before the round,
  `-p <archive> -p <tip>`, named as below. One merge a round; a round redone
  replaces its merge, never stacks another.

  ```
  merge(archive): main from <base> to <tip>, N commits, squashed into <squash>
  ```
- **Push** the archive, then main with `--force-with-lease` on the main it
  replaces, then the checkpoint. main is rewritten: tell the sessions working
  in the checkout.

## Deploy

```bash
git push origin main:deploy        # --force once main has been rewritten
```

```bash
gh run watch "$(gh run list --workflow deploy.yml --limit 1 --json databaseId --jq '.[0].databaseId')"
gh run view <id> --log-failed       # a failed job's steps
```

The jobs run `number` → `deploy-server` → `deploy-web` → `smoke` → `record`. A
failed `deploy-server` keeps the client back; a failed `deploy-web` leaves the
servers new and the client old, which is the safe direction. A `deploy-web`
that fails at `wrangler deploy` says in its last line whether plei.me serves
the build anyway (wrangler can fail after the Worker went out), and `record`
logs it so.

### When the smoke fails

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
the Worker's or the bucket's. Fix forward with a new round; going back is
pushing the last good commit to deploy, which takes a number and redeploys
both halves from it.

## Export xpute

xpute's own repository (`github.com/canplane/xpute`) takes one commit a round,
made from the squash, since its message names the commit it came from.
Cloned under the session's scratch directory, never beside the checkout:

```bash
./xpute/script/export.py --to <clone>            # prints the stamp, pixelet@<squash>
git -C <clone> commit -F <message>                # no Claude trailer
./xpute/script/export.py --to <clone> --check    # the repository is the tree
```

The message is prose, as the repository's other commits are: a subject of
what xpute became, a paragraph a part, ending `Taken from pixelet@<squash>.`
It carries no Co-Authored-By or session line. Where the repository's last
commit is this line's own export, the new one replaces it — reset the clone
to the commit before it and push with `--force-with-lease` on the one it
replaces — so the line is one commit there too.
