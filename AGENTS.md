# Agent Instructions

## Hold firmly

- Symmetry usually wins: of role and contract, not of form. Parts whose roles
  differ are not forced alike. A fix goes where the fact is owned and holds on
  every platform alike; a platform differs only in how it is reached. No patch is
  laid over a part to make one case pass.
- A structure that is not symmetric (one platform, case or observer handled
  apart from the others, or a step one path takes and another does not), or that
  carries ad hoc pieces (a branch, a flag, a retry or a workaround for one case):
  warn the user when you meet it, before working beside it, and fix it only on
  their word.
- `doc/` is notes, not rules: a minute or an experiment is what was said or
  measured then, and no renaming reaches it. Read what it holds on the subject
  before proposing, to know what was tried and why it was dropped.
- The user's machine is theirs: ask before taking its screen, a browser or a
  device, and never raise a window by force. Work goes to another session only
  on the user's word.
- After a change, check its state space: it is correct and no larger than
  before, or what it adds is justified by what it buys and why less would not
  do. Report the state removed, or the state added and what it buys.
- Before a commit, the review agent (`.claude/agents/review.md`) reads the
  change. Each finding is checked against the code or its source before it is
  acted on or passed on, and a fix is no larger than its finding.

## The system

- `pxl-core` and `xpute-core` link into one image, the core. xpute is the FFI
  between it and its runtime.
- Under `xpute/`: runtime, core and their contract, with no pixelet domain or
  name. It is to be published: don't propose removing a part of its `kit` or its
  C++ core because nothing here calls it.
- The UI computes no coordinate or tile. It reaches what is below it through
  `service/` alone, and the core through `service/core/` alone.
- What differs by platform is a file pair in `service/platform/` (`<x>.ts` the
  page's, `<x>.native.ts` the device's) or, for a piece of the UI, a file and
  its override beside it (`ts/client/src/ui/shared/component/README.md`).

## Working

- Read the skill the task needs (`.agents/skills/`).
- The code is agents' work, not the user's intent: judge it against the user's
  decisions. A comment, a structure or a pattern in it is no reason to keep it.
- Work in the main checkout and commit only your own paths; a worktree only for
  a tree that must be built apart.
- `out/` holds what a build makes, and no drafts. A temporary folder, an
  experiment's data too, goes under the checkout's `tmp/`; delete only the
  folder you made.
- Cargo builds into `<root>/rust/target/session/$CLAUDE_CODE_SESSION_ID`,
  written absolute.
- A command that uses a device's GPU or starts or stops an app on it, and does
  not hold the device itself (the shader probe, an install, xpute's GPU tests),
  runs under its device's module: `script/perf/mac.py`, `android.py` or `ios.py`
  `<command>`.
- A change is checked by `./script/diff-check.py [base]`: the checks and tests
  of what the change since `base` reaches (default HEAD and the uncommitted). `make check`
  and `make test` whole are a deploy's. A passed check is not run again over
  unchanged code, and a long run shows its progress.

## Instructions

In English. A rule lives in one place and any other links it; what every task
must hold firmly is in this file as well:

- this file: what every task needs, a line each;
- a skill: one kind of task with its procedure, its `description` saying where
  it ends;
- an agent file: a reviewer's scope and report, the rules staying in the skill;
- `CONTRIBUTING.md`: what a person runs; a README or doc comment: how a part
  works;
- memory: what the user said about working that none of these holds.

A change rewrites the rule that covers it, and a correction the rule it
corrects. A new file is for a new kind of task. A rule says what holds now: no
incident, date or reason, and nothing the code or a test already holds. A path
or a name appears only where the reader needs it to find the thing.
