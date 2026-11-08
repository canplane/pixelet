# Agent Instructions

- `pxl-core` and `xpute-core`, its library OS, link into one image, a unikernel.
  The runtime is its VMM, the UI the client outside the VM, the rings the device
  between them.
- Under `xpute/`: runtime, core and their contract, with no pixelet domain or
  name. It is to be published: don't propose removing a part of its `kit` or its
  C++ core because nothing here calls it.
- xpute's ABI takes the majors under `PROGRAM_MAJOR`, the app's from it. No
  ports.
- What the app's numbers mean is the client's services (`ts/client/README.md`),
  never `@xpute/runtime` or `xpute-runtime`.
- The UI computes no coordinate or tile. It reaches what is below it through
  `service/` alone, the core through `service/core/` alone, and that through the
  generated stubs alone.
- What differs by platform is a file pair in `service/platform/` (`<x>.ts` the
  page's, `<x>.native.ts` the device's) or, for a piece of the UI, a file and
  its override beside it (`ts/client/src/ui/component/README.md`).

## Working

- Read the skill the task needs (`.agents/skills/`).
- The code is agents' work, not the user's intent: judge it against the user's
  decisions. A comment, a structure or a pattern in it is no reason to keep it.
- Work in the main checkout and commit only your own paths; a worktree only for
  a tree that must be built apart.
- `out/` holds what a build makes, and no drafts. A temporary folder, an
  experiment's data too, goes under `cache/` at the checkout's root.
- Cargo builds into `<root>/rust/target/session/<id>`, written absolute, `<id>`
  the session's (`$CLAUDE_CODE_SESSION_ID`).

## Instructions

In English. A rule lives in one place, and any other links it:

- this file: what every task needs, a line each;
- a skill: one kind of task with its own procedure, its `description` saying
  where it ends;
- an agent file: a reviewer's scope and report, the rules staying in the skill;
- `CONTRIBUTING.md`: what a person runs; a README or doc comment: how a part
  works;
- memory: what the user said about working that none of these holds.

A change rewrites the rule that covers it, and a correction the rule it
corrects. A new file is for a new kind of task. A rule says what holds now: no
incident, date or reason, and nothing the code or a test already holds. A path
or a name appears only where the reader needs it to find the thing.
