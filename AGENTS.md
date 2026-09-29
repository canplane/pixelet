# Agent Instructions

Instructions for AI coding agents working in the pixelet repository. Tool-agnostic
on purpose — Claude Code, Copilot, and Cursor all read this file (directly or
through a symlink), so nothing here should assume a particular assistant. It is
read in every session, so it holds what an agent would get wrong without it;
what is local lives beside the code it is about.

What the system is: [README.md](./README.md) and
[ARCHITECTURE.md](./ARCHITECTURE.md) for the application,
[README.xpute.md](./README.xpute.md) and
[ARCHITECTURE.xpute.md](./ARCHITECTURE.xpute.md) for the system under it.
**The two `.xpute` documents must never name pixelet**: they are the xpute
repository's, and the export drops the infix and ships them unchanged. How all
four are written and reviewed:
[.agents/skills/write-documents](./.agents/skills/write-documents/SKILL.md) —
read it before reworking one, not after. Why past decisions went the way they
did: [issue/](./issue/). What is queued:
[issue/BACKLOG.md](./issue/BACKLOG.md). Procedures — a bake, a publish — are
the skills in [.agents/skills/](./.agents/skills/).

---

## Environment

**This is a Deno workspace, not an npm project**, driven through `deno task`
from the root; README.md lists the tasks. The one `package.json` is the device
app's (`apps/native`), because Expo and Metro find their packages the way npm
lays them out.

**`deno task check` is the gate.** Do not report work as done without it
passing. main is not built by CI; publishing is a push to `deploy`
([infra/README.md](./infra/README.md) §5).

The geo database (`pixelet-db-geo`) holds a multi-gigabyte PostGIS dataset.
**Never `down -v` it casually** — the volume is the data. Ingestion and
operations are [infra/README.md](./infra/README.md); follow it rather than
improvising SQL.

## Boundaries

The tree is grouped by where a thing runs (`crates/xpute`, `crates/pxl/{spec,web,server}`,
`packages/xpute`, `apps/web`, `apps/native`); README.md has the map. Three lines are easy to
erode by accident:

- **xpute holds no domain.** No coordinates, terrain, tiles or Overture in
  `xpute-core` or `@xpute/core`; those belong to `pxl-spec`. The view
  (`pxl-render`) knows the model and nothing of the kernel (`pxl-kernel`).
- **`core` is taken as it is; `runtime` asks you to run its way.** The kit
  works in any program; the runtime means a turn on a doorbell, a quota from
  the frame, I/O asked of the host. `runtime` depends on `core`, never the
  reverse.
  `core` is symmetric across the two languages and the golden records hold
  them to each other; `runtime` is two ends of one arrangement, so a module
  missing on one side is not a gap to fill — only the format must agree.
- **`pxl-spec` is the one model and has no TypeScript twin.** The shell reads
  a page through the kernel, never by decoding one. What the shell holds — the
  cell catalog, the asset tables, the ABI enums — is generated into
  `apps/web/src/` from `spec/`, beside the Rust generated from the same JSON.

**What shares `pxl-spec` is Rust, and what does not is free.** The kernel, the
servers and the bakes compile the model, so they are crates; an ingest that
moves a provider's columns into Postgres shares nothing, so it is a script;
`apps/web` is the shell — the device app runs the same one, and adds only
what a device does behind `@platform/` — and stays TypeScript.

**A tool that makes a file is Rust; one that talks to a service is
TypeScript.** What writes or reads a format the Rust side owns, or processes
a binary — the asset ingest, the stars, the Moon, the brand, the code
generated from `spec/` — is a binary of `pxl-tool` (`crates/pxl/tool`)
wherever it fetches its input from, and links no crate it writes into. The
shell's own build is
`apps/web/script/`; the tree's checks, the export and what reaches a
deployment or a bucket are `script/`. A script under `infra/` makes a
database's state, not a file, and runs as infra's README says.

**On an iPhone, a control a touch lands on is UIKit's.** SwiftUI
(`@expo/ui`) draws the sheet and nothing inside React Native's views that
takes a touch: a SwiftUI switch there missed quick taps that UIKit's took.
Which technology draws each `@platform/` piece is
[apps/native/src/platform/README.md](./apps/native/src/platform/README.md).

## One spatial model

Everything placed on the map sits in one hierarchy, the **pyramid**: level k
is z(16 − k), cells 2^k units wide, from z16 — terrain's atom, a 1u cell —
to z6, a page 512 cells a side at every level and named by the Web Mercator
tile it stands on. Say z16, not "level 0": no level is special.
The rules below keep it one thing. They come from finding it several.

- **The lattice has one owner.** A size is a `Scale` against the unit,
  stepped from two roots — the tiers (`pxl-spec space/indexer.rs`) and the
  raster's cell (`bake/constant.rs` `CELL`), which is a resolution and
  derives from neither — and parent and children in any number of axes are
  `space/hierarchy.rs`'s. Level and span, a cell's base and center, its
  place in its page — which is the index into every lane the page has —
  the 4 × 4 neighborhood a fold reads, and a page's name are
  `space/pyramid.rs`'s; a level is a count from a lattice's cell, and the
  lattice is what turns it into a size. A layer that shifts by a tier or a
  scale's exponent itself, or numbers a page's cells by its width, is a
  second owner, and `check:rules` refuses it outside `space/`.
- **A layer is a payload and a one-level fold, and nothing else.** No key,
  hierarchy, bounds, window, schedule or residency of its own: which pages
  are wanted, held, fetched and folded is the kernel's `Pyramid<L>`, from
  the one domain (`world/pyramid.rs`), and the bake's `build`
  (`pxl-geo-bake pyramid.rs`). Layers share mechanics, not data — terrain
  folds by majority and average, POI by sum.
- **The view is 3D; storage is not.** A chunk is a cube, addressed by its
  level and x, y and z (`world/pyramid/chunk.rs`), and a cube that holds
  nothing is no chunk. Terrain and POI are pages of the pyramid, a 2D
  heightfield and a 2D count; placed objects are resident in their store, by
  column and band. A lookup points a chunk at them (`Domain::occupied`,
  `Chunk::column`), and nothing else reads them by column. A placed object
  lives in the chunk its band is in — built, made again and drawn with it,
  the ground's lifecycle there — and shares the lattice and the fold rule
  (`majority`, `pxl-spec bake/fold.rs`).
- **2.5D is how ground is stored, not how anything behaves.** A page's
  heightfield is a compression; what the view selects, orders, budgets,
  occludes and draws is 3D — cubes in a sphere about the eye — for terrain,
  places and objects alike. A 2.5D shortcut (a height read off the raster,
  `ground_ele_at`) is an optimization where it is known to behave exactly as
  the 3D rule would, never a model of its own.
- **A mark stands at a cell's center** (`cell_center`), at the height of the
  surface drawn there, never at a center derived from its data's bounds. A
  mark for many things stands at the z16 cell their centroid falls in,
  which folds by count as the things do (`pxl-spec bake/poi_page.rs`).
- **One rule chooses the level; a layer chooses its detail.** The shared
  rule chooses a level for a piece of ground by its distance from the eye,
  finer than z16 too — z17 to z19 near the eye. How much detail a layer
  draws at that level is the layer's own — terrain its cells averaged
  2 × 2, places a lattice sixteen times the ground's cell, objects their own
  cells — a tuning inside the perspective and never a second rule. Where
  the rule asks for a level finer than a layer's data, the layer draws its
  finest: terrain z16, objects their sub-cell tiers down to z19.
- **A number is derived or said aloud.** A capacity or a count that bounds
  what something else already bounds is a second limit derived from nothing
  (a badge count once capped what screen collision already did). A product
  number is raised in the conversation, not buried in a constant.
- **Dev is guarded like terrain.** The tile server answers no page above
  `--top-level`, and that is the whole guard for every pyramid layer; no
  feature carries a dev exception of its own.
- **No test-only API, no adapter kept past the change that needed it.**
  Tests keep the shared algebraic laws (a fold's fixed point, the lattice),
  the wire and storage contracts, and stated invariants; one that restates
  an implementation goes with it.
- **Before calling it done, audit the symmetry of the whole tree**: terrain,
  POI and objects side by side, for anything one does its own way. An
  asymmetry is removed or explained.

## Scope

The MVP is a concept demo — real-world location (Pokémon GO), block building
(Minecraft), lightweight social presence (Cyworld) — meant to show the concept
works, not to carry traffic. **Out of scope, deferred not rejected:** P2P
networking, traffic/scale optimization (AOI filtering, decay, sync protocols),
reporting/moderation. Don't propose them as missing pieces. The engine and the
product surface are at different stages, so check the code before
characterizing any part of the surface.

**The client is a Rust kernel drawing through WebGPU**: in WASM on a page
([issue/20260912-why-the-destination-is-wasm.md](./issue/20260912-why-the-destination-is-wasm.md)),
and on a device linked into a React Native app and drawing through wgpu. The
kernel decides and lists what to draw; the renderer executes and decides
nothing. **Don't propose a WebGL path or pre-WebGPU support** — closed.

**Nothing configures a spending cap, and the bucket's reads cannot bill.**
The guarantee is a composition: the bucket is private, so every read goes
through the Worker, and Workers is on the free plan, which fails closed at
100k requests/day. Three acts break it — a paid Workers plan, the `r2.dev`
public URL, a custom domain on the **bucket** (the Worker is what `plei.me`
is). Don't propose any without saying what it costs
([issue/20260816-deployment-hosting.md](./issue/20260816-deployment-hosting.md)).
Writes are outside it: an upload is a Class A operation an object, and a
publish is a pyramid's worth of them, so `deno task usage` comes before one.

A decision left alone on purpose says so where it lives — a comment beside
the code — so read the code's own comments before "fixing" something that
looks unfinished.

## Code conventions

**Every source file opens with a comment naming its own logical path** — the
import-map path (`// @pxl/web/feature/map/ui/view.tsx`), or for Rust the
package as Cargo.toml spells it and the path under `src/`
(`// pxl-spec/space/indexer.rs`); an executable script opens on its
interpreter. `check:rules` holds every file to it.

**A name's case follows the side it is on.** `PascalCase` for types,
`SCREAMING_SNAKE_CASE` for module constants. Functions and locals are
`snake_case` in Rust, in `packages/xpute` and in `apps/web/src/kernel/`,
which share the kernel's vocabulary, and `camelCase` in the rest of
`apps/` and in `script/`, which is TypeScript and nothing else. An
acronym is a word (`nextChatLineId`, `assetUrl`), as Rust cases it. A name
that crosses — a packet's field, a catalog entry, a profiler record — keeps the
kernel's `snake_case` on both sides. React hooks, stores and components keep
their ecosystem's casing; ported code keeps its original conventions
(`alloc/buddy_tree.rs`). A name Rust would case differently carries an `allow`
and a reason, and a shape that needs none — a `const`, a `LazyLock`, a type
with associated constants — comes first.

**A file's name is its language's**: `snake_case` in Rust and `packages/`,
`kebab-case` in `apps/` and `script/`, `kebab-case` for a file of no language.
What is generated from `spec/` carries the `.spec.` infix on both sides
(`cube_type.spec.rs`, `cube_type.spec.ts`) and is regenerated, never edited;
a test carries `.test.` the same way, and a Rust module reaches either through
`#[path]`. **Directories are singular**, named for the concept they hold,
except where an ecosystem fixes the name (`node_modules/`, `apps/`,
`crates/`), and a file inside one does not repeat it (`toast/store.ts`).

**A shape is an `interface`; what an interface cannot say is a `type`.**
**A number says its width where it has one**: `u32` for a count, length,
index or address; `f64` for a millisecond or coordinate; `u64` for a key;
`i32` for what can be −errno; a typed array that may sit in a
`SharedArrayBuffer` keeps `Uint32Array`, one that cannot is `U32Array`.

**`pxl` is for code, `pixelet` is for infrastructure.** Import paths and
storage keys abbreviate (`@xpute/core`, `pxl:lang`); provisioned resources
spell it out (`pixelet-db-geo`, `pixelet-tiles`).

**Comments explain why, not what.** The long rationale comments — which bug a
branch prevents, why an obvious optimization is wrong, what a constant was
derived from — are the most valuable thing in many files; preserve and extend
them, never trim one for length, in English. A comment earns its place only
when the current reasoning is non-obvious: never restate what the next line
shows, never write it as a changelog ("X → Y because…", "previously…"). Trivial
or visual code gets none unless the choice rests on profiling or a derivation.

**Comments are working notes, not a contract.** Never cite an in-repo comment
as evidence; verify against the code, its behavior or git history. When the
author contradicts a comment — even one written this session — defer. Keep
comments accurate; don't treat them as authority. A source path a comment or a
document names must exist, and `check:rules` says where one does not; a
decision record in `issue/` names the files of its day and is exempt.

## Architecture first

This tree is being built, not maintained. Making it structurally simple and
correct is the objective; keeping what is here is not one. The current
structure, the current tests and the current line count have no preservation
value of their own. Pick the structurally correct shape, matching real prior
art where one exists (Minecraft's chunk model for grid content, Cesium's
`skipLevelOfDetail` for a tile tree whose nodes arrive out of order).

- **An existing mechanism is not a constraint.** An abstraction, a boundary,
  a queue, a cache, a slot layout, a format, an API or a test does not earn
  deference by existing. When a problem turns up inside one, ask first why
  the mechanism is there at all, and whether deleting or replacing it makes
  the problem impossible — before optimizing it, extending it, guarding it
  or repairing it. Compatibility with an internal detail is worth nothing;
  only a contract something outside this tree depends on is.
- **Remove a state rather than handle it.** In order: make the bad state
  unrepresentable; remove the boundary that creates it; fold two
  representations into one; move ownership or lifetime so the conflict
  cannot arise; then scheduling and policy; and last, capacity, a retry, a
  cleanup pass, a periodic sweep, a flag, a guard, a local patch. Some of
  those last are sometimes needed, but none of them is an architecture. Do
  not start at the end because the diff is smaller.
- **Question a transformation before optimizing it.** Where data is copied,
  packed, mirrored, synchronized or reconciled across a boundary, ask whether
  both representations are needed. Where two things are kept in step, ask
  whether one owner would do. Where something is cleaned up, ask whether a
  lifetime makes the cleanup unnecessary. Where a capacity is short, ask
  whether capacity is the problem or whether unrelated states are competing
  for one bounded resource — a queue held what was wanted and what was on
  the wire in one array, and the rows had credits added to them to pay for
  it (2026-09-23).
- **Correctness comes from invariants, not from cleanup.** State them before
  a substantial fix, in the form they will have to hold in: obsolete work
  cannot keep current demand from being represented; residency cannot exceed
  its byte budget; a missing ancestor cannot make a resident descendant
  unreachable; the camera decides the authoritative demand at once; the
  ground has one runtime representation. Prefer a structure the property
  follows from. Do not claim correctness from a periodic sweep, an eventual
  drain, generous capacity, a retry, a timing assumption, "this cannot
  normally happen", or a green test run.
- **Tests carry semantics, not architecture.** They are evidence of intended
  behavior, not a requirement to design around: one that encodes an
  implementation detail is changed or deleted with the detail. Write them
  for what is externally meaningful and for the invariants. The order is
  invariant, architecture, implementation, verification, then the tests
  worth keeping — never a regression test first and a minimal patch until it
  goes green.
- **Profiling chooses between architectures; it does not justify work that
  should not exist.** Work that is only there because of an unnecessary
  representation or an unnecessary boundary is removed with the boundary,
  whether or not it currently measures as cheap — and it is not optimized
  before its deletion has been considered. Implementation size is not a
  cost; server spend and end-user device load are (the valid cost axes;
  turns spent and lines written are not among them).
- **Diff size is not a goal.** A five-hundred-line rewrite that leaves a
  smaller state space beats a twenty-line patch that does not. Do not keep
  an API, a type, a test or a module to hold churn down, and when a change
  makes old machinery unnecessary, delete the machinery and its shims in the
  same breath. A correct larger rewrite beats rounds of patching a wrong
  structure (2026-09-03, the terrain/decor pipeline desync).
- **Split the assumptions before editing.** For any bug or slowness worth a
  change, name the symptom, the mechanism producing it, the assumptions that
  make that mechanism necessary, which of those are the product's and which
  are only the current code's, and whether dropping one of them kills the
  whole class. A mesh slot was a chunk's coordinate, so a level's chunks were
  addressable only inside that level's own ring — which silently decided what
  could be drawn at all, and none of it was a requirement (2026-09-23). Do
  not edit before that split is made.
- **Put a consequential choice in the conversation first.** Where two designs
  imply different representations, ownership, memory or scheduling, stop and
  give both: the invariant, the resulting state space, what machinery goes,
  what machinery arrives, runtime cost, memory cost, migration cost. Never
  quietly take the one that preserves the most code. In the conversation, as
  everything here is — not in a plan document.
- **Review against the new invariant before saying it is done.** What is left
  that exists only for the old architecture? What shim can go now? What
  duplicate representation remains? What still needs cleaning up that could
  instead be impossible? What correctness still rests on capacity or timing?
  Did I optimize around an assumption I should have removed? Did I keep
  something only because a test leaned on it? Designing this subsystem today,
  against the invariant it now has, would I write what I just wrote? If the
  last answer is no, it is not finished.

**When these conflict**, the order is: semantic correctness and stated
invariants; a smaller state space; simpler representation and ownership; a
whole failure class gone; runtime responsiveness; bounded resource
behavior; performance; contracts genuinely outside this tree; effort; diff
size.

## Working agreements

- **Investigate and report before changing anything.** Answer the question;
  don't silently fix what you found. Wait for a go-ahead before editing.
- **Never commit unless asked**, and let work accumulate: a commit per round
  trip is finer than anyone reads. **Then one commit per thing worth reverting
  or reading on its own** — several fixes in one layer together, an infra
  repair and an unrelated tool apart.
- **A branch that is kept is named for why it is kept.** `original/<date>-<name>`
  is the history a squash replaced, pushed before the squash and never moved,
  as git keeps a rewritten branch's under `refs/original/`;
  `checkpoint/<date>-<name>` is a point on main worth coming back to;
  `archive/<date>-<name>` is a line of work set aside rather than merged.
- **A commit's subject is `type(scope): a sentence`.** Conventional Commits'
  prefix — `feat`, `fix`, `refactor`, `docs`, `test`, `chore`, `perf`,
  `build`, `ci` — then a lowercase sentence saying what the change is and
  why, as a comment here would, not a label of what was touched. Scope is the
  directory as the tree names it (`xpute`, `spec`, `render`, `kernel`, `wasm`,
  `web`, `geo`, `geo-bake`, `api`, `infra`, `asset`); a change across several
  has none. Commits before 2026-09-20 are not rewritten.
- **Verify against a running system where one exists.** A conclusion from
  reading alone is a hypothesis; say so. If a predicted fix does not work,
  revert it.
- **"Revert" means leave no trace** — no comment on what was tried, no
  `// removed:` marker.
- **Keep mechanisms orthogonal.** Gestures, animation and styling stay
  independently verifiable; never fuse them into one library mechanism.
- **Design in the conversation, not in a planning artifact.** Plan documents,
  approval gates and agent fan-outs are overhead here; skip them unless asked.
- Code, comments and documentation stay in English.
