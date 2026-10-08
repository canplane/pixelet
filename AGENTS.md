# Agent Instructions

For any AI coding agent in this repository, whichever reads it. It holds what
an agent would get wrong without it; what is local lives beside the code it is
about.

What the system is: [README.md](./README.md) and
[ARCHITECTURE.md](./ARCHITECTURE.md) for the application,
[xpute/README.md](./xpute/README.md) and
[xpute/ARCHITECTURE.md](./xpute/ARCHITECTURE.md) for the system under it.
**`xpute/` never names pixelet**: it is xpute's repository root, copied
there byte for byte (`xpute/script/export.ts`) with its own Cargo and Deno
workspaces, toolchain and checks. How the four are written:
[.agents/skills/write-documents](./.agents/skills/write-documents/SKILL.md).
Why past decisions went as they did: [decision/](./decision/). What is
queued: [BACKLOG.md](./BACKLOG.md). Procedures — a bake, a publish — are the
skills in [.agents/skills/](./.agents/skills/).

---

## Environment

**A Deno workspace, not an npm project**, driven by `deno task` from the
root; README.md lists the tasks. The one `package.json` is the device app's
(`apps/native`), where Expo and Metro find packages as npm lays them out.

**`deno task check` is the gate before a push to `deploy`**, which is how
publishing happens ([infra/README.md](./infra/README.md) §5), or when asked;
`check:diff` checks only what changed. A kernel change reaches the running
dev server through `deno task build:wasm`. main is not built by CI.

**Never `down -v` the geo database** (`pixelet-db-geo`): the volume is a
multi-gigabyte PostGIS dataset. Ingestion and operations are
[infra/README.md](./infra/README.md), not improvised SQL. **A table is filled
beside itself and swapped in, never emptied first**, a change of shape
included (§2 there): a table dropped or truncated by hand leaves its readers
nothing for the hours a re-ingest takes.

## Boundaries

The tree is grouped by where a thing runs (`xpute/`, `crates/{spec,client,server}`,
`apps/web`, `apps/native`);
README.md has the map. Three lines erode by accident:

- **xpute holds no domain.** No coordinates, terrain, tiles or Overture in
  `xpute-core` or `@xpute/core`; those are `pxl-spec`'s. The view
  (`pxl-render`) knows the model and nothing of the kernel (`pxl-kernel`).
- **`core` is taken as it is; `guest` and `host` ask you to run their way**
  — a turn on a doorbell, a quota from the frame, I/O asked of the host.
  Both depend on `core`, never the reverse. `core` is symmetric across every
  language xpute is written in, held to it by the golden records under
  `xpute/spec/golden/`. `guest` and `host` are the two ends of one
  arrangement, named by role, not language: a module missing on one side is
  no gap, only the format must agree, and the guests in different languages
  are symmetric as `core` is, each in its own language's shape. Never number the languages, hosts or
  guests: the count is stale the day one more arrives.
- **`pxl-spec` is the one model and has no TypeScript twin.** The shell reads
  a tile through the kernel, never by decoding one; what it holds — the cell
  catalog, the asset tables, the ABI enums — is generated into
  `apps/web/src/` from `spec/`, beside the Rust from the same JSON.

**What shares `pxl-spec` is Rust, and what does not is free.** The kernel,
the servers and the bakes compile the model, so they are crates; an ingest
into Postgres shares nothing, so it is a script. `apps/web` is the
TypeScript around the kernel, two things to it: its host, which holds the
memory, the turns, the devices and the I/O (most of `src/kernel/`), and its
shell, the UI, whose end of the channel is `shell.ts`, `snapshot.ts` and
`render/surface.ts`. A command is between the kernel and one of them
(`cmd.json`). The device app runs the same TypeScript and adds only what a
device does, behind `@platform/`.

**The kernel tells facts; the shell decides how they are shown.** The
snapshot carries what the kernel owns — the world's state, the camera's and
the devices', a physical quantity like the meters a pixel spans. A length
rounded for a scale bar, how deep a place is named, when a toast speaks,
which modal is open are the shell's, derived where read; a constant both
sides need is generated from `spec/`. A field sent for one presentation is
the boundary in the wrong place.

**A tool that makes a file is Rust; one that talks to a service is
TypeScript.** What writes or reads a format the Rust side owns, or processes
a binary — the asset ingest, the stars, the Moon, the brand, the code
generated from `spec/` — is a binary of `pxl-tool` (`crates/tool`) and
links no crate it writes into. The shell's build is `apps/web/script/`; the
tree's checks and what reaches a deployment or a bucket are
`script/`; a script under `infra/` makes a database's state.

**On an iPhone, a control a touch lands on is UIKit's.** SwiftUI (`@expo/ui`)
draws the sheet and nothing inside React Native's views that takes a touch:
a SwiftUI switch there missed quick taps. Which technology draws each
`@platform/` piece: [apps/native/src/platform/README.md](./apps/native/src/platform/README.md).

## One spatial model

Everything placed in the world sits in one hierarchy, the **pyramid**, of
tiles from the leaves at z15 to z5, where it stops for now and not by any
limit. **Every size is a zoom**: the X lane is one world of 2^24 units, so a
thing the world over 2^z wide is at zoom z, and a bigger zoom is finer
(`pxl-spec space/coord/zoom.rs`). A cell, a chunk and a tile are each named by it:
terrain's cell is z24, a unit, and objects' finest z27; a chunk is
`CHUNK_STEPS` zooms coarser than the camera grid's cells it draws (32 a
side, `space/camera/grid.rs`); a tile holds a layer's own count
of cells a side, the same at every zoom (`Lattice`), so terrain's z24 cells
make z15 tiles.
Where several zooms meet, a name says whose it is (`cell_zoom`). A web map's
zoom is the 3857 adapter's alone (`web_zoom_of`), for comparing with one.
**The pyramid is planar and never leaves Pvf3** (`pxl-spec space/coord/pvf3.rs`),
which follows 3857 and leaves the poles out (`space/coord/adapter/4326.rs`
`CLAMP_LAT`). The planet is its superset, a sphere: where the pyramid holds
no tile, the sphere answers (the Earth's global picture, `pxl-kernel
layer/earth.rs` `earth_window`). A plane and a sphere are two domains, not
two copies of one fact, and no fold carries the pyramid onto the sphere.

- **The view keeps only what an observer can tell apart.** The world is
  stored in each layer's shape; what the view holds and does is bounded by
  the screen. The eye's view is quantized into cells, each a solid angle on
  the sphere about the eye — a screen only calibrates how many pixels one
  spans — and inside a cell nothing is promised: a piece refines only where
  its error shows the eye more than a cell's area. Storage, query, demand, residency,
  prefetch, materialization and scheduling are separate concerns, each with
  its own bound; ARCHITECTURE.md's "The view" states them — read it before
  changing one.
- **The lattice has one owner.** A size is a zoom stepped from two roots —
  the leaf tile (`space/layer/tile.rs` `LEAF_ZOOM`, z15) and each layer's
  lattice knob, its cells a tile (`space/layer/lattice.rs`; the ground's 512,
  `GROUND`) — and parent and children in
  any number of axes are `space/hierarchy/tree.rs`'s. A cell's base, center and
  place in its tile (the index into every lane the tile has), the 4 × 4
  neighborhood a fold reads, and a tile's name are `space/layer/`'s. A
  layer that shifts by a zoom's exponent (`shamt`), or numbers a tile's cells
  by its width, is a second owner; `check:rules` refuses it outside `space/`.
- **A layer is a payload and a one-zoom fold, and nothing else.** Which
  tiles are wanted is the observation's demand (`view/demand.rs`
  `observe`), which are held residency's (`layer/residency.rs`), when they are
  fetched the scheduler's (`io/request.rs`), and a tile is folded only by the
  bake's `build` (`pxl-geo-bake pyramid.rs`). Layers share mechanics, not
  data — terrain folds by majority and average, places by sum.
- **The view is 3D; storage is not.** A chunk is a cube, addressed by zoom
  and x, y, z (`chunk.rs`); a cube that holds nothing is no
  chunk. Terrain and places are tiles, a 2D heightfield and a 2D count; placed
  objects are resident in their store, a sparse tree a z8 column. Each layer,
  and the world composed of them, is read through one query
  (`layer/query.rs` `Query`): what a node holds and its records, at a cell
  zoom the asker states; a point or a ray is asked of the world's
  (`query.rs`). Nothing outside a layer reads its records in the
  layer's shape, and which of a layer's tiles holds a chunk's cells is the
  layer's lattice's (`pxl-spec space/layer/lattice.rs` `read_tile`). A placed object lives in the chunks its cube is in — built,
  remade and drawn with it — and shares the lattice and the fold rule
  (`majority`, `pxl-spec bake/fold.rs`).
- **2.5D is how ground is stored, not how anything behaves.** What the view
  selects, orders, budgets, occludes and draws is 3D, height no different
  from the rest, for terrain, places and objects alike. A 2.5D shortcut (a
  height read off the raster, `ground_ele_at`) is an optimization where it
  behaves exactly as the 3D rule would, never a model of its own.
- **A mark stands at a cell's center** (`Cell::center`), at the height of the
  surface drawn there: one thing's at the leaf cell it stands in, many at
  their cell's middle (`pxl-kernel chunk/place.rs`).
- **The camera grid is the resolution; a layer folds what is in a cell.**
  The grid's cell is the distance's (`pxl-spec space/camera/grid.rs` `Tau::cell_at`,
  `SPLIT` its one knob, one factor τ), and one cell of it is one thing
  drawn. A chunk is that grid's cells, so it refines only where the screen
  sees an error cross a cell, alike on every axis, and something under it
  is held finer. What lies inside a cell is its layer's to fold into one,
  never a second rule outside it: terrain's by majority and average in the
  bake, objects' by `majority`, places' to a lattice sixty-four ground cells
  wide. A chunk draws a cell from what is composed there one zoom finer
  (`space/layer/lattice.rs` `Lattice::fold`, the ground's `GROUND`) as its
  surfaces' mean color and its fill's volume (`pxl-render
  mesh/chunk.rs`); past a layer's data it draws its finest. What is folded is a summary, so nothing
  in a folded cell is struck, hidden or cut by a preview.
- **Dev is guarded like terrain.** The tile server answers no tile more than
  `--folds` zooms over the leaves, the whole guard for every layer. A source declares the
  coarsest zoom it supplies whole — the published version in its metadata,
  the tile server at `/tile/source` — and the floor is that zoom.
- **Terrain, places and objects are audited side by side** before a change is
  done: anything one does its own way is removed or explained.

## What the bake paints

**The bake follows precedent, not invention.** A way's width, the order ways
paint in, what a sidewalk, a crossing or a slip road is, what is left off the
surface, how high a deck stands: each rule comes from the renderers and
converters that met the data first and from the source's schema, cited
beside the code — OpenStreetMap Carto (`openstreetmap-carto-flex.lua`),
OpenMapTiles/MapTiler's schema, Arnis, osm2streets and osm2lanes, Overture's
documented properties. A number none of them gives is said aloud with its
reason. Sample what the data holds before trusting a field: Overture's `road`
column was empty, and much of `width_rules` is an import's width classes.
What a chunk can make when drawn — a deck's slab, a tunnel's mouth — is the
client's; the bake carries only what the client cannot work out
([decision/20261001-ways-off-the-ground.md](./decision/20261001-ways-off-the-ground.md)).

**A leaf reads its 1-ring and no further — an invariant, never a default.**
A terrain leaf's bake reads at most itself and its eight neighbours: a
query, a search radius or a way's geometry past them is a violation,
however small. Reading a neighbour's own derived data (its padded
elevation) carries that neighbour's reach along, and even so nothing
reaches past the source's 2-ring. Every bake, the divisions' z12 leaves
too, reads no farther past its leaf than one reach (`pxl-geo-bake
coverage.rs` `BAKE_REACH`, a z13 tile), which the shard's apron holds; a rule
that wants more, a structure that runs on, is cut there, not given a
wider read.

**A read is scoped by the policy that consumes it.** What the bake asks of
the database is what its paint policy uses: the policy's own tables are the
read's filter (`pxl-geo-bake overture/policy.rs`), never a second list kept
in SQL beside them.

## Scope

The MVP is a concept demo — real-world location (Pokémon GO), block building
(Minecraft), light social presence (Cyworld) — to show the concept works,
not to carry traffic. **Deferred, not rejected, and not to be proposed as
missing:** P2P networking, traffic and scale (AOI filtering, decay, sync
protocols), reporting and moderation. The engine and the product surface are
at different stages: check the code before characterizing the surface.

**The client is a Rust kernel drawing through WebGPU**: WASM on a page
([decision/20260912-why-the-destination-is-wasm.md](./decision/20260912-why-the-destination-is-wasm.md)),
wgpu linked into a React Native app on a device. The kernel decides and lists
what to draw; the renderer executes. **No WebGL path or pre-WebGPU support** —
closed.

**Nothing configures a spending cap, and the bucket's reads cannot bill**:
the bucket is private, so every read goes through the Worker, on the free
plan, which fails closed at 100k requests/day. A paid Workers plan, the
`r2.dev` public URL or a custom domain on the **bucket** (the Worker is what
`plei.me` is) breaks it; none is proposed without its cost
([decision/20260816-deployment-hosting.md](./decision/20260816-deployment-hosting.md)).
Writes are outside it: a publish is a pyramid's worth of Class A operations,
so `deno task usage` comes before one.

## Code conventions

**The bar is [.agents/reference/calab-loader/](./.agents/reference/calab-loader/),
structural before stylistic.** A layout is one block derived top-down from a
few roots; each fact has one owner and is said once; a derived name exists
only where more than one place reads it; a procedure is straight steps under
short section comments. A change that leaves two names for one fact, one
conversion in two places, or two computations of one quantity meeting at a
seam is not finished.

**Every source file opens with a comment naming its logical path** — the
import-map path (`// @pxl/web/feature/world/ui/view.tsx`), for Rust the package
as Cargo.toml spells it and the path under `src/` (`// pxl-spec/space/coord/zoom.rs`),
for C++ the path from its repository's root (`// cpp/guest/ipc/ring.cpp`); an executable
script opens on its interpreter. `check:rules` holds every file to it.

**A name's case follows its side.** `PascalCase` for types,
`SCREAMING_SNAKE_CASE` for module constants; functions and locals
`snake_case` in Rust, `xpute/ts` and `apps/web/src/kernel/`, which
share the kernel's vocabulary, and `camelCase` in the rest of `apps/` and in
`script/`. An acronym is a word (`assetUrl`). A name that crosses — a
packet's field, a catalog entry, a profiler record — keeps the kernel's
`snake_case` on both sides. React hooks, stores and components keep their
ecosystem's casing; ported code keeps its own (`alloc/buddy_tree.rs`). A name
Rust would case differently carries an `allow` and a reason, after a shape
that needs none — a `const`, a `LazyLock`, associated constants.

**A file's name is its language's**: `snake_case` in Rust, C++ and
`packages/`, `kebab-case` in `apps/`, `script/` and for a file of no
language. Generated from `spec/` carries `.spec.` on both sides
(`cube_type.spec.rs`, `cube_type.spec.ts`) and is regenerated, never edited;
a test carries `.test.`; a Rust module reaches either through `#[path]`.
**Directories are singular**, named for the concept they hold, except where an
ecosystem fixes the name (`node_modules/`, `apps/`, `crates/`), and a file
inside one does not repeat it (`toast/store.ts`).

**A C++ guest is written the way the Rust one is.** C++20, exceptions and
RTTI off, none of the library's runtime surface (`iostream`, locale,
filesystem). RAII, templates, `constexpr`, `std::array`, `std::span`,
`std::optional` are used where they say something; plain code stays as C
would write it. Memory is the guest's own: its one allocator over the heap
section, `.fixed` for what has a bound (`pxl-kernel mem/fixed.rs`), a settled
turn asking the heap for nothing — so a standard container allocates through
the guest's heap, never the system's `operator new`.

**A shape is an `interface`; what an interface cannot say is a `type`.** **A
number says its width**: `u32` for a count, length, index or address; `f64`
for a millisecond or coordinate; `u64` for a key; `i32` for what can be
−errno; a typed array that may sit in a `SharedArrayBuffer` keeps
`Uint32Array`, one that cannot is `U32Array`.

**A double is the same bits on every platform.** Rounding is xpute's `round`
(half to even, as WGSL); a transcendental is the pinned `libm`'s, never the
platform's, which `crates/clippy.toml` refuses.

**`pxl` is for code, `pixelet` for infrastructure.** Import paths and storage
keys abbreviate (`@xpute/core`, `pxl:lang`); provisioned resources spell it
out (`pixelet-db-geo`, `pixelet-tiles`).

**Comments are few and short**: a few words at the line they explain, saying
what a reader would get wrong without them — the failure a branch prevents,
where a number comes from, a non-obvious algorithm or invariant, a decision
left alone on purpose. Never restate the code, narrate a design, its
alternatives or its history, or list references; that is `decision/`'s.
Trivial code gets none.

**Comments are working notes, not a contract.** Read them before "fixing"
what looks unfinished, but never cite one as evidence: verify against the
code, its behavior or git history, and when the author contradicts one,
defer. A path a comment or document names must exist (`check:rules`);
`decision/` names the files of its day and is exempt.

## The main axis

Every structural question is judged against one picture, and a change is
wanted where it points the code further down it. Its small precedent is the
bar under "Code conventions", [.agents/reference/calab-loader/](./.agents/reference/calab-loader/):
a few roots everything is derived from, dependencies running one way from
the shared mechanism to each policy, and every state owned by a
transition — the same reduction at the size of one loader.

- **The eye decides.** The view is a grid of angular cells about the eye; a
  cell's ray lands at a distance that fixes the resolution there, and a
  chunk is that grid's cells at one zoom.
- **The query is the only seam.** A chunk asks a node of the query — a
  bound to descend by, the records to build from — and storage's shape
  stays behind it; render is handed cells.
- **Descent carries what it knows.** Drawing and casting descend the
  pyramid, each step answered from its parent, never re-asked from the top.
  Speed is sought only inside that hierarchy (the camera grid's resolution,
  the folded pyramid); a special-cased fast path beside it is refused even
  when measured faster. How a thing is drawn is weighed as a whole, on the
  CPU or on the GPU, never one kind of it sent to the other side.
- **Dependencies point one way:** turn, door and shell (the UI's way in)
  wire everything from the top, and nothing below names them; control,
  which the door's events enter, moves the eye over the view and names
  neither scene nor chunk, whose share of what it decides the turn hands
  down; scene → chunk → view → query → layer;
  io and the turn's edge and phases below everyone, io as transport; leaves
  (settings, models, billboards) below what reads them. The profiler is a
  debug path beside the axis: it reads anything, and production reaches it
  only where it records. An edge up the axis is a defect.

**Reducing is measured by what disappears** — a state, an owner, a name, a
path, a copy — never by line count. In order of value:

1. **One fact, one representation.** Two expressions of one fact fold into
   one; done repeatedly, it is what leaves the dependencies a tree. A rebake
   or a broken old artifact weighs nothing against it. Two voices for one
   role means one goes; the one exception is an interface whose
   implementations genuinely differ, as each layer answers the query.
2. **One fact, one owner** ("Architecture first" below). A fact the bake and
   the client both need — the drawn surface, its bound — is one function
   where both compile, and a bound is derived from that function's own
   reads, never from a second fact about it.
3. **A state removed rather than handled**, in "Architecture first"'s order.

**Never by force.** Things merge because they turn out to be one structure —
a lattice under several layers, a column at every zoom — and what differs by
nature is split. Either is done whole: a decision once made is carried
through completely, and is not reopened by an audit. The code usually
already does what is wanted; the change makes it say so once, by extending
the structure that holds the fact, not by inventing mechanism beside it.

**Identity, not revision.** A tile's content is fixed by its name within a
version, and a published version is named by the source and the code that
made it; what changes is
which identity answers a node. Keys compare identities, and the one change event left
carries identities, not a counter — only content that mutates under a fixed
key, the objects' edits, needs its own. An extremal summary (max, OR, a
revision) carries every change to the top, so one is stored only where a
bound is the contract and content changes only at bake; where content
mutates, it is computed from the records when asked.

## Architecture first

The tree is being built, not maintained: making it structurally simple and
correct is the objective, and the current structure, tests and line count
have no value of their own. Take the structurally correct shape, matching
real prior art where it exists. Google Earth and the web map's tile pyramid
are the axioms for structure, storage, tiling and invalidation across zooms,
with Cesium's 3D Tiles (`skipLevelOfDetail`) for a tile tree whose nodes
arrive out of order. Minecraft is neither isotropic nor a pyramid: it can
inspire what happens inside a cell or a chunk, never the structure.

- **An existing mechanism is not a constraint.** An abstraction, boundary,
  queue, cache, slot layout, format, API or test earns no deference by
  existing. When a problem turns up inside one, ask whether deleting or
  replacing it makes the problem impossible before optimizing, extending,
  guarding or repairing it. Only a contract something outside the tree
  depends on is kept for compatibility; machinery a change makes unnecessary
  goes in the same change, with its shims.
- **One fact, one owner; the rest derived.** A value that follows from
  another is computed where it is read. A constant, limit, reserve or policy
  is derived from the roots that fix it; where none can be, what makes the
  quantity unbounded is fixed first (a write per change folded into one per
  turn), not the number. A count bounding what something else bounds is a
  second limit; a product number is said aloud and named once, in
  `pxl-kernel param.rs` under the policy it tunes, with its reason. A module
  reads the contract above a concept, not the concept: a controller's words
  (a pivot, a radius, a zoom) stay in the controller, and what is generic
  reads the geometric fact (the eye, the look, the focus). Code keeping
  one value in step with another is a second copy of a truth — remove it, do
  not tune the sync — and a derived value is never written back into its
  source each turn; an outside event writes the source once. Where data is
  copied or reconciled across a boundary, ask whether both representations
  are needed; where something is cleaned up, whether a lifetime makes it
  unnecessary; where a capacity is short, whether unrelated states compete
  for it.
- **Remove a state rather than handle it**, in this order: make the bad
  state unrepresentable; remove the boundary that creates it; fold two
  representations into one; move ownership or lifetime so the conflict
  cannot arise; then scheduling and policy; last, a capacity, retry,
  cleanup pass, sweep, flag, guard or local patch, none of which is an
  architecture.
- **Correctness comes from invariants.** State them before a substantial
  fix, in the form they must hold in — obsolete work cannot keep current
  demand from being represented; residency cannot exceed its byte budget; a
  missing ancestor cannot make a resident descendant unreachable; the
  camera decides the demand at once; the ground has one runtime
  representation — and prefer a structure they follow from. A sweep, an
  eventual drain, generous capacity, a retry, a timing assumption, "this
  cannot normally happen" or a green test run proves nothing.
- **Tests carry semantics, not architecture.** They test what is externally
  meaningful — the algebraic laws (a fold's fixed point, the lattice), the
  wire and storage contracts, the invariants — and one that encodes an
  implementation detail goes with the detail. No API exists for a test
  alone. The order is invariant, architecture, implementation,
  verification, then the tests worth keeping.
- **Remove work before adding machinery to speed it up.** Before a cache,
  ordering, index or retained state, find the work that can go. A **free
  reduction** — behavior-equivalent, nothing kept, nothing else spent:
  allocation on a settled turn, a repeated traversal, a copy, a bind or an
  upload that changes nothing, work for a consumer that is off — is done
  without measurement. A **trade** — one resource for another, or anything
  that changes what is seen — has its cost and gain said before it is done,
  and is measured where the structure does not settle it. **Structural
  debt** — dead state, a fact kept twice, an owner left from an older
  architecture, a wrapper, a sync — is removed. Less work beats faster work,
  less state beats cached state, a derivation beats a synchronization;
  profiling ranks trades and licenses no waste. The costs are server spend
  and device load, never implementation size or turns.
- **What memory being full means decides how it is sized.** **Must-fit
  state** (the resident terrain, the mesh slots' tables, the block
  registry) is sized from a bound the structure proves or an owner keeps by
  admission; past it is a bug, never a knob. **A bounded flow** (a stream to
  the host, a ring, the profiler's window) has a capacity said as a product
  number; full is ordinary, met by its owner's policy — the producer waits,
  the oldest drops, the excess is cut and said to be — and a reserve in it is
  derived from what cannot wait. **A workspace that grows with the world**
  (the heap's) asks for room first; a refusal is backpressure, the work
  waits and what is drawn stays drawn. One item larger than any room is
  refused at the boundary it came through, or is a bug if the program made
  it. Memory pressure slows progress or quality; it never opens a hole in
  what is shown, drops what someone placed, or ends the module.
- **Split the assumptions before editing.** For a bug or slowness worth a
  change: the symptom, the mechanism producing it, the assumptions that make
  the mechanism necessary, which are the product's and which only the
  code's, and whether dropping one kills the whole class.
- **A choice between designs goes to the conversation first**: where two
  imply different representations, ownership, memory or scheduling, give
  both — the invariant, the state space, the machinery that goes and
  arrives, runtime, memory and migration cost. Plan documents, approval
  gates and agent fan-outs are overhead, skipped unless asked.
- **Done is reviewed against the new invariant**, over every path that
  produces the symptom: what exists only for the old architecture, what shim
  can go, what representation is still duplicated, what cleanup could be
  made impossible, what correctness still rests on capacity or timing, what
  was kept only because a test leaned on it. If, designing it today, you
  would not write what you wrote, it is not done.

**When these conflict**, the order is: semantic correctness and stated
invariants; a smaller state space; simpler representation and ownership; a
whole failure class gone; runtime responsiveness; bounded resource behavior;
performance; contracts outside the tree; effort; diff size.

## Working agreements

- **Commit only when asked, one commit per thing worth reverting or reading
  on its own** — several fixes in one layer together, an infra repair and an
  unrelated tool apart; never one per round trip.
- **A kept branch is named for why it is kept.** `original/<date>-<name>` is
  the history a squash replaced, pushed before it and never moved, as git
  keeps a rewritten branch's under `refs/original/`;
  `checkpoint/<date>-<name>` is a point on main worth returning to;
  `archive/<date>-<name>` is work set aside rather than merged.
- **A commit's subject is `type(scope): a sentence`**: a Conventional
  Commits prefix (`feat`, `fix`, `refactor`, `docs`, `test`, `chore`,
  `perf`, `build`, `ci`), then a lowercase sentence saying what the change is
  and why, not what was touched. The scope is the directory as the tree names
  it (`xpute`, `spec`, `render`, `kernel`, `wasm`, `web`, `geo`, `geo-bake`,
  `api`, `infra`, `asset`); a change across several has none. Commits before
  2026-09-20 are not rewritten.
- **"Revert" leaves no trace** — no comment on what was tried, no
  `// removed:` marker.
- **Mechanisms stay orthogonal.** Gestures, animation and styling are
  verified independently, never fused into one library mechanism.
- **Agents working at once run optimistically.** They share the checkout and
  edit, commit and push without asking one another; a conflict that might
  happen is no reason to wait, lock or protect another's edits. Only an
  actual git conflict, or edits that cannot both stand, is resolved — one
  agent at a time in an order agreed once, each taking the latest, fixing,
  committing and pushing before the next. A broken intermediate state is
  allowed; correctness is held where work is integrated. An agent commits
  its own paths only (`git commit -- <paths>`): the index is shared, and a
  path carries every hunk in it, so read each file's diff first and name
  another's hunk in the commit. A build
  that fails on the working tree is no one's commit yet; say whose file it
  is before calling main broken.
- Code, comments and documentation stay in English.
