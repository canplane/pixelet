---
name: deploy
description: Send a finished line out, in one order — check and test once, squash main into its archive and checkpoint, push to deploy and watch CI, export xpute last — and what to do when the smoke fails. Use before squashing main, before any `git push origin main:deploy`, before exporting xpute, and while a deploy runs.
---

# Deploying

Do these steps in this order. You may skip a step, but never reorder them. A
squash changes main's hash, and the deploy and the xpute export record that
hash, so anything done before the squash has to be done again after it.

1. **Commit everything.** A fix found later goes into the next round, which
   starts again from step 1.
2. **Check and test once** on that tree (below). Nothing after this step
   changes a file, so the result still holds.
3. **Squash** main into one commit, keeping the originals in the archive and
   putting the checkpoint on the squash.
4. **Deploy**, if one was asked for: push the squash and watch CI.
5. **Export xpute**, once, from the squash.

## Check and test

CI runs no checks; it only builds, deploys, smokes and records. A check that
fails after the push wastes a build number, so run everything first. Uploading
to the bucket is [publish-bucket](../publish-bucket/SKILL.md), not a deploy.

Run from a tree whose HEAD is the commit to send, with nothing uncommitted (the
checks read the working tree, and another session's edits would be in it):

```bash
make -j8 check && make -j8 test
```

Always run the full `check` and `test`, never a subset. Neither covers the GPU
hosts: if the change touches a GPU call (`xpute/spec/gpu/`, xpute's `gpu`
crates and packages, or how pxl-render encodes), also run
`make -C xpute test-gpu`, which needs a GPU and Chrome (`$CHROME`).

## Squash

main is the last squash plus the commits since. Fold those commits into one
commit on the last squash's parent, keep the originals in
`archive/<date>-<line>`, and put `checkpoint/<date>-<line>` on the squash. Name
the line after what it started with (`gpu-cast`), not its last change.

- **Originals as one line.** The archive holds the originals as a single line
  from the last squash's parent. If this round already squashed once, the
  commits after that squash sit on it: re-create each on the original before it
  with `git commit-tree`, keeping its tree, author, date and message.
- **The squash:** `git commit-tree <tip>^{tree} -p <base>`. Its message is the
  line's subject, then this line, then what the line did, part by part:

  ```
  N commits from <base> to <tip>, kept whole in archive/<name>.
  ```
- **The archive:** one merge per round on top of the previous archive,
  `-p <archive> -p <tip>`. Redoing a round replaces its merge; don't stack a
  second one.

  ```
  merge(archive): main from <base> to <tip>, N commits, squashed into <squash>
  ```
- **Push** the archive, then main with `--force-with-lease` against the main it
  replaces, then the checkpoint. Tell the sessions working in the checkout that
  main was rewritten.

## Deploy

```bash
git push origin main:deploy        # add --force once main has been rewritten
gh run watch "$(gh run list --workflow deploy.yml --limit 1 --json databaseId --jq '.[0].databaseId')"
gh run view <id> --log-failed       # the failed steps of a job
```

The jobs run `number` → `deploy-server` → `deploy-web` → `smoke` → `record`.

- `deploy-server` fails: the client is not deployed either.
- `deploy-web` fails: the servers are new and the client is old, which is safe.
  If it fails at `wrangler deploy`, its last line says whether plei.me serves
  the build anyway, and `record` logs that.

### When the smoke fails

The client and servers are already out, and `record` has logged what the smoke
found. Run it again:

```bash
./script/smoke.py https://plei.me
./script/smoke.py https://api.plei.me --origin    # the VM only, bypassing the edge
```

- A `/geo/search` 521 that passes on the second try: the stack was still
  restarting.
- Fails at the origin too: the server. Check `docker compose ps` and
  `docker compose logs` in `/opt/pixelet/infra/prod` on the VM
  (infra/README.md §3).
- Passes at the origin, fails at the edge: the Worker or the bucket.

Fix forward with a new round. To go back, push the last good commit to deploy;
that takes a new build number and redeploys both halves.

## Export xpute

xpute's repository (`github.com/canplane/xpute`) gets one commit per round, made
from the squash, because its message names the source commit. Clone it under
the session's scratch directory, never next to the checkout:

```bash
./xpute/script/export.py --to <clone>            # prints the stamp, pixelet@<squash>
git -C <clone> commit -F <message>                # no Claude trailer
./xpute/script/export.py --to <clone> --check    # the repository matches the tree
```

Write the message as prose, like the repository's other commits: a subject
saying what xpute became, a paragraph per part, ending
`Taken from pixelet@<squash>.` No Co-Authored-By or session line. If the
repository's last commit is this line's own earlier export, replace it: reset
the clone to the commit before it and push with `--force-with-lease` against
the one it replaces, so the line stays one commit there too.
