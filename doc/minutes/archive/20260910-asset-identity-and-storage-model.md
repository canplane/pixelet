# What identifies an asset, and the storage model that is not unified yet

**2026-09-10** — prompted by asking whether character models and texture packs
should move to R2 next to the tiles. The answer is no, but the question is worth
keeping because it is really a question about resource identity, and answering
it exposed that the storage model has four shapes rather than one.

## Two identity systems already exist

**Domain identity — persisted, a contract.**

```
cube_types.land.BARE = 1        decor_types.tree = 228
```

These bytes go into the object layer's sparse tree and are saved as
`pxl:object:v5`. A byte written today has to mean the same thing tomorrow or
`OBJECT_STORAGE_VERSION` bumps. That version has already moved four times, so
the contract is real and enforced.

**Build identity — ephemeral, not a contract.**

```
/assets/female-a-D-Rd8hvU.glb
```

A Vite content hash. Nothing persists it; it exists inside one build. Each
`.glb`/`.png` is a `?url` import (`@pxl/shared/assets/characters/variants.ts`
and the texture catalogs), so the bundle carries the URL and the bytes are
fetched on demand.

The line is in the right place today: the only thing persisted is the byte, and
the byte-to-asset mapping is resolved fresh on every load by the bundle that
shipped with it.

## Where that stops being true

**The mapping ships whole, to everyone.** 58 `.glb` and 67 `.png` URLs sit in
the main chunk because the catalogs use one static import per file
(`variants.ts` documents why: `import.meta.glob` silently matches nothing once
the pattern crosses out of its own directory tree). Only strings, not bytes —
but the table is in the first chunk a visitor downloads and a player uses one
character at a time.

**A new decor kind is a redeploy**, because the id-to-asset table is compiled
in.

**And the one that actually decides this**: objects have no server persistence
yet, and the design intends to reuse one L2 Volume as the storage/transfer
envelope when they do. The moment objects move between clients,
`decor_types.tree = 228` becomes a wire contract, and 228's asset has to be
resolvable by something other than whichever bundle the receiving client happens
to be running.

## Three models

|                              | identity                                          | cache                                     | cost                                                                                              | right when                                          |
| :--------------------------- | :------------------------------------------------ | :---------------------------------------- | :------------------------------------------------------------------------------------------------ | :-------------------------------------------------- |
| **A. build binding** (today) | TS symbol → content hash, resolved by the bundler | content hash, no manifest needed          | table in the main chunk; new asset means a redeploy; nothing outside the bundle can name an asset | assets are few, fixed, and only this app reads them |
| **B. runtime catalog**       | domain id → URL through a manifest                | content hashes still, inside the manifest | one fetch, one more version to keep in step                                                       | an id has to mean something outside the bundle      |
| **C. B, with bytes in R2**   | same as B                                         | same as B                                 | class-B reads are billed, a second origin, deploy stops being atomic                              | the assets are user-supplied rather than deployed   |

**C is wrong now and later.** Character models are a deploy artifact, not user
content. Moving them buys no caching — Workers static assets are already
edge-cached — and R2 has no spend limit while class-B reads are the one meter
that scales with traffic (`worker/index.ts` says as much about tiles). It also
splits the single origin the deployment exists to keep, and breaks atomic
deploys: the app could reference an asset that has not landed yet.

**Tiles are already the B pattern**, which is why they are not a counterexample:
`<path>/<version>/<page_idx>` is a domain address, not a content hash, and
`manifest.json` carries each source's as-of date. Tiles are bake output —
thousands of files, regenerated on their own cadence, addressed by a spatial
index. Character models are none of those things.

**B is the right direction and not yet.** The trigger is not size. It is
whichever comes first:

1. a second asset kit supplied by something other than a deploy, or
2. objects becoming server-persisted or shared, which turns the numeric catalog
   into a wire contract.

Until one of those lands, B is generality bought ahead of the boundary that
would pay for it — the same trade
[20260910-january-origin-salvage.md](./20260910-january-origin-salvage.md)
concludes the January architecture kept losing.

The eager URL table is separable from all of this. Splitting the catalogs into a
lazily imported chunk is a bundling change inside model A and does not touch
identity.

## The part that is not decided: one storage model

Asking where assets live surfaced that world data does not have one storage
shape, it has four:

| what              | shape                           | addressed by        | persisted                     |
| :---------------- | :------------------------------ | :------------------ | :---------------------------- |
| terrain           | 2.5D Tile, four ground channels | L3 page             | baked, immutable per version  |
| placed objects    | sparse octree per L1 cell       | L1 cell key         | `pxl:object:v5`, client-local |
| scattered decor   | nothing — derived from a seed   | recomputed per page | not stored at all             |
| billboards (POI)  | flat records per page           | L3 page             | baked alongside terrain       |
| billboards (chat) | component-local                 | nothing             | not stored                    |

These are each locally reasonable and collectively unreconciled. The sharpest
gap is billboards: a POI is a world-anchored point with no spatial structure of
its own, fetched as a page-sized list and filtered per frame by
`frustum_budget.ts`. That works at current densities and says nothing about what
happens when billboards are user-authored, numerous, or shared — at which point
they want the same kind of spatial index placed objects already have, and the
question becomes whether the object octree is that index or whether points need
their own.

That is the real unification, and it is unstarted. Asset identity is one axis
crossing it, not a thing to settle on its own: whatever answers "which octree
holds a billboard" will also constrain how a billboard names the asset it draws.

## What this records

- Assets stay in the app deploy, bound at build time (model A).
- R2 is for bake output, not for deploy artifacts.
- Model B is the direction; its trigger is a non-deploy asset supplier or
  server-persisted objects, not bundle size.
- The eager catalog is a bundling problem, fixable without changing any of the
  above.
- Storage-model unification, billboards included, is open and blocks nothing
  here.

## Status 2026-09-13

Superseded by
[20260913-everything-is-a-file-the-placement-and-the-palette.md](./20260913-everything-is-a-file-the-placement-and-the-palette.md).
Both triggers named above for model B fired at once: the palette is one global
record whose ids are the wire contract, and user-supplied models are decided (a
bucket keyed by content hash, an ingest route). The `Palette` record _is_ the
runtime catalog — id to model by hash and bucket key — so the question of what
identifies an asset is answered there, and the four storage shapes listed here
are now one row (`Placement`) over containers that stay as they were. What this
document still holds: why C was wrong _for deploy artifacts_, which is unchanged
— the bucket is for what a deploy cannot carry, not for what it can.
