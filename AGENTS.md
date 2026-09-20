# Agent Instructions

Instructions for AI coding agents working in the pixelet repository. Tool-agnostic
on purpose — Claude Code, Copilot, and Cursor all read this file (directly or
through a symlink), so nothing here should assume a particular assistant.

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
[issue/BACKLOG.md](./issue/BACKLOG.md).

---

## Environment

**This is a Deno workspace, not an npm project.** There is no `package.json`;
`node_modules/` exists only because Deno materializes it. Drive the repo
through its tasks, from the root:

| Task                     | Purpose                                                     |
| ------------------------ | ----------------------------------------------------------- |
| `deno task dev`          | the client alone, every route from the published deployment |
| `deno task dev:local`    | the client and the three servers a deployment has two of    |
| `deno task bake:tiles`   | bake every land page into `./out/terrain/<version>`         |
| `deno task deploy`       | build the client and publish the Cloudflare Worker          |
| `deno task smoke <url>`  | what a deployment must answer, against a running one        |
| `deno task check`        | type-check every workspace member                           |
| `deno task test`         | tests, every workspace member                               |
| `deno task fmt` / `lint` | format / lint                                               |

**`deno task check` is the gate.** Do not report work as done without it
passing. `check`, `test` and `lint` are what CI runs on every push; a green
push to main takes a build number and deploys whichever half its files reached
(`.github/workflows/check-and-deploy.yml`), so a red gate is a deploy that does
not happen.

The two Postgres containers come from `infra/local/docker-compose.yml`. The geo
one (`pixelet-db-geo`) holds a multi-gigabyte PostGIS dataset. **Never `down -v`
it casually** — the volume is the data. Ingestion and operations are
[infra/README.md](./infra/README.md); follow it rather than improvising SQL.

## Boundaries

The tree is grouped by where a thing runs (`crates/xpute`, `crates/pxl/{spec,web,server}`,
`packages/xpute`, `apps/web`); README.md has the map. Three lines are easy to
erode by accident:

- **xpute holds no domain.** No coordinates, terrain, tiles or Overture in
  `xpute-core` or `@xpute/core`; those belong to `pxl-spec`. The view
  (`pxl-render`) knows the model and nothing of the kernel (`pxl-kernel`).
- **`core` is taken as it is; `runtime` asks you to run its way.** The kit
  works in any program; the runtime means a turn on a doorbell, a quota from
  the frame, I/O by credits. `runtime` depends on `core`, never the reverse.
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
`apps/web` is the shell and stays TypeScript.

## Scope and maturity

The MVP is a concept demo — real-world location (Pokémon GO), block building
(Minecraft), lightweight social presence (Cyworld) — meant to show the concept
works, not to carry traffic. **Out of scope, deferred not rejected:** P2P
networking, traffic/scale optimization (AOI filtering, decay, sync protocols),
reporting/moderation. Don't propose them as missing pieces.

**The engine and the product surface are at different stages.** The substrate —
coordinates, floating origin, tile streaming, camera, sensors, the terrain
pipeline — is production-grade; the building layer works (place, erase, stack,
raycast, persist); the chat/social half is a UI shell wired to nothing. Check
the code before characterizing any part of the surface.

**The client is a Rust kernel in WASM drawing through WebGPU**
([issue/20260912-why-the-destination-is-wasm.md](./issue/20260912-why-the-destination-is-wasm.md)).
three.js is gone. The kernel decides and lists what to draw; the renderer
executes and decides nothing. **Don't propose a WebGL path or pre-WebGPU
support** — closed.

**Pick the structurally correct shape**, matching real prior art where one
exists (Minecraft's chunk model for grid content). Implementation size is not
a cost; server spend and end-user device load are. A correct larger rewrite
beats rounds of patching a wrong structure (2026-09-03, the terrain/decor
pipeline desync).

**One validated UI pattern to keep:** `mode_bar` reuses an idle slot rather
than adding an element — the send button borrows the palette pill's
collapsed-icon slot, the size flyout anchors per swatch. Reviewed and kept on
purpose; don't discard it in a rework without raising it.

## Deliberate omissions — do not "fix" these

Found, understood, left alone. Re-flagging them wastes time; fixing them
unprompted causes regressions or premature hardening.

- **`pxl-geo`'s forward search is unhardened** — no rate limit, timeout or
  abort. It is one indexed query on one table, the only route that reaches a
  database at request time, and hardening it is a traffic decision with no
  traffic. Tiles are the Worker's and the bucket's; see the last item.
- **`MapStatus.ERROR` is unreachable.** Tile failures are logged and swallowed
  and the effect sets `SUCCESS` regardless, though the toast can render ERROR.
- **`OVERTURE_BUILDINGS_SQL` and `OVERTURE_TRANSPORTATION_SQL` do not clip to
  the page envelope** (`terrain/repository.rs`): a polygon grazing a page costs
  it the whole geometry. Clipping broke ring closure; any attempt must preserve
  closure — clip-and-reclose, Weiler–Atherton, Sutherland–Hodgman with holes —
  never naive bbox clipping. The water branch of `OVERTURE_BASE_SQL` and
  `OVERTURE_COAST_SQL` does clip (`ST_Intersection` guarded by `ST_IsEmpty`):
  those rows run to hundreds of thousands of points.
- **Placed objects live in `localStorage`.** A demo with no server-side state.
- **Nothing configures a spending cap, and R2 still cannot bill.** The
  guarantee is a composition: the bucket is private, so every read goes through
  the Worker, and Workers is on the free plan, which fails closed at 100k
  requests/day. Three acts break it — a paid Workers plan, the `r2.dev` public
  URL, a custom domain on the **bucket** (the Worker is what `plei.me` is).
  Don't propose any without saying what it costs
  ([issue/20260816-deployment-hosting.md](./issue/20260816-deployment-hosting.md)).

## Overlay CSS over the canvas

The UI sits over a canvas that repaints every frame, and WebKit handles that
worse than Chromium. Three rules, each learned from the scale bar:

- **`box-shadow` / `text-shadow`, not `filter: drop-shadow()`.** A filter
  region over the canvas smeared, clipped transformed children and vanished
  under an animated ancestor on iOS. `filter` only when the shape is irregular
  and following its alpha is the point.
- **Animate `transform` and `opacity`, not layout.** A `width` tween ghosted
  on iOS. When neither is clean, ask whether the animation is worth having —
  the scale bar snaps between its 1/2/5 steps.
- **Avoid `backdrop-filter` over the canvas.** It re-blurs what is behind it
  every frame; the leading suspect for a Galaxy S23 janking where an iPhone
  13 mini ran fine, and why the map controls and the address pill carry an
  opaque fill. Only where the element is rarely on screen.

## Sizing UI against something

Chrome sizes come from measuring screenshots of apps doing the same job, at
device scale. Already derived — don't re-derive or drift:

|                                | Measured from              | Value                 |
| ------------------------------ | -------------------------- | --------------------- |
| Map circular controls          | Naver / Kakao Map          | 42, stacked 5 apart   |
| mode_bar tool & swatch pitch   | Instagram DM icon row      | 38, 18 between glyphs |
| Equal-width action button pair | Instagram Follow / Message | 32 tall, ~13 label    |
| Floating overlay inset         | iOS keyboard accessory bar | 16                    |

- **A tap target is as wide as the pitch and no wider**; past that it overlaps
  a neighbour and the overlap belongs to whichever is later in the DOM. Where
  the visible thing is smaller (colour swatches), the button is a transparent
  box at full pitch with the shape centred.
- **The named radius steps are theme tokens.** `app/style/index.css` derives
  `--radius-*` from `--radius`, which themes redefine as far as 2.5rem, so `rounded-md` is not a
  fixed number. Spell literals out (`rounded-[6px]`) when a radius is meant.

## Code conventions

**Every source file opens with a comment naming its own logical path** — the
import-map path with its extension (`// @pxl/web/feature/map/ui/view.tsx`); a
Rust file gives its package name as Cargo.toml spells it, hyphens and all,
then the path under `src/` (`// pxl-spec/space/indexer.rs`). A crate named in
a comment is hyphenated; only a `::` path keeps the underscore. New files get
one too, tests included. The one exception is `shared/ui/shadcn/`, uniformly
header-less as a flat shadcn tree; its notice is in
[THIRD_PARTY_NOTICES.md](./THIRD_PARTY_NOTICES.md). Don't add headers there,
and don't drop them elsewhere.

**A name's case follows the side it is on.** `PascalCase` for types,
`SCREAMING_SNAKE_CASE` for module constants. Functions and locals are
`snake_case` in Rust, in `packages/xpute` and in `apps/web/src/kernel/`,
which share the kernel's vocabulary, and `camelCase` in the rest of
`apps/web` and in `script/`, which is TypeScript and nothing else. An
acronym is a word (`nextChatLineId`, `assetUrl`), as Rust cases it. A name
that crosses — a snapshot field, an action string, a catalog entry, a ledger
record — keeps the kernel's `snake_case` on both sides.

**A file's name is its language's**: `snake_case` in Rust and `packages/`,
`kebab-case` in `apps/web` and `script/`. The WGSL beside the Rust and the
SQL Postgres runs in order are `snake_case` too; a file that belongs to no
language — a document, a workflow, a shell script — is `kebab-case`. What is
generated from `spec/` is Rust's shadow: its stem is the JSON's,
`snake_case`, and it carries the `.spec.` infix on both sides
(`cube_type.spec.rs`, `cube_type.spec.ts`, `mem/mem.spec.rs` for
`mem/mem.json`), the way a test carries `.test.`; a Rust module reaches it
through `#[path]`, as `.test.rs` is reached, and keeps its module name.
Every identifier in it is `snake_case`.

**Directories are singular** — named for the concept they hold (`object/`,
`marker/`, `issue/`), never for their count. The exceptions are names an
ecosystem already expects: ones its tooling fixes (`node_modules/`,
`@types/`, `.github/workflows/`) and ones its people reach for (`apps/`,
`packages/`, `crates/`, `.agents/skills/`). A module too (`constant.ts`,
`route.ts`), and a file inside a directory that names the concept does not
repeat it (`setting/store.ts`).

**A name Rust would case differently carries an `allow` and a reason.** Two
shapes earn one: a constant that cannot be a `const` — it reads what the shell
installed at boot, or is not const-evaluable — kept as a zero-argument function
(`APP_ENV()`, `RETAIN_BUDGET_BYTES()`); and a file in the spec's own notation,
where a stateless operation is named like a C macro, for the whole file
(`wire/xtp/`, `space/pvf3/`, `space/adapter/`) with the `allow` at the top.
Reach first for a shape that needs none: a `const`, a `LazyLock`, a type with
associated constants where a module stood in for a namespace
(`MaterialClasses`). `macro_rules!` escapes the lint and is the wrong trade.

**A shape is an `interface`; what an interface cannot say is a `type`.** A
record, a contract a class implements, a library type with fields added is an
`interface`, which the compiler names and caches as one. A union, a tuple, an
alias of a width (`u32`, `U8Array`), a mapped or conditional type is a `type`.

**A number says its width where it has one.** A count, length, index or
address is `u32`; a millisecond, coordinate or Float64 word is `f64`; a key is
`u64`; a palette value is `u8`; what can be −1 or −errno is `i32`. A number
that is only a number stays `number`. A typed array over an `ArrayBuffer` for
certain is `U32Array` and kin; one that may sit in a `SharedArrayBuffer` keeps
`Uint32Array`.

**Ported code keeps its original conventions** (`shadcn/tabs.tsx`). Some of
`@xpute/core` came from other projects to stay reusable; the divergence is
deliberate. React hooks, stores and components keep their ecosystem's
`camelCase`/`PascalCase` (`useTileStore`, `MapStatus`), and a component file
holding several is plural (`overlays.tsx` for `<Overlays />`).

**`pxl` is for code, `pixelet` is for infrastructure.** Import paths and
storage keys abbreviate (`@xpute/core`, `pxl:lang`); provisioned resources
spell it out (`pixelet-db-geo`, `pixelet-tiles`). Word separators follow the
target: snake for Postgres, kebab for Docker and R2 (which rejects underscores).

**Comments explain why, not what.** The long rationale comments — which bug a
branch prevents, why an obvious optimization is wrong, what a constant was
derived from — are the most valuable thing in many files; preserve and extend
them, never trim one for length, in English. A comment earns its place only
when the current reasoning is non-obvious: never restate what the next line
shows, never write it as a changelog ("X → Y because…", "previously…"). Trivial
or visual code gets none unless the choice rests on profiling or a derivation.

**Comments are working notes, not a contract.** Never cite an in-repo comment
as evidence; verify against the code, its behaviour or git history. When the
author contradicts a comment — even one written this session — defer. Keep
comments accurate; don't treat them as authority.

## Working agreements

- **Investigate and report before changing anything.** Answer the question;
  don't silently fix what you found. Wait for a go-ahead before editing.
- **Never commit unless asked**, and let work accumulate: a commit per round
  trip is finer than anyone reads. **Then one commit per thing worth reverting
  or reading on its own** — several fixes in one layer together, an infra
  repair and an unrelated tool apart.
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
- Type-level generics that compile to nothing are fine with one consumer.
- **Design in the conversation, not in a planning artifact.** Plan documents,
  approval gates and agent fan-outs are overhead here; skip them unless asked.
- Code, comments and documentation stay in English.

## Skills

Reusable procedures live in [`.agents/skills/`](./.agents/skills/), one
directory per skill with a `SKILL.md`, kept vendor-neutral. Assistant
directories are gitignored; link them at the shared content:

```bash
mkdir -p .claude
ln -sfn ../AGENTS.md      .claude/CLAUDE.md
ln -sfn ../.agents/skills .claude/skills
```

Personal, uncommitted variants use a `.local.` infix (`*.local.skill.md`,
`.agents/skills/.local/`) — see `.gitignore`.
