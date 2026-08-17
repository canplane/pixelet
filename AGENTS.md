# Agent Instructions

Instructions for AI coding agents working in the pixelet repository. Tool-agnostic
on purpose — Claude Code, Copilot, and Cursor all read this file (directly or
through a symlink), so nothing here should assume a particular assistant.

For what the system is, see [README.md](./README.md) and
[ARCHITECTURE.md](./ARCHITECTURE.md). For why past decisions went the way they
did, see [discussions/](./discussions/).

---

## Environment

**This is a Deno workspace, not an npm project.** There is no `package.json` to
install from, and `node_modules/` exists only because Deno materializes it.
Always drive the repo through its tasks, run from the repo root:

| Task | Purpose |
| --- | --- |
| `deno task dev` | web + api together |
| `deno task dev:remote` | client only, tiles from the published deployment |
| `deno task bake:tiles` | bake every land page into `./out/tiles/<version>` |
| `deno task deploy` | build the client and publish the Cloudflare Worker |
| `deno task check` | type-check every workspace member |
| `deno task test` | tests (only `apps/web` and `packages/pxl/shared` have any) |
| `deno task fmt` / `lint` | format / lint |

`deno task check` is the gate. Do not report work as done without it passing.

The two Postgres containers come from `infra/docker-compose.yml`, and the geo one
(`pixelet-db-geo`) holds a multi-gigabyte PostGIS dataset that is expensive to
rebuild. **Never `down -v` it casually** — the volume is the data. Ingestion and
operational procedures are documented in [infra/geo/README.md](./infra/geo/README.md);
follow that rather than improvising SQL against the raw tables.

## Repository layers

```
packages/xpute/core     generic runtime kit — collections, codecs, XTP binary protocol
packages/pxl/shared     pixelet domain — PVF3 coordinates, cell/terrain model, bake wire format
apps/api                Hono server + the OSM/DEM bake pipeline
apps/web                Vite + React 19 + React Three Fiber client
```

Two boundaries matter, and both are easy to erode by accident:

- **`@xpute/core` must contain no pixelet domain knowledge.** It is meant to stay
  portable to other projects. Coordinates, terrain, tiles, OSM — none of those
  concepts belong there. If something needs them, it belongs in `@pxl/shared`.
- **`@pxl/shared` is a contract between `apps/web` and `apps/api`.** Both sides
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
milestone, alongside the parked WASM/WebGPU direction. Don't propose them as
missing pieces.

**The engine and the product surface are at very different stages, and grading
them on one axis produces nonsense.** The substrate — coordinate system,
floating-origin anchor registry, tile streaming, camera, sensor lifecycle,
terrain render pipeline — is production-grade. The building/object layer is real
and working (place, erase, stack, raycast, persist). The chat/social half is a UI
shell wired to nothing. UI polish here does not imply feature completeness; check
current code before characterizing any part of the product surface.

**Don't volunteer the Rust/WASM port.** It's parked deliberately (see
[discussions/](./discussions/)); finishing the MVP in TypeScript with Three.js
used directly, unwrapped, is the current direction.

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
- **`OSM_FEAT_SQL` does not clip feature geometry to the page envelope.** A
  sprawling polygon that merely grazes a page's bbox can spike that one page's
  bake cost, independent of page size. Clipping was tried and destroyed
  closed-ring topology — polygons rendered broken. The unclipped version is the
  fix for that regression, not an oversight. Any future attempt has to preserve
  ring closure across the boundary (clip-and-reclose, Weiler–Atherton,
  Sutherland–Hodgman with hole handling), never naive bbox clipping.
  `OSM_COAST_SQL` already clips via `ST_Intersection` and is not affected.
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
  [discussions/20260816-deployment-hosting.md](./discussions/20260816-deployment-hosting.md).

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

| | Measured from | Value |
| --- | --- | --- |
| Map circular controls | Naver / Kakao Map | 42, stacked 5 apart |
| mode_bar tool & swatch pitch | Instagram DM icon row | 38, 18 between glyphs |
| Equal-width action button pair | Instagram Follow / Message | 32 tall, ~13 label |
| Floating overlay inset | iOS keyboard accessory bar | 16 |

Two rules that came out of doing it:

- **A tap target can be as wide as the pitch and no wider.** Past that it
  overlaps a neighbour, and the overlap silently belongs to whichever element
  is later in the DOM. Where the visible thing is smaller than that (the colour
  swatches), the button is a transparent box at full pitch with the visible
  shape centred inside — costs nothing, roughly doubles the target.
- **The named radius steps are theme tokens in this repo.** `app/styles/index.css`
  derives `--radius-*` from `--radius`, which themes redefine as far as 2.5rem,
  so `rounded-md` is not 6px and is not even a fixed number. Spell literals out
  (`rounded-[6px]`) when a specific radius is meant.

## Code conventions

**Every source file opens with a comment naming its own logical path:**

```ts
// @pxl/web/modules/world/terrain/use_tile_fetcher.ts
```

This is the import-map path, not the filesystem path — extension included, so
a `.tsx` file says `.tsx`. New files get one too, tests included.

The one exception is `shared/ui/theme/`, where no file carries a header. That
tree is shadcn-derived and its authored variants (`glass/`, `cozy/`) follow the
same shape deliberately, so it is uniformly header-less rather than
inconsistently so. Don't mass-add headers there, and don't read its absence as
licence to drop them anywhere else.

**Naming:** `snake_case` for files, functions, and locals; `PascalCase` for types;
`SCREAMING_SNAKE_CASE` for module constants. React hooks, stores, and components
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

**No dead code by deletion.** Code being retired gets archived on a branch rather
than deleted outright, so the reasoning stays recoverable.

**Comments are working notes, not a contract.** Never cite an in-repo comment,
doc-string, or "the contract says X" as evidence when justifying something —
verify against the actual code, its behavior, or git history instead, every
time. Much of the prose in this repository was written by a previous agent turn
describing what it decided the semantics *should* be, occasionally without
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
- **"Revert" means leave no trace.** Restore the exact prior text — no comment
  explaining what was tried, no `// removed: ...` marker, no doc note about the
  tradeoff. An abandoned in-session experiment gets erased completely. That is
  the opposite of the archive-a-branch rule above, and the distinction is
  deliberate: shipped code being retired is worth preserving, a change made this
  session that didn't pan out is not.
- **Keep mechanisms orthogonal.** Gestures, animations, and styling stay
  independently verifiable; do not fuse them into one library mechanism because
  it's convenient at the call site.
- Type-level generics that compile away to nothing are fine even with a single
  consumer. Zero runtime cost is not premature abstraction.
- **Design in the conversation, not in a planning artifact.** Even a large
  redesign is worked out by discussing and building it directly. Plan documents,
  approval gates, and exploratory agent fan-outs are overhead here — skip them
  unless asked for.
- The author works in Korean. Respond in Korean unless asked otherwise; code,
  comments, and documentation stay in English.

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
