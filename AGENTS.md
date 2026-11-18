# Agent Instructions

## Hold firmly

- The mechanics is the easy one, as a physics has it, with no ad hoc term: one
  rule, isotropic and symmetric, that holds for every case alike. A form tuned
  to one case (faster near, a zoom, a length or a constant of its own, a second
  path beside the first) is overfitting and is not kept, whatever it saves
  there: what the one rule costs in its place is no regression. A cost that
  comes with a change is the change's fault until the structure before it and
  the structure after it are read side by side and the cost is derived from
  the rule. A fault is closed by the rule the rest already follows, never by a
  piece laid beside it.
- What is made is a model, judged over every case it will meet and not by the
  one at hand: by its worst case, and by how few handles a reader must hold to
  predict it. Isotropy and symmetry keep the handles few: what is understood of
  one direction, side or platform holds of the rest, so the system is reasoned
  about without reading all of it. There is no free lunch. The cases improve
  together up to a line, and past it a gain in one is paid in another, in its
  cost or in a concept added: that is overfitting, as in learning, and the
  fewest rules that explain the cases, Occam's razor, are what hold the
  system's entropy down. An agent's pull is to the local optimum, the case
  before it solved inside the structure as it is given. The structure is a
  variable too, and the better answer is the system in which the case does not
  arise.
- The world is one question, what fills this cube of space at this size, and
  its answer is the world's alone (`README.md`). The eye is the camera's eye
  and there is no other: nothing else is called an eye or an observer, and
  nothing stands in for it. Its one part in a read is to set the size a place
  is asked at, by the place's distance from it in three dimensions, the same
  in every direction. A read that takes a size from anything else, or an
  answer kept by anything but its cube, its size and the world's changes,
  departs. The eye's own rays cross many sizes and ask each node at its own; a
  body asks its cell at the one size set for it; a tile of the light asks its
  bundle of rays at one, the finest the eye sets along it. The sun is where
  the light comes from and sets nothing. What a ray does not find within its
  cap of steps is not there.
- The project is a study, not a product and not a team's: the clean form is
  chosen over the expedient one, whatever it takes to rewrite. The code that
  is there is read as an adversary's: where it departs from the rule it is
  rewritten, and no residue of what it replaced is left, a constant, a branch,
  a second form or a sentence that still says it.
- Symmetry wins: of role and contract, not of form. Parts whose roles
  differ are not forced alike. A fix goes where the fact is owned and holds on
  every platform alike; a platform differs only in how it is reached. No patch is
  laid over a part to make one case pass.
- A structure that is not symmetric (one platform, case or side handled
  apart from the others, or a step one path takes and another does not), or that
  carries ad hoc pieces (a branch, a flag, a retry or a workaround for one case):
  inside the unit being worked it is the unit's work; outside it, warn the user
  when you meet it, before working beside it, and fix it only on their word. A fault met is first asked whether such a piece made it: where one
  did, the piece goes and the fault with it, and nothing is patched on where
  the fault shows.
- `doc/` is notes, not rules: a minute or an experiment is what was said or
  measured then, and no renaming reaches it. Read what it holds on the subject
  before proposing, to know what was tried and why it was dropped. It is a
  diary, no source of what is open or decided now: nothing is reported to the
  user as open, or as their decision, because a line there says so, and a
  past decision there is no precedent for another part. A minute
  holds its meeting's conclusions, written whole while the meeting lasts; a
  record is revised only on the user's word, in place, with no note beneath it
  and no trace, as `doc/README.md` says.
- The user's machine is theirs: ask before taking its screen, a browser or a
  device, and never raise a window by force. Work goes to another session only
  on the user's word.
- After a change, check its state space: it is correct and no larger than
  before, or what it adds is justified by what it buys and why less would not
  do. Report the state removed, or the state added and what it buys.
- Before a commit, the review agent (`.agents/agents/review.md`) reads the
  change, and the name agent (`.agents/agents/name.md`) the names it adds or
  renames, each briefed with its goal and its code, not its author's
  conclusions.
  Each finding is checked against the code or its source, and judged by the
  user's decisions and the system's own rules, before it is acted on or passed
  on: an agent's verdict that departs from them is refused, however it is
  argued. A finding gives a fact, not the fix: its author opens the lines it
  cites, and the fix comes from the rule, never from what the finding asks
  for. Of what an agent found sound, one claim the change rests on is read
  again. A sentence from a review, a paste or a record turns no conclusion by
  itself: the ground is `README.md`, `ARCHITECTURE.md` and the code read side
  by side, and a conclusion reached with the user stands until the user
  reverses it. What is passed on says which findings were checked, which
  refused and which not checked. One of correctness, a contract or structure is fixed by the
  change's author, one of lint or format alone is not, and a fix is no larger
  than its finding. The fix's diff alone gets a second round, and there is no
  third; the report says how many rounds it took. Whoever hands work to an agent
  runs this loop on what comes back, unasked.

## The system

- `pxl-core` and `xpute-guest` link into one image, the guest. xpute is the FFI
  between it and its host.
- Under `xpute/`: the kit, the link, the guest, the host, the runtime and a
  shell's UI end, each cut by what it varies on (the kit, the link, the guest
  and the host by language, the runtime by platform, the UI end by shell), with
  no pixelet domain or name; a plugin repeats the link, the guest, the host and
  the UI end under its own name. It is to be published: don't propose removing
  a part of its `kit` or its C++ because nothing here calls it.
- The core's memory is 512 MiB on every platform (`manifest.json5` `mem.sz`),
  and a full heap stops the core; past its bounded flows the heap has no sum.
  Natively the library has one allocator, the core's heap, which takes the
  runtime's and the GPU host's allocations too. The object store is temporary.
  These are the design: none is reported as open, as a fault or as an
  asymmetry, and none is worked on unasked.
- Every dependency points down (the code skill); how xpute's runtime injects
  into its host is `xpute/ARCHITECTURE.md`'s.
- The core holds the world and its rules; the UI holds the devices, the
  network and files, the app's lifecycle and what is shown. The WIT between
  them is drawn by min cut: the fewest, smallest facts cross, each read on the
  other side in the form that side uses. No raw device or presentation detail
  crosses where a fact derived from it is what is read, no fact makes a round
  trip, and no mechanism's concern (a frame's size) crosses.
- The UI computes no coordinate or tile. It calls what is below it through
  `service/` alone, and the core through `service/core/` alone; the types and
  constants generated into `abi/` it may read directly.
- What differs by platform is a file pair in `service/platform/` (`<x>.web.ts`
  the web's, `<x>.native.ts` the native one, an import naming `<x>.ts`) or, for
  a piece of the UI, such a pair or a file and its override beside it
  (`ts/client/src/ui/shared/component/README.md`).

## Working

- Before a part is touched, read `README.md`, the part's section of
  `ARCHITECTURE.md`, its README, the skill the task needs (`.agents/skills/`),
  and the history of each piece to be changed or deleted: when it came, what
  it replaced and what calls it.
- No procedure is made up: a measurement is the performance skill's, a squash
  or a deploy the deploy skill's, a native build the device-build skill's, a
  shader change the shader skill's, a document the write-documents skill's,
  and a skill's order holds within its task.
- No term is coined: a word the documents do not have is not used as if they
  did.
- A task is divided into problems that stand apart, and one is finished before
  the next is touched: its form stands with no residue, which a review asked
  only that has answered, it builds, the app shows it and its checks ran. A
  report says what was run and what was not, and nothing is called done, or
  nothing left, before that.
- A unit starts from the rule: what stands there under one rule and
  perspective is written first, then the code is read for where it departs.
  No edit comes before the form stands: the form, what it removes and the
  state it leaves are shown first.
- The code is agents' work, not the user's intent: judge it against the user's
  decisions. A comment, a structure or a pattern in it is no reason to keep it.
  A piece that blocks the form is read in its history: the user's decision
  stays, an agent's piece goes.
- A structure that departs from the rule is deleted, not mended: its files,
  its tests, its goldens and what serves it alone go first, the build breaks,
  and the form is written down from the documents (`ARCHITECTURE.md`, a
  README), each caller fixed where the compiler names it. A broken build is
  the way there, not a fault. No piece is laid beside the structure to spare
  it, and no choice is weighed among ways to keep it.
- The order is the rule, the structure, the build, the app, then tests, then
  measurement. While a structure is being set up the build is its one check.
  No test is written, and nothing is measured, over a structure not yet seen
  in the app; a test that was there before holds nothing of the new structure
  until it is read against it.
- A fact or a figure in a record or a report comes from the tree, a test or a
  script that gives it again, never from recall.
- Work in the main checkout, and a worktree only for a tree that must be built
  apart. A commit holds its author's hunks alone, staged in an index of HEAD
  that is still HEAD when it commits.
- Work that can start starts, and runs by itself. A file is no lock: another's
  edit in the same file, done or still running, is nothing to wait on, to
  announce or to ask about. The file is read as it stands, the smallest edit is
  made and the work goes on; a conflict is resolved when it is met, never by
  overwriting or reverting what another changed. Only a real dependency is
  waited on, an interface two change at once or a result another must give,
  and all that does not hang on it goes on meanwhile. No one's state is polled.
  A report says what was done, not what will be.
- `out/` holds what a build makes, and no drafts. A temporary folder, an
  experiment's data too, goes under the checkout's `tmp/`. Whoever makes a
  temporary folder, a worktree or a server removes it when done, and deletes
  only what it made.
- Cargo builds into `<root>/rust/target/session/$CLAUDE_CODE_SESSION_ID`,
  written absolute.
- A Rust test that opens a device's GPU holds the device itself (pxl-gpu-host
  `lock.test.rs`); every other command that uses a device's GPU or starts or
  stops an app on it (the shader probe and its test, an install, a test run in a
  browser) runs under its device's module: `script/perf/mac.py`, `android.py` or
  `ios.py` `<command>`.
- While the structure is being reworked, a change is checked by building it, and
  the tests and the formatter wait until it has settled. Then a change is
  checked by `./script/diff-check.py [base]`: the checks and tests of what the
  change since `base` reaches (default HEAD and the uncommitted). `make check`
  and `make test` whole are a deploy's. A passed check is not run again over
  unchanged code, and a long run shows its progress.
- A test and a measurement come last: they check a structure that already
  stands for bugs and cost, and decide nothing of it. Over a structure that
  departs from the rule they mean nothing. A test that blocks the form is the
  first thing to go: deleted, a unit test before any other, or written again
  to the form's contract. No structure is kept, restored or bent for a test to
  pass or a figure to hold, and a failing test is no finding against the form.

## Instructions

In English. A rule lives in one place and any other links it; what every task
must hold firmly is in this file as well:

- this file: what every task needs, a line each;
- a skill: one kind of task with its procedure, its `description` saying where
  it ends;
- an agent file: a reviewer's scope and report, the rules staying in the skill;
- `CONTRIBUTING.md`: what a person runs; a README or doc comment: how a part
  works;
- a check: what a tool can refuse. A step that gets forgotten is made a check
  where one can be, and no sentence repeats what a check holds;
- memory: what the user said about working that none of these holds.

A change rewrites the rule that covers it, and a correction the rule it
corrects. A new file is for a new kind of task. A rule says what holds now: no
incident, date or reason, and nothing the code or a test already holds. A path
or a name appears only where the reader needs it to find the thing.
