---
name: deploy
description: Send a finished line out, in one order — check and test once, squash main with its originals on `original`, push to the deploy branch and watch CI, export xpute last — and what to do when the smoke fails. Use only when the user asks to deploy, squash main or export xpute. Not for a plain `git push origin main`.
---

# Deploying

Do these steps in this order, never reordered. A step the request does not
take is left out (a squash alone asks for no deploy); a check already passed on
the same tree is not run again; any other step is skipped only on the user's
word.

1. **Commit everything.** A fix found later starts a new round from step 1.
2. **Check and test once** on that tree (below). Change no file after it.
3. **Squash** main since the last squash, its originals on `original`.
4. **Deploy**, if one was asked for: push the squash and watch CI.
5. **Export xpute**, once, from the squash.

## Check and test

CI runs no checks. Uploading to the bucket is
[publish-bucket](../publish-bucket/SKILL.md).

Run from a tree whose HEAD is the commit to send, with nothing uncommitted:

```bash
make -j8 check && make -j8 test
```

If the change touches a GPU call (`xpute/spec/gpu/`, xpute's `gpu` crates and
packages, or how pxl-render encodes), also run `make -C xpute test-gpu` (a GPU
and Chrome, `$CHROME`).

## Squash

main is the last squash plus the commits since. Squash those commits on the
last squash, as the user says, keeping every original message.

- **The originals** go on `original` as one line behind a marker, an empty
  commit on its tip, and `original` moves only by fast-forward:

  ```
  chore: original/<date>-<line>

  What follows is the history that branch kept, from the state it began at (<previous date-line>).
  ```

  The date is the day of the squash; the line is the round's central change.
- **Push** `original`, then main with `--force-with-lease` against the main it
  replaces. Tell the sessions working in the checkout that main was rewritten.
- **A checkpoint** goes on main only when the user asks for one.

## Deploy

A push starts no run where deploy is already at the commit, or where only
markdown changed (deploy.yml `paths-ignore`); a dispatch
(`gh workflow run deploy.yml --ref deploy`) deploys deploy's tip again.

```bash
sha=$(git rev-parse main)
old=$(git ls-remote origin refs/heads/deploy | cut -f1)
last=$(gh run list --workflow deploy.yml --limit 1 --json databaseId --jq '.[0].databaseId // 0')
# The commit recorded, not main, which another session may move meanwhile;
# add --force once main has been rewritten.
git push origin "$sha:refs/heads/deploy" || exit 1
[ "$old" = "$sha" ] && { echo "deploy was already $sha: no run"; exit 1; }
# This push's run: of that commit and listed after the push (a redeploy of an
# old commit has earlier runs); never an earlier one, and not past 5 minutes.
for _ in $(seq 60); do
  id=$(gh run list --workflow deploy.yml --commit "$sha" --json databaseId \
    --jq "map(select(.databaseId > $last))[0].databaseId // empty") && [ -n "$id" ] && break
  id=; sleep 5
done
[ -n "$id" ] || { echo "no run for $sha (only markdown changed, or the listing failed)"; exit 1; }
gh run watch "$id" --exit-status || gh run view "$id" --log-failed   # the failed steps of a job
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

xpute's repository (`github.com/canplane/xpute`) gets one commit per round,
made from the squash. Clone it under the session's scratch directory:

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
the one it replaces.
