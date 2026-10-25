# What the pipeline could not say about itself

**2026-09-19** — the gate and the two deploys went up on 09-18
([20260918-the-gate-that-deploys.md](./20260918-the-gate-that-deploys.md)) and
this was the first day they were leaned on. Eight things were wrong. None of
them was visible from outside a log, several had been wrong since the first
green build, and three were the same mistake: **the pipeline reporting something
it had not checked.**

It started from one question with no answer in the repository — _what went out
in b18?_

## The counter names no commit

`script/build-counter.ts` keeps the count as one object in the bucket, outside
git on purpose: no squash, rebase or fresh clone can renumber it. The cost was
that `b18` existed with nothing to look it up by. Answering it meant opening the
workflow's run history and reading a sha out of a log.

A tag was the obvious fix and the wrong one. Main is rewritten here as a matter
of course, so a tag, a stored sha and `BUILD_ID`'s own `r<count>-<hash>` all
name something that may not exist by the time anyone looks. What survives a
rewrite is text written before it.

So `record` appends a line per published build to `_meta/build-log.jsonl` — its
number, what it was built from, which halves it reached, and **the subjects of
the commits it carried**, read at deploy time while the history is still that
history. `./script/build-log.ts` reads it back. The sha is kept beside the
subjects because it resolves right up until it does not, and the next record
falls back to its own tip when it stops.

The commit subjects in this repository are already written as a summary would
want them, so nothing has to be composed. That is the whole of what a changelog
was wanted for here, and why there is no `CHANGELOG.md`: its reader is someone
deciding whether to move between versions, and continuous deployment has no such
reader. The trigger that would make one earn its place is `xpute` shipping as a
package, and then it belongs inside the package with its semver.

## A run built the branch, not its own commit

`actions/checkout` left to itself takes the branch tip at fetch time. That is
the commit the run was started for only when nothing has landed since — and here
something usually has. Every checkout now pins `ref: github.sha`, and the VM
resets to that commit rather than to whatever `origin/main` reached by the time
the ssh connected.

The VM keeps a fallback: a rewrite can leave that commit unreachable there, and
refusing would leave production on old code over a habit the project keeps
deliberately, so it warns into the run and takes the tip.

## `-dirty` meant nothing, on every build

`build_id()` marks a build `-dirty` when `git status --porcelain` answers with
anything — the compiled files are not the files that commit holds, so the id is
a lie and the bytes cannot be got back. b18 carried the mark although CI checks
out clean.

Ruled out by checking: the wasm and the vite build both leave the tree
untouched, `kernel.wasm`, the generated `symbol.ts`, `.build_number`,
`crates/target`, `dist` and `node_modules` are ignored, binaryen unpacks outside
the tree, and both locks were in sync at that commit. What was left was a tree
only the runner could see, so `deploy.ts` now asks the same question at the same
point and **prints the paths**.

It has not reappeared since the checkout was pinned. The cause is unproven and
the diagnostic is what stands guard: a mark that says a tree was unclean and not
which part of it is what this ends.

## The gate was flaky, and the test was wrong

`tick.test.ts` asserted what a budget gives away and what it holds back, and
answered in wall time by spinning. A budget can only stop **between** steps — a
step cannot be preempted — so "content always gets a tenth" is a promise the
scheduler never made. On a quiet machine the steps were small enough that it
held; on a loaded runner one was preempted and the phase after it saw 0.18ms of
the 0.6.

The scheduler already reads time through `clock.ts`, so the tests now move it by
hand, installed per test and put back after because `io.ts` reads the same
clock. They take no wall time at all now, which is the shape of the proof: a
timing test that runs in 0ms is not timing anything it does not control. The
real thing — how far a step actually overshoots on a real device — is the
ledger's to measure, not a unit test's.

## A script that cost a toolchain

`test:asset` ran a single-file cargo script on every push: 34s of a 3-minute
gate re-resolving and rebuilding 49 packages, because a script keeps its lock
where a fresh runner cannot find it. It resolved `pxl-spec`'s transitive
versions apart from `crates/Cargo.lock` while doing it, and `-Zscript` is
nightly-only, so CI installed a second toolchain for that alone.

The first attempt made it a member of the `crates/` workspace, and that broke
the servers: both Dockerfiles copy `crates` alone, so a member reached through
`..` pointed at a path their build context does not contain. It is its own
workspace now, with a committed lock beside it — the lock and the stable
toolchain without asking `crates/` to know about a tree outside it.

Its sibling `download_texture_pack.rs` stays a script. Nothing in CI runs it,
and its HTTP client would be compiled by every gate for nothing.

## The two deploys raced, and the client won

Nobody's open tab reloads on command, so a deploy is never simultaneous and the
skew lasts as long as a session. What makes that survivable is not speed but
**order**: a new client against an old server is the direction that breaks, and
an old client against a new one is not, so long as the server still answers the
old shape for a release. That is expand–contract, and the project already does
it for tiles — versioned prefixes, uploaded before the client that asks for
them.

The b15 deploy followed that order by hand
([20260918-the-server-in-rust-and-the-line-already-drawn.md](../20260918-the-server-in-rust-and-the-line-already-drawn.md)
says "server then client"); the workflow did not, since both deploys needed the
gate alone. b20 showed what that is worth — the client published, the servers
failed to rebuild, and the live site ran new code against old servers until a
later run fixed it. It held only because the surface between them is two routes
and neither had changed.

`deploy-web` now needs `deploy-server`. It earned itself the same day: the next
server failure skipped the client rather than shipping it alone.

## Two builds, one cargo registry

The VM's rebuild then failed unpacking `libm v0.2.11` —
`.cargo-ok: File
exists`. The two server images build in parallel and mount the
same registry cache, while cargo's package lock is at
`$CARGO_HOME/.package-cache`, outside the mount: each container holds its own
and neither excludes the other. `libm` arriving in the workspace that day is
what first made both want it at once. `sharing=locked` gives BuildKit the
exclusion cargo could not. The target mount needs no flag; cargo's build lock
lives inside it.

## The log believed a job that failed

Both deploys published their outputs from the scope step, which says only
whether to _try_. A failed job still carries its earlier steps' outputs, so
`record` wrote b23 as having reached the servers on a run where the server
deploy failed and the client never ran. The outputs now come from a step after
the deploy, which a failure above it skips, and the false line is out of the
log.

## What kept coming back

Three of the eight were the same shape: **a record that can be wrong is worse
than no record.** The build id said `-dirty` without saying why; the run
reported a commit it had not built; the log reported a half that had not landed.
Each was a line that looked like information and was not, and each was fixed by
making the thing that reports also be the thing that checked.

The fourth, the flaky test, is the same idea turned inward — it asserted a
guarantee the design does not make, and passed for months because the machine
was quiet.

## Status

Through. b24 (`81dd5c6f`) is the first build where both halves are the same
commit, recorded as such, with a clean id. b19–b23 are the day's cost: two
counted and never published, one published to the edge alone, one recorded
falsely and removed, one gate failure.

Left open: the `-dirty` cause is unproven, only absent. b20 has no line in the
log — it was live for some forty minutes and `record` skipped it, which is the
hole that closed the same evening.
