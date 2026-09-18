# Agent Instructions

Instructions for AI coding agents working in the pixelet repository. Tool-agnostic
on purpose — Claude Code, Copilot, and Cursor all read this file (directly or
through a symlink), so nothing here should assume a particular assistant.

For what the system is, see [README.md](./README.md) and
[ARCHITECTURE.md](./ARCHITECTURE.md). For why past decisions went the way they
did, see [issue/](./issue/). For what is queued and in what order,
see [issue/BACKLOG.md](./issue/BACKLOG.md) — work follows the list, not what was last
noticed.

---

## Environment

**This is a Deno workspace, not an npm project.** There is no `package.json` to
install from, and `node_modules/` exists only because Deno materializes it.
Always drive the repo through its tasks, run from the repo root:

| Task                     | Purpose                                                                           |
| ------------------------ | --------------------------------------------------------------------------------- |
| `deno task dev`          | the client and the three servers a deployment has two of                          |
| `deno task dev:remote`   | client only, server routes from the published deployment                          |
| `deno task bake:tiles`   | bake every land page into `./out/terrain/<version>`                               |
| `deno task ingest:asset` | hash every file under `asset/` into `./out/asset`, regenerate the palette's table |
| `deno task deploy`       | build the client and publish the Cloudflare Worker                                |
| `deno task smoke <url>`  | what a deployment must answer, against a running one                              |
| `deno task check`        | type-check every workspace member                                                 |
| `deno task test`         | tests, every workspace member                                                     |
| `deno task fmt` / `lint` | format / lint                                                                     |

`deno task check` is the gate. Do not report work as done without it passing.
`check`, `test` and `lint` are also what CI runs on every push and pull
request, and a green push to main deploys
(`.github/workflows/check-and-deploy.yml`, infra/README.md §7) — so a red gate
is not just a local inconvenience, it is a deploy that does not happen.

The two Postgres containers come from `infra/local/docker-compose.yml`, and the geo one
(`pixelet-db-geo`) holds a multi-gigabyte PostGIS dataset that is expensive to
rebuild. **Never `down -v` it casually** — the volume is the data. Ingestion and
operational procedures are documented in [infra/README.md](./infra/README.md);
follow that rather than improvising SQL against the raw tables.

## Repository layers

```
asset                   the shipped models and texture pack; data, not code — nothing imports it
spec                    the numbers and names every language is generated from (script/spec.ts): pxl-spec's at the top, kernel/, render/ and xpute/ below
packages/xpute/core     generic runtime kit — collections, codecs, XTP binary protocol
packages/pxl/shared     pixelet domain — PVF3 coordinates, cell/terrain model, bake wire format
crates/xpute/core       the generic runtime kit in Rust; no domain
crates/pxl/spec         the spec in Rust, the server's and the client's — the space, the cell catalogs, the commands, the bake's formats, the assets' tables; @pxl/shared's counterpart
crates/pxl/render       the view — the volumes and meshes a chunk bakes into, glTF, the billboards' sheet, the draw list, the shaders, the device
crates/pxl/kernel       the kernel — the stores, the fetches, the schedulers, the turn; what is drawn and when, handed to the view
crates/pxl/wasm         the one WebAssembly module: rings, a boot block and a doorbell over the crates
crates/pxl/geo-bake     the offline half of the geo data: the Overture ingests, the three page bakes, and the dev tile server
crates/pxl/asset-bake   the asset tree's own tooling: the texture pack, the brand's icons, and the ingest that writes the catalogs' tables
tool                    what is left of the hand-run tools: the DEM into db-geo, in bash and GDAL
apps/web                Vite + React 19 client, drawing through WebGPU
```

Two boundaries matter, and both are easy to erode by accident:

- **`@xpute/core` must contain no pixelet domain knowledge.** It is meant to stay
  portable to other projects. Coordinates, terrain, tiles, Overture — none of
  those concepts belong there. If something needs them, it belongs in
  `@pxl/shared`. The same line runs through the Rust: `crates/xpute/core`
  holds no domain, and the domain is `crates/pxl/spec`'s. The view
  (`crates/pxl/render`) knows the model and nothing of the kernel
  (`crates/pxl/kernel`): the kernel hands a frame what it draws.
- **`@pxl/shared` is a contract between `apps/web` and the servers.** Both sides
  encode and decode the same binary formats from this one source. Changing a
  format changes both ends at once; there is no versioned negotiation between
  them.

## Scope and maturity

The MVP is a concept demo: real-world location (Pokémon GO style) + block
building (Minecraft style) + lightweight social presence (Cyworld style). It is
meant to show the concept works, not to carry real traffic.

**Explicitly out of MVP scope:** P2P networking, traffic/scale optimization (AOI
filtering, decay systems, sync protocols), and reporting/moderation. These are
deferred, not rejected — they get re-evaluated when the project leaves the MVP
milestone. Don't propose them as missing pieces.

**The engine and the product surface are at very different stages, and grading
them on one axis produces nonsense.** The substrate — coordinate system,
floating-origin anchor registry, tile streaming, camera, sensor lifecycle,
terrain render pipeline — is production-grade. The building/object layer is real
and working (place, erase, stack, raycast, persist). The chat/social half is a UI
shell wired to nothing. UI polish here does not imply feature completeness; check
current code before characterizing any part of the product surface.

**The WASM/WebGPU move is the priority, and the renderer is through it.**
The destination — a Rust core compiled to WASM, drawing through WebGPU,
three.js gone, iOS 26 and iPhone 11 as the floor
([issue/20260912-why-the-destination-is-wasm.md](./issue/20260912-why-the-destination-is-wasm.md))
— is where the client stands since 2026-09-14: the core builds, decides
and lists what to draw, and the renderer (`crates/pxl/render/src/gpu/`)
executes the list through the host's WebGPU and decides nothing. What is still TypeScript is glue and the pieces not
yet moved, listed in P0 of [issue/BACKLOG.md](./issue/BACKLOG.md): a piece
is first cut out behind a boundary that carries typed arrays, still in
TypeScript, and only then does what sits behind the boundary become Rust.
The state is already in the shape the move wants: lanes in one linear
memory, one tick, records. Build the next thing in that shape; don't port
a piece ahead of the list, and don't propose keeping a WebGL path or
supporting pre-WebGPU devices — that question is closed.

**That doesn't license implementation-shaped architecture.** What is still
TypeScript is going to move — by whoever ends up doing it, on whatever
timeline — so building in TypeScript now is a language/runtime choice, not
licence to let a React quirk decide a core design. Pick the structurally correct shape for the problem (matching
real prior art where one exists, e.g. Minecraft's own chunk model for
grid-content rendering) even in TS, so a later rewrite carries the design
over rather than rediscovering it. Two failure modes to avoid, both hit in
practice (2026-09-03, terrain/decor render-pipeline desync): don't use "we
might rewrite this later" to justify a lesser design now, and don't treat
implementation size/effort as a cost that argues for a smaller ad-hoc patch
over the correct structure — the only real costs are server spend and
end-user device load, and a correct larger rewrite usually beats several
rounds of patching a wrong one anyway.

**One validated UI pattern worth keeping:** `mode_bar` reuses an idle slot
instead of adding a new element — the send button borrows the palette pill's
collapsed-icon slot, and the size flyout anchors per-swatch rather than to one
shared control. That flow was reviewed and confirmed to read cleanly, so it is a
judgment call to reach for again, not an accident. Don't discard it in a rework
without raising it first.

## Deliberate omissions — do not "fix" these

These look like defects and are not. They were found, understood, and left
alone on purpose. Re-flagging them as fresh discoveries wastes everyone's time;
"fixing" them unprompted causes regressions or premature hardening.

- **Tile API hardening is absent by choice.** No response cache beyond the
  in-process LRU, no app-level concurrency cap, no cancellation on client abort,
  no per-request timeout, no rate limiting. The client-server integration shape
  isn't settled, and hardening it now would be premature. Revisit only when
  deployment/traffic planning makes it concrete.
- **`MapStatus.ERROR` is unreachable.** Tile fetch failures are logged and
  swallowed; the effect sets `SUCCESS` regardless of per-tile outcome, even
  though the toast UI has real ERROR rendering wired up. Known, same reasoning as
  above.
- **`OVERTURE_BUILDINGS_SQL`/`OVERTURE_TRANSPORTATION_SQL` (repository.ts) do not
  clip feature geometry to the page envelope.** A sprawling polygon that merely
  grazes a page's bbox can spike that one page's bake cost, independent of page
  size. Clipping was tried and destroyed closed-ring topology — polygons
  rendered broken. The unclipped version is the fix for that regression, not an
  oversight. Any future attempt has to preserve ring closure across the
  boundary (clip-and-reclose, Weiler–Atherton, Sutherland–Hodgman with hole
  handling), never naive bbox clipping. `OVERTURE_BASE_SQL`'s water branch and
  `OVERTURE_COAST_SQL` already clip via `ST_Intersection` (guarded by
  `ST_IsEmpty` against `&&`'s bbox-only false positives) and are not affected —
  their rows run large enough (up to 444,626 points) that skipping the clip
  costs far more than any topology risk.
- **Placed objects live in `localStorage`.** Deliberate for a demo with no
  server-side user state.
- **Nothing configures a spending cap, and R2 still cannot bill.** The
  guarantee is a composition, not a setting: the bucket is private, so every
  read goes through the Worker; Workers is on the free plan, which fails
  closed at 100k requests/day rather than charging. Three deliberate acts
  break it — moving Workers to a paid plan, enabling the `r2.dev` public URL,
  or attaching a custom domain to the **bucket** (as opposed to the Worker,
  which is what `plei.me` is). Do not propose any of the three as an
  improvement without saying what it costs; see
  [issue/20260816-deployment-hosting.md](./issue/20260816-deployment-hosting.md).

## Overlay CSS over the canvas

The UI layer sits on top of a WebGL canvas that repaints continuously, and
WebKit handles that combination worse than Chromium does. Two rules came out of
chasing the same symptom three times through the scale bar:

- **Prefer `box-shadow` / `text-shadow` to `filter: drop-shadow()`.** A filter
  builds a new rendering context and a filter region; over the canvas on iOS
  that produced smearing, clipped the element once its children were
  transformed, and vanished outright while an ancestor animated opacity — none
  of it reproducible on Android or desktop Chromium. Reach for `filter` only
  when the shape is genuinely irregular and following its alpha is the point;
  a rectangle or a rounded pill does not need it.
- **Animate `transform` and `opacity`, not layout properties.** A `width` tween
  over the canvas ghosted on iOS for the same reason. And when neither is
  available cleanly, consider whether the animation is worth having at all —
  the scale bar ended up snapping between its 1/2/5 steps, which is what the
  convention it follows does anyway.

- **Avoid `backdrop-filter` over the canvas.** It is the same trap one level
  up: a backdrop filter re-reads and re-blurs whatever is behind it, and what
  is behind it here repaints every frame, so the cost is paid continuously
  rather than once. This is the leading suspect behind a Galaxy S23 janking on
  a build an iPhone 13 mini ran fine, and it is why the map's circular controls
  and the address pill carry a more opaque fill instead of a blur. Reach for it
  only where the element is rarely on screen.

Both are cross-browser rules rather than iOS workarounds: what a layout
animation or a filter costs over a canvas differs per engine, so the portable
choice is the one that stays on the compositor.

## Sizing UI against something

Chrome sizes here come from measuring screenshots of apps doing the same job,
at their real device scale, not from picking round numbers. The ones already
derived that way, so they don't get re-derived or drifted off:

|                                | Measured from              | Value                 |
| ------------------------------ | -------------------------- | --------------------- |
| Map circular controls          | Naver / Kakao Map          | 42, stacked 5 apart   |
| mode_bar tool & swatch pitch   | Instagram DM icon row      | 38, 18 between glyphs |
| Equal-width action button pair | Instagram Follow / Message | 32 tall, ~13 label    |
| Floating overlay inset         | iOS keyboard accessory bar | 16                    |

Two rules that came out of doing it:

- **A tap target can be as wide as the pitch and no wider.** Past that it
  overlaps a neighbour, and the overlap silently belongs to whichever element
  is later in the DOM. Where the visible thing is smaller than that (the colour
  swatches), the button is a transparent box at full pitch with the visible
  shape centred inside — costs nothing, roughly doubles the target.
- **The named radius steps are theme tokens in this repo.** `app/style/index.css`
  derives `--radius-*` from `--radius`, which themes redefine as far as 2.5rem,
  so `rounded-md` is not 6px and is not even a fixed number. Spell literals out
  (`rounded-[6px]`) when a specific radius is meant.

## Code conventions

**Every source file opens with a comment naming its own logical path:**

```ts
// @pxl/web/ui/map/view.tsx
```

This is the import-map path, not the filesystem path — extension included, so
a `.tsx` file says `.tsx`. New files get one too, tests included. A Rust file
says it the same way: its package's name as Cargo.toml has it, hyphens and
all, then the file's path under `src/`: `// pxl-spec/space/indexer.rs`. A
comment that names a crate by name uses the same hyphenated form; only a
`::` path, which is code, keeps the underscore.

The one exception is `kit/ui/shadcn/`, where no file carries a header —
it's a flat, one-file-per-component shadcn tree, uniformly header-less rather
than inconsistently so. Don't mass-add headers there, and don't read its
absence as licence to drop them anywhere else.

**Naming:** `snake_case` for files, functions, and locals; `PascalCase` for types;
`SCREAMING_SNAKE_CASE` for module constants. **Directories are singular**:
a directory is named for the concept it holds — `object/`, `marker/`,
`tool/`, `issue/` — not for how many files are in it, which is
always several and never the point. A plural would have to answer whether
the directory is one thing or a collection of things, and for most
directories that has no clear answer (is `object/` the object layer or
the objects?), so nothing here asks it. The exceptions are names the
ecosystem fixes and tooling reads: `apps/`, `packages/`, `node_modules/`,
`@types/`, `.github/workflows/`, `.agents/skills/`. The same holds for a
module: `constant.ts`, `route.ts`, `slab.ts` — and a file inside a
directory that already names the concept does not repeat it
(`setting/store.ts`, not `setting/settings_store.ts`). Two things are not
concept names and keep their plurals: a tool script, which is a command
(`bake_tiles.ts` bakes the tiles; `ingest_buildings.ts` ingests Overture's
Buildings theme), and code ported from elsewhere, which keeps its own
names (`shadcn/tabs.tsx`). React hooks, stores, and components
keep their ecosystem's `camelCase`/`PascalCase` (`useTileStore`, `MapStatus`) —
that is not an inconsistency to fix.

**Code ported from elsewhere keeps its original conventions.** Some of
`@xpute/core` came in from other projects to stay reusable. Do not sweep it into
house style; the divergence is deliberate.

**`pxl` is for code, `pixelet` is for infrastructure.** Import paths and storage
keys abbreviate (`@pxl/shared`, `pxl:lang`); provisioned resources spell the
project out — `pixelet-db-geo`, `pixelet-network`, `pixelet_geo` in
`.env.example`, and the tile bucket in `apps/web/wrangler.jsonc`. Neither is a
lapse to normalize into the other.

Word separator follows the target system rather than one repo-wide choice:
snake for Postgres identifiers, where a hyphen would force quoting at every
use; kebab for Docker and for R2, where names are DNS-shaped — R2 rejects
underscores in bucket names outright. Check what the platform accepts before
assuming either.

**Comments explain why, not what.** This codebase carries unusually long
rationale comments — which bug a branch exists to prevent, why an obvious
optimization is wrong, what a constant was re-derived from. They are the most
valuable thing in many files. Preserve them when refactoring, extend them when
you add a non-obvious decision, and never trim one just because it is long. Write
them in English.

A comment earns its place only when the _current_ reasoning is genuinely
non-obvious. Never restate something the code already shows on the same or
next line (a literal value, a class name, an effect a reasonable reader would
infer) — that isn't a "what" comment given for free, it's the same fact said
twice. Never write it as a changelog entry either ("X -> Y, changed because
...", "previously did Z, now does ..."): state only why the code is the way it
is now, not the history of how it got there. This bar is almost never cleared
by trivial or purely visual code (a Tailwind class, a small UI tweak) — those
default to no comment at all, unless the choice is backed by something as
load-bearing as real profiling data or a non-trivial derivation (the same bar
"what a constant was re-derived from" already sets above), not just a
stylistic preference.

**Comments are working notes, not a contract.** Never cite an in-repo comment,
doc-string, or "the contract says X" as evidence when justifying something —
verify against the actual code, its behavior, or git history instead, every
time. Much of the prose in this repository was written by a previous agent turn
describing what it decided the semantics _should_ be, occasionally without
matching what the code does. When the author contradicts a comment — including
one added earlier in the same session — defer immediately rather than arguing
from the old text. This is not licence to let comments rot: keep them accurate,
just don't treat them as authority.

## Working agreements

- **Investigate and report before changing anything.** For a question about how
  something works, answer it — don't silently "fix" what you found along the way.
  Wait for an explicit go-ahead before editing or deleting.
- **Never commit unless asked.** Finishing a task is not a reason to commit.
  Leave the working tree for review.
- **Let work accumulate before committing.** Don't commit at the end of each
  report or fix — that produces a commit per round trip, which is finer than
  anyone wants to read. Keep going, and commit once there's a coherent batch
  (or when asked).
- **Then: one commit per thing worth reverting or reading on its own** — not
  one per file, and not one per bug. Several small fixes found in the same
  pass, in the same layer, belong together; an infrastructure repair and an
  unrelated new tool do not, because reverting one should not drag in the
  other. When in doubt, ask whether the two halves would ever be undone
  separately.
- **Verify claims against a running system where one exists.** A conclusion
  reached only by reading code is a hypothesis. Say so when that's all you have,
  and if a predicted fix turns out not to work, revert it rather than leaving it
  in.
- **Don't propose measuring what is going to be replaced.** Instrumenting
  code the plan already retires — the React glue a piece leaves behind on its
  way into the core, today — decides nothing before it leaves and only
  lengthens the milestone. Measure what survives the move, or what a decision in front
  of us actually turns on; otherwise say what is known and move on.
- **"Revert" means leave no trace.** Restore the exact prior text — no comment
  explaining what was tried, no `// removed: ...` marker, no doc note about the
  tradeoff. An abandoned in-session experiment gets erased completely.
- **Keep mechanisms orthogonal.** Gestures, animations, and styling stay
  independently verifiable; do not fuse them into one library mechanism because
  it's convenient at the call site.
- Type-level generics that compile away to nothing are fine even with a single
  consumer. Zero runtime cost is not premature abstraction.
- **Design in the conversation, not in a planning artifact.** Even a large
  redesign is worked out by discussing and building it directly. Plan documents,
  approval gates, and exploratory agent fan-outs are overhead here — skip them
  unless asked for.
- Code, comments, and documentation stay in English.

## Skills

Reusable procedures live in [`.agents/skills/`](./.agents/skills/), one directory
per skill with a `SKILL.md` inside. They are kept here — rather than in a
vendor-specific folder — so a change of assistant doesn't strand them.

Assistant-specific directories (`.claude/` and friends) are gitignored; link them
at the shared content instead of copying it:

```bash
mkdir -p .claude
ln -sfn ../AGENTS.md      .claude/CLAUDE.md
ln -sfn ../.agents/skills .claude/skills
```

Personal, uncommitted variants use a `.local.` infix (`*.local.skill.md`,
`.agents/skills/.local/`) — see `.gitignore`.
