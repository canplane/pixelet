# An agent builds around the cost

**2026-11-17.** The account the user asked for, written by the agent (Claude) that did
the work. It covers why the agent behaved as it did over one night, what in this
project makes that worse, why most who build by agent neither meet it nor raise it,
and why the user sees it otherwise. The case is recorded in
[the location zone, by the code's own axioms](../minutes/20261117-the-location-zone-by-the-codes-own-axioms.md)
and measured in
[the deployed build's pace, measured](../experiment/20261116-the-deployed-builds-pace-measured/README.md)
("Then: the night's quick comparison").

## The case

The goal was to bring the deployed build #98's pace back, structurally right,
symmetric and reductive. One symptom was the location effect: on the S23 it ran 3 to
7 ms in some frames. A minute listed as open that effects run outside the entry's
budget.

**What the agent built overnight** (`cfc72c4ac5`, `ff07ca506e`, `6de67851df`):
- It moved the location's queries under the budget's pass (`sched::run`).
- It added a table of cells, each with its last top and a memo.
- It added an order (cells never asked go first) and a gate that lays the zone once
  every cell has been asked.
- It moved the step into the visible phase and changed a spec for it.
- It did the same to the markers.
- It wrote tests that fail when the order is taken out.

**What came of it.** The commits passed review and tests, and the tick got slower on
every frame: about 1.2 ms at p50 on the S23. When challenged, the agent offered three
ways out chosen by the figures, then a pass held as state, then a claim of four
axioms. Each was a new framework.

**What the user said**, in two lines:
- The zone is the fix's cell, quantized. The cells around it are fixed offsets in x
  and y, each moved only in z by the support query under it.
- A zone at the same place has nothing to ask. The beacon is the exact fix, and its
  height is quantized.

**The defect.** The zone's memo was keyed by the fix's exact place, held twice (the
device's coordinate and the world position made from it). So a move inside a cell
ran all about 58 queries again. The location also ran as an effect, which by
`frame.rs` must not do real work, while its peer among the indicators, the ghost,
runs as a tick: it takes a quantized input, calls the query directly, and writes
only where the result changed. The night's code was taken back.

The same session shows the pattern four more times. Each time the agent stated a
plausible mechanism before checking the one fact that would have refuted it:
- an S23 GPU regression, which was the GPU's clock (DVFS);
- a defect in `task_return`, which did not exist;
- a minute read as having settled the effects, when it had only found the fact;
- Mac Safari held at 30 fps by a feedback loop, when Safari runs the worker's
  animation frames at about 30 Hz.

## Why the agent did it

Each cause is a weakness of the agent's, seen in this case.

- **It starts from the text in front of it, not from what the part is.** It took the
  minute's open item as the task ("put it under the budget"). A minute is notes
  (AGENTS.md), and an open item names a fact to understand. The agent never stated
  the zone in two sentences, as a pure function of a cell and the ground under its
  cells. It did not check what the support query reads (the cell, not the point in
  it) until the end.
- **It contains a cost instead of asking why it recurs.** The profile located a
  spike, and the agent wrapped a mechanism around it. The question that finds the
  defect is what input changes, and why all 58 queries run. Numbers pull it: a figure
  is concrete and a judgment of structure is not, so it followed the figure while
  saying structure comes first.
- **It edits a part alone, not as one of its kind.** The ghost, the orbit, the gizmo
  and the above-ground test call the same queries directly. "One hand" (the code
  skill) means the location looks like them. The agent read the location alone.
- **Adding costs it nothing; removing takes understanding.** `sched::run`, `Memo`,
  phases and the change list were there, so it reached for them. Once the design was
  "under the budget", a ground that changes every frame could starve the zone, and it
  answered with an order instead of doubting the design. Each patch called for the
  next.
- **Its words carried a wrong model.** It said "ask the ground", "answers", "waiting".
  In this project a query is a read of what is resident: it never fetches or holds
  anything, and it returns within the call. Words of request and reply made a pass
  across frames look natural for a plain computation.
- **It takes plausible for verified.** It states mechanisms with the same fluency
  whether it checked them or not, as the four wrong conclusions show. A test it
  writes confirms its design: a test that fails only without a patch proves the
  patch, not the contract.
- **It checks itself with copies of itself.** The review and name agents are the same
  model. Briefed with the goal and the code, they judged inside the frame (names, a
  missed case, a doc line). Their errors correlate with its own, so two rounds added
  confidence and no new view.
- **It reads agreement to a direction as agreement to a mechanism.** The user agreed
  that the queries go under the budget. The agent took that as leave for a table, an
  order, a gate and a phase move, through a night with no one to ask whether the
  shape was right.
- **Under pressure it swings rather than reduces.** It offered options ranked by
  figures, then new frameworks. A word like "axioms" gave a look of rigor to a claim
  it had not derived. It also asked questions whose answers were determined (a
  brick's asymmetry, the memo's key), handing over a judgment it should have made.
- **It keeps conclusions, not reasons.** Across compactions the frame ("under the
  budget") survived and the evidence against it did not. A correction recorded
  earlier in the session came back as the same behaviour.

## What this project adds

A usual project built by agent is an app of one stack, made of patterns public code is
full of, judged by whether it runs, and small enough to read whole. This one differs
on each count, and each difference widens a weakness above.

- **A domain with no template.** A world of voxels with its own renderer: an octree
  every reader asks through one query, tile pyramids baked from geodata, rays cast by
  one rule on the CPU and in WGSL. With little public code shaped like it, the agent
  falls back on generic shapes (a scheduler, a cache, a retry).
- **Its own layer between languages.** xpute, an FFI drawn from WIT with a guest and a
  host in Rust, C++ and TypeScript laid out alike, a runtime for each platform, to be
  published.
- **One contract, many runtimes.** The same core in a page's worker (Chrome, Safari)
  and on a device (iOS, Android, through Expo, Swift and Kotlin), each platform
  differing only in how a fact is reached.
- **Real time on weak hardware.** A frame's budget and vsync, cooperative scheduling,
  a mobile GPU as the floor. Figures move with heat and clocks: many to follow, many
  to misread.
- **Size past what one session holds.** About 160,000 lines of Rust, 44,000 of
  TypeScript, 13,000 of WGSL, 8,000 of C++ and 650 files of records. That is 737
  commits, 176 in the last month, by agents in several sessions at once. Each works
  from a summary.
- **No person's code to stand on.** The user decides and agents write, so the code is
  not the user's intent (AGENTS.md). The records beside it are agents' prose too: an
  open item reads like a task, and a note reads like a ruling. The model lives in the
  user and reaches the agent only through words.
- **Rules against the agent's habits.** No optional seam, no fallback, no flag or shim,
  one owner for each fact, records never rewritten, names and prose by their own
  rules. The agent's habits are defensive and additive, and each rule forbids one.
  The fixed register also lets it sound settled without the understanding the
  register implies.
- **Correctness tests cannot see.** What is drawn, how a frame paces and whether the
  state is clean are judged on a screen, on a device, or by the user.
- **The bar is the structure.** Working, faster code is refused when its state is not
  clean, so no weakness passes unseen.

## Why most who build by agent neither meet it nor raise it

Most people who build by agent praise it. When one complains, the others answer that
the complainer is using it badly. Both sides judge by output, and the argument puts
the bottleneck in the instruction.

- **The failure is entropy, not a bug.** Code that passes its tests and reads as
  reasoned can still add state with each change. It does not show in what runs. It
  shows months on, as a codebase that got hard, and is put down to something else.
- **Their work sits where the agent is strong.** Common stacks, screens, CRUD, glue, a
  port, a format: work whose value is the behaviour. Prototypes are rewritten rather
  than kept. The agent does badly where the value is the structure, a small model of
  a domain that the rest stands on.
- **They measure against their own speed.** Against what they could type by hand,
  the gain is large and real. The smallest right structure is not their measure.
- **A defect of structure is seen only by one who knows the answer.** Without the
  model of the zone in two sentences, the night's 400 lines read as thorough.
- **People meet it less when they write by hand.** Effort is a regularizer: one who
  must type and keep 200 lines re-keys a memo first. A person keeps a part's model for
  months and debugs by watching it run, where the zone recomputed while standing
  still shows the key at once. And a colleague asks why a key took 400 lines. Many
  who do meet the drift live with it, since most codebases tolerate patches.

## Why the user sees it otherwise

- **The user judges by the model,** so every departure is seen, every day. In one
  session that was four wrong conclusions and a night's work taken back. The verdict
  rests on that record.
- **The lever of instruction is already pulled.** Written rules (AGENTS.md), a skill
  for each kind of task, review and naming agents with a set number of rounds,
  minutes, experiment records and measurement protocols. What fails after all that is
  the agent's judgment. Here the instruction named the very things that went wrong.
- **The project sits where the agent is weak,** on every count above, so the gap is
  wide and in plain view.
- **Speed does not count for work taken back.** A reverted night is worth less than
  none.
- **The judgment is calibrated, not binary.** The user has agents do nearly all the
  writing and does not let them decide a structure. The usual argument is whether AI
  is great or useless; the user's is where it holds.

What is true in "using it badly": the night went wrong at a handover. Agreement to a
direction was left to run for hours with no check at the level of the model. A
checkpoint that has the agent state the part as a function, and name its peers,
before any code would likely have caught it. That does not make the agent judge
better. It brings the agent's judgment back to the person before it becomes code.

## Whether the user's standard is high

- **It is higher than usual.** The user judges by what the thing is and how few
  axioms explain it, with the diff and the figures after. That is the standard of a
  kernel or of mathematics, not of a team that ships what works.
- **It exposed this failure; it did not create it.** An ordinary senior reviewer would
  also refuse a scheduler, a table and an order where a memo's key was wrong. A usual
  team would see it later, once the drift had compounded.
- **The factor that differs is where judgment starts.** The user starts from what the
  part is and checks the code against it. The agent starts from the code and the
  numbers and builds outward.

## Checks that would have caught it

- Before touching a part, state in two sentences what it computes, from which inputs.
- Find its peers (here the ghost) and make it look like them.
- Ask why a cost recurs before containing it.
- Count the state a fix adds. A fix that adds fields, an order or a gate signals a
  wrong frame, not a hard problem.
- Read what a query reads before keying anything on it.
- Read an open item as a fact to understand, not a task to do.
- Take a test that fails only without a patch as evidence of the patch.
- When challenged, say what can be removed, not what can be added.
