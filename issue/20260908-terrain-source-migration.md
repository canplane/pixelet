# Terrain's turn: move its OSM half to Overture and GLO-30 to GEDTM30 — the real payoff is one lifecycle, not fresher data

**2026-09-08** — revisits the "Buildings/Transportation/Base themes and
GEDTM30 are next-gen sources, not adopted yet" position recorded in
[20260908-overture-poi-adoption.md](./20260908-overture-poi-adoption.md).
That position still holds on the merits it was argued on (schema-only
convenience, no license win, not worth a pipeline rewrite _on its own_).
This document is about a reason that section didn't weigh: **lifecycle
unification.** Nothing here is decided yet — it's the scope, the blockers,
and a sequencing proposal.

## The thing that actually motivates this

POI and reverse-geocode already refresh themselves. Terrain doesn't, and
can't, as long as it's on OSM.

|               | Terrain (`tiles/v1`)                                                                                    | POI / geocode (`poi/v2`, `geocode/v1`)                                        |
| ------------- | ------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------- |
| Source ingest | `osm2pgsql --slim --hstore-all` + split land-polygons shapefile + `CLUSTER` — multi-GB, stateful, local | `overturemaps` CLI pull for a bbox — runs in a throwaway CI postgis container |
| Refresh       | Manual, `bake-tiles` skill procedure                                                                    | `.github/workflows/overture-monthly-refresh.yml`, cron `0 3 1 * *`            |
| Publish       | Immutable: new `tiles/vN` prefix, app redeployed with new `VITE_TILE_BASE_URL`                          | Mutable: `aws s3 sync --delete` over the same prefix, `max-age=86400`         |

The monthly workflow's own comment spells out why terrain is excluded:

> _Deliberately needs no OSM/terrain infrastructure in CI ... instead of
> re-ingesting all of OSM into a fresh database every month just to
> regenerate it._

`osm2pgsql` of a country PBF plus a global land-polygons shapefile is not
something to run monthly in CI. `overturemaps` for a bbox already is —
it's the existing Places/Divisions ingest step. **Moving terrain's feature
source to Overture removes the single reason terrain can't join that
workflow.** The goal isn't fresher buildings (a three-week-old building
footprint matters as little as a three-week-old restaurant listing) — it's
one pipeline, one place things break, one mental model, instead of two.

## Elevation: GLO-30 → GEDTM30, and it doesn't need to be automated

Covered in [20260908-overture-poi-adoption.md](./20260908-overture-poi-adoption.md)'s
next-gen section — GEDTM30 (CC-BY 4.0, 30 m, drop-in GeoTIFF, bare-earth,
strips the DSM canopy/building noise GLO-30 bakes into ground elevation).
FABDEM was considered and rejected: CC BY-NC-SA, needs a paid Fathom
license for this project's for-profit use.

GEDTM30 is a fixed 2025 dataset with no release cadence. GLO-30 barely
changes either. Elevation is "bake once per coverage change", not a driver
of the monthly cycle — so "can't automate elevation" is not a real loss
against the lifecycle-unification goal. `ESA_ELE_SQL` in
`terrain/repository.ts` is source-agnostic (`ST_Union`/`ST_Resample`/
`ST_Clip` over whatever raster is in `esa_elevation_raster`); swapping the
DEM is a data reload, not a code change.

**Unconfirmed:** whether the DSM-noise artifact is actually visible in the
current build. Worth a direct look before committing to a full elevation
re-bake.

## Migration scope — what the OSM→Overture feature swap touches

The bake's OSM coupling is in three layers; the rasterizer, the tile
format, the `cube_types` output vocabulary, and the R2 serving path are all
untouched.

| Piece                                             | Change                                                                                                                                                                       | Difficulty   |
| ------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------ |
| `shared/bake/format.ts` row types                 | `FeatRow`/`CoastRow`/`OSMTags` reshaped to the new SQL result                                                                                                                | Low          |
| Elevation path                                    | none (data reload only)                                                                                                                                                      | —            |
| Ingestion tools                                   | new `overture_buildings` / `overture_transportation` / `overture_base_*` tables; `ingest_places.ts` is the template                                                          | Low–Med      |
| `terrain/repository.ts` SQL                       | one `osm_elements` query → per-theme queries (Buildings has `height`/`num_floors`/`class`; segments have `subtype`/`class` + `road`; Base has `subtype`/`class`)             | Medium       |
| `bake/osm/model/osm_adapter.ts`                   | drop the hstore parser; keep WKB decode; structured columns instead                                                                                                          | Low–Med      |
| `bake/terrain/pipeline/osm_policy.ts` (464 lines) | near-total rewrite: every branch keys off OSM vocab (`natural=`, `building:levels`, `waterway`, `aeroway`, open-ended tags) → Overture's constrained `class`/`subtype` enums | **Med–High** |
| Coastline / land-vs-ocean mask                    | OSM `land-polygons-split-3857` has no Overture equivalent — see below                                                                                                        | Medium       |
| Module/file naming                                | `bake/osm/` → `bake/overture/` (or neutral `bake/features/`)                                                                                                                 | Cosmetic     |

Two things that make the policy rewrite less bad than the line count
suggests:

- **Terrain POI output is already disabled** — `encode_terrain` has the
  `pois` field commented out; POI comes entirely from Overture Places now.
  So `osm_policy.ts`'s `category === "poi"` / `amenity`/`shop`/`tourism`
  handling is _deleted_, not ported.
- **Overture's schema is genuinely better for this use**: `height` in
  meters directly instead of `building:levels * 3.5` guesswork; structured
  `road` lanes/width instead of `DEFAULT_LANES_BY_HIGHWAY` fallback tables;
  closed `class` enums instead of open hstore. The policy file should come
  out shorter and more predictable.

**All of the regression risk is in "bake it and look".** Every Overture
`class` value needs a considered `cube_types` mapping and there's no way to
know it's right without a re-bake and a visual diff. Do the mapping
`class`-value by `class`-value against Overture's schema reference — not by
porting OSM branches.

## The coastline / ocean-mask gotcha

`osm_coastline_polygons` comes from OSM's `land-polygons-split-3857`
shapefile — a purpose-built, topologically clean global land-polygon set.
It drives three things:

- `ground_baker.ts`'s sea/land base fill (the inverse-sea pass)
- `bake_tiles.ts`'s `HAS_LAND_SQL` → open-ocean page classification → the
  shared `_ocean` fallback tile (roughly two thirds of the coverage bbox is
  open sea, so this optimization is load-bearing)
- `classify_ocean_pages.ts`, and the `_meta/land_pages.json` object the
  monthly workflow fetches from R2 rather than regenerating

Overture has no equivalent product. Land/water lives in the Base theme
(`water` polygons with `subtype=ocean`, plus `land`). Options:

- (a) derive the land mask by inverting Overture `water subtype=ocean`
  polygons — full unification, but Overture's Base water is not the same
  clean split-polygon product, so this needs its own validation pass
- (b) hybrid: keep OSM `land-polygons` for the ocean mask only, take
  features from Overture — de-risks the migration, but leaves one OSM
  dependency in place

Once terrain runs in the monthly workflow, this classification gets
regenerated there every month instead of being a manual re-upload — which
is cleaner _if_ the Overture-derived mask works. Higher stakes than before
because it's now in the automated path.

## Two blockers the data swap alone doesn't solve

**1. Tile publishing convention — immutable-versioned vs mutable-in-place.**
Terrain tiles are `max-age=31536000, immutable`, published to a new
`tiles/vN` prefix each time with the app redeployed to point at it; the
`bake-tiles` skill is emphatic that a version is _never_ overwritten in
place (one write invalidates CDN edge + browser cache + any future OPFS
cache with no purge call). A monthly terrain refresh can't `sync --delete`
over `tiles/v2` the way POI does. Either:

- move terrain to the mutable convention (accept ~1-day-stale terrain, work
  out edge + OPFS cache invalidation), or
- keep immutable, have the workflow bake to `tiles/v{N+1}`, bump
  `VITE_TILE_BASE_URL`, and redeploy the Worker monthly (more moving parts;
  the workflow now also deploys the app)

This is a design decision independent of the data source, and it needs an
answer first.

**2. Elevation data must be reachable from the CI job.** `bake_terrain`
fuses elevation and ground channels (`encode_terrain(ele, ground)`) — no
partial re-bake. A monthly terrain bake needs `esa_elevation_raster`
populated in whatever DB the job uses. Keeping the throwaway-container
model means adding a job step that fetches the GEDTM30 Korea-bbox COG and
`raster2pgsql`s it (a few hundred MB — plausibly 2–5 min, needs a spike to
confirm it's not a blocker).

## Proposed sequencing

1. **Decide the tile publishing convention** (immutable + monthly redeploy
   vs mutable-in-place) — blocks everything downstream, source-independent.
2. **GEDTM30 elevation swap** — data reload, no code change; confirm the
   DSM-noise artifact is real first.
3. **Overture feature migration** — as its own effort: `osm_policy.ts`
   rewrite + coastline mask decision; bake a parallel `tiles/v2` and diff
   page-by-page against `v1`, cut over only when it looks right.
4. **Join `overture-monthly-refresh.yml`** — add the Overture feature-theme
   ingest steps and the GEDTM30 COG fetch step; retire the manual
   `land_pages.json` re-upload note.

## Open questions

- Does Overture's Korea coverage for the OSM-derived themes
  (Buildings `class`/`num_floors`, Base `land_use` `class`) carry enough
  populated attributes to bake against? Places density was ~5x OSM, but
  that's the non-OSM-derived theme — Buildings/Base inherit OSM Korea's
  known thinness. Needs a Gangnam/Suwon spot-check like the one Places got.
- Terrain bake runtime in a GitHub-hosted runner — the manifest has the
  land-tile count; check it against the 6h job limit (POI already bakes
  691k places + 35k geocode pages in the same workflow, so likely fine).
- Is the hybrid coastline (option b) an acceptable permanent state, or only
  a migration stepping-stone?

Sources: existing pipeline — `apps/api/src/bake/terrain/`,
`infra/geo/`, `.github/workflows/overture-monthly-refresh.yml`,
`.agents/skills/bake-tiles/SKILL.md` ·
[20260908-overture-poi-adoption.md](./20260908-overture-poi-adoption.md) ·
[20260816-deployment-hosting.md](./20260816-deployment-hosting.md)

## Resolved (2026-09-09)

Migrated. The coastline gotcha's option (a) is what shipped, not the hybrid
in option (b): `overture_base_water`'s `ocean` subtype, inverted per page
(`OVERTURE_COAST_SQL` in `apps/api/src/bake/terrain/repository.ts`) — no
OSM table anywhere in the pipeline. `osm_policy.ts` became `coast_policy.ts`
(`apps/api/src/bake/terrain/pipeline/`); Buildings/Transportation/Base map
through `overture_policy.ts` in the same directory. Elevation is GEDTM30,
queried as `elevation_raster` (`infra/README.md`). Tile publishing kept its
immutable-versioned convention (`PUBLISHED_VERSION.terrain` in
`packages/pxl/shared/src/bake/published.ts`, read by both the bake tools and
the client); the monthly workflow join (sequencing
step 4) has not happened — terrain re-bakes are still manual.
