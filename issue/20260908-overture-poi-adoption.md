# POI and reverse-geocoding switch to Overture now; Overture's terrain-adjacent themes and GEDTM30 stay next-generation sources

**2026-09-08, same-day follow-up to the section below:** reverse geocoding
doesn't have a bake pipeline yet at all (this session's own opening
suggestion, never built) — unlike terrain, there's nothing already working
to disrupt by picking Overture as its source from day one. So it's adopting
Overture's **Divisions** theme (administrative boundary polygons) alongside
Places, decided the same day as everything below. This does **not** change
the reasoning for Buildings/Transportation/Base staying out of scope — that
was specifically about not rewriting terrain's own already-built,
already-deployed OSM pipeline for a schema-only convenience. Divisions has
no existing pipeline to disrupt, so there's no such cost to weigh against.

One thing this does _not_ do: Divisions is still OSM-derived inside Overture
(`© OpenStreetMap contributors`, ODbL) — same as Buildings/Transportation.
Building reverse-geocoding against it doesn't get the "OSM-independent"
win Places did; it gets the nicer Parquet/GERS packaging only, same
ODbL terms this project already operates under for terrain. Worth being
precise about since Places' win and Divisions' win are different in kind.

**Separate table per theme, not a shared one, and neither touches
terrain's own data:**

|                   | Source (today→)                                  | Where it's ingested                                                                            | What bakes from it                                                 | Touches terrain? |
| ----------------- | ------------------------------------------------ | ---------------------------------------------------------------------------------------------- | ------------------------------------------------------------------ | ---------------- |
| Terrain           | `osm_elements` + `esa_elevation_raster` (GLO-30) | (unchanged)                                                                                    | Combined custom binary format, already baked and deployed (`v1/*`) | —                |
| POI               | ~~`osm_elements`~~ → **Overture Places**         | New `overture_places` table, same `pixelet_geo` DB, alongside (not merged with) `osm_elements` | JSON per-page (`poi/v1/*`)                                         | No               |
| Reverse geocoding | (none yet) → **Overture Divisions**              | New `overture_divisions` table, same DB                                                        | Not yet built — new pipeline, this session                         | No               |

`overture_provider.ts` and the new geocode bake tool each read their own
table only; nothing added here is visible to or read by the terrain bake at
all — same Postgres instance, structurally unrelated tables.

---

# POI switches to Overture Places now; Overture's other themes and GEDTM30 are next-generation sources, not adopted yet

**2026-09-08** — follow-up to
[20260908-geocode-poi-provider-survey.md](./20260908-geocode-poi-provider-survey.md),
same day: Overture Maps came up mid-survey as a fifth option the original
provider comparison didn't cover, got verified properly, and changed the
POI decision. This records that verdict and what's explicitly _not_
decided yet, so the difference doesn't blur later.

## Verdict: POI's baseline becomes Overture Places, effective now

Checked two ways, both came back clean:

**License.** Overture's Places theme is generated entirely from Meta,
Microsoft, Foursquare, PinMeTo, and a handful of smaller contributors —
[Overture's own attribution page](https://docs.overturemaps.org/attribution/)
states outright that the theme "does not include OpenStreetMap data."
Licensed CDLA-Permissive-2.0 (Foursquare's own slice is Apache-2.0), no
ODbL share-alike anywhere in it. Overture's own
[terms of use](https://overturemaps.org/terms-of-use/) impose no
display-venue or caching restriction at all — nothing like Google's
"must show on a Google Map" or Kakao/Naver/VWorld's "may not store
results." A downloaded Parquet file living in our own S3/R2 bucket forever
is exactly the intended use.

**Measured density.** Downloaded real Overture Places extracts (via the
`overturemaps` Python CLI against the public S3 release bucket) for two
~1 km² Korean bboxes and compared against this app's own `osm_elements`
table for the identical boxes:

| Area                                                          | Overture Places | OSM (amenity/shop/tourism) | Ratio |
| ------------------------------------------------------------- | --------------- | -------------------------- | ----- |
| Gangnam Station                                               | 2,954           | 546                        | 5.4x  |
| Suwon Maetan-dong (a residential/suburban area, not downtown) | 351             | 67                         | 5.2x  |

Consistent ~5x density in both a dense core and a suburban area, not just
one lucky spot — and the names sampled from both are real, plausible
Korean business names (진흥종합상가, 크린토피아 강남진흥점, 일인자감자탕,
BBQ치킨, ...), not junk/duplicate noise. This directly overturns an earlier
guess in this same conversation that Overture's Korea coverage would
inherit OSM Korea's known weakness — that guess assumed Places was
OSM-derived like Overture's other themes; it isn't, and the measurement
shows it.

## Explicitly not adopted yet — next-generation sources, noted for later

Two things came up in the same conversation that are good candidates for a
_future_ migration but are staying out of scope for now (effort vs. payoff
right now doesn't justify it):

- **Overture's Buildings/Transportation/Divisions/Base themes.** Unlike
  Places, these genuinely are OSM-derived (`© OpenStreetMap contributors`,
  ODbL) — Buildings adds Microsoft's ML building-footprint layer and
  Google's Open Buildings on top, Transportation adds TomTom, but OSM is
  still in the mix and the ODbL terms still apply. Switching this app's
  terrain/structure bake to source from Overture instead of raw OSM PBF
  extracts would be a data-pipeline convenience (Parquet + GERS
  cross-theme IDs instead of PBF/XML tag parsing) — it would **not**
  change what license governs the result, since it's the same underlying
  OSM content either way. Not worth a pipeline rewrite on its own; revisit
  if the Buildings theme's own added layers (real building heights, not
  just footprints) become something this project actually wants to
  render.
- **GEDTM30** (OpenGeoHub, peer-reviewed in PeerJ 2025) as a candidate
  replacement for the Copernicus GLO-30 DEM this app's terrain elevation
  currently bakes from. GLO-30 is a _surface_ model (DSM) — building and
  tree canopy height gets baked into the ground elevation raster itself,
  which is a real, plausible source of "terrain looks bumped up under
  dense buildings/forest" artifacts in a pipeline that (like this one)
  treats elevation and structure as separate layers composited together.
  GEDTM30 fuses GLO-30 + ALOS World 3D with real bare-earth returns from
  ICESat-2/GEDI satellite laser altimetry via ML to strip that noise out —
  same 30 m grid, drop-in GeoTIFF, and licensed CC-BY 4.0 (attribution
  only, fully commercial-use-friendly — unlike FABDEM, the older
  bare-earth alternative, which is CC BY-NC-SA and would need a paid
  Fathom license for this project's for-profit use). **Not yet confirmed
  whether the DSM-noise artifact is actually visible in this app's current
  build** — worth a direct look before committing to a full terrain
  re-bake on a new elevation source.

Both are recorded here explicitly, per this session's own instruction, so
"we looked into this and it's a real option" doesn't get lost or
rediscovered from scratch later.

## POI switch: what changes, and the open design question

`PoiProvider` (`apps/api/src/modules/world/poi/provider.ts`) already exists
for exactly this swap — a new `providers/overture_provider.ts` implementing
`fetch_pois(page_idx)` replaces `osm_provider.ts` as `service.ts`'s and
`bake_poi_tiles.ts`'s default. Two real pieces of work this isn't hand-waving
past:

- **Category mapping.** Overture's `categories.primary` taxonomy
  (`korean_restaurant`, `chicken_restaurant`, `dry_cleaning`, ...) is a
  different, larger vocabulary than OSM's `amenity`/`shop`/`tourism` tag
  values `poi_theme.ts` keys off today. Needs its own mapping table, not a
  reuse of the existing one.
- **Ingestion shape.** Mirrors how `osm_elements` already gets populated:
  pull a Korea-bbox Places extract from Overture's public S3 release
  (`overturemaps` CLI or DuckDB against the Parquet release) into a new
  Postgres table once per release, then `overture_provider.ts` queries
  that table by bbox — not a live per-page network call out to Overture's
  bucket during baking.

**Open question this document raises but doesn't resolve: how the monthly
Overture release cadence maps onto R2 serving.** Overture cuts a new
dated release roughly monthly; terrain/POI's existing R2 convention is
"immutable forever, bump the version prefix to invalidate" (no purge API,
`cache-control: immutable`) — which assumes content essentially never
changes. A monthly-refreshing POI set doesn't fit that assumption cleanly:
bumping the version prefix every month means also updating
`VITE_POI_TILE_BASE_URL`'s resolved value and redeploying every month, just
to pick up new data. Two ways to resolve this, not chosen yet:

1. Keep the existing immutable-versioned pattern and accept the monthly
   redeploy as a real recurring step (possibly scripted).
2. Give POI objects their own, shorter cache-control (a day or a week, not
   `immutable`) under a stable prefix, and let the monthly job overwrite
   objects in place — no version bump, no redeploy, at the cost of some
   edge cache efficiency terrain doesn't have to give up. POI's own
   staleness tolerance is much higher than terrain's (a three-week-old
   restaurant listing is a non-issue; three-week-old terrain would be a
   real bug), so this tradeoff reads as the right one, but hasn't been
   decided as a change to the deployment model.

Automation itself (a monthly trigger running ingest → bake → `aws s3 sync`)
is better suited to real scheduled infra (a GitHub Actions cron workflow,
matching how `bake-tiles`'s own manual `deno run` procedure is already
documented) than to this session's own tooling — start as a documented
manual monthly run, wire up real cron once the pipeline itself is proven
once.

## Status

**Implemented, same day.** Both POI and reverse geocoding (the latter a new
pipeline — it had none before) now run on Overture, built and baked
end-to-end locally:

- `apps/api/src/bake/overture/tools/{ingest_places,ingest_divisions}.ts` —
  Korea-wide ingestion into `overture_places` (691,824 rows) and
  `overture_divisions` (4,009 rows, `county`+`locality` only).
- `apps/api/src/modules/world/poi/providers/overture_provider.ts` and the
  new `apps/api/src/modules/world/geocode/` module (provider + live
  service) replace `osm_provider.ts` (deleted) and the direct client-side
  Nominatim call (`use_reverse_geocode.ts`, rewritten to be page-keyed like
  POI instead of per-exact-coordinate).
- Baked results: POI 694,203 places across 22,259 non-empty pages (51.1 MB
  gzip, vs. OSM's 233,193/20,073/23.3 MB); geocode 35,047 labeled pages
  (1.9 MB gzip). Spot-checked (Gangnam Station, Suwon City Hall, Gyeongbokgung)
  against known real labels — correct.
- Client wiring (`config.ts`, `vite.config.ts`, `worker/index.ts`'s
  `KEY_RE`, `.env`/`.env.example`) mirrors POI's own static/live pattern
  exactly, one more `geocode/` sibling prefix.

Shipped later the same day: both baked sets uploaded to R2 and deployed to
production, plus a monthly GitHub Actions workflow
(`.github/workflows/overture-monthly-refresh.yml`, cron `0 3 1 * *`) that
re-runs ingest → bake → `aws s3 sync` against an ephemeral CI Postgres —
the manual procedure above, now automated. Also removed the stale
Nominatim mention from settings' location-notice copy (`en.json`/
`ko.json`) and `infra/geo/README.md`'s attribution table, and added an
Overture Places credit line to `attribution_section.tsx`.

The next-gen items (Overture's terrain-adjacent themes, GEDTM30) are still
deliberately parked, unaffected by today's build.

### Follow-up, same day: reverse geocoding's real granularity gap

Investigating why "서울특별시 종로구 사직동" (3 levels) and "경기도 성남시
분당구" (stuck at 2, missing the dong) looked inconsistent surfaced two
separate real issues, both fixed:

1. **`region` was filtered out at ingestion** (`KEEP_SUBTYPES` originally
   kept only `county`/`locality`) — fixed by including `region` too, and
   Overture's `names.common` turned out to be a genuinely rich per-language
   map at every level (unlike POI's sparse `names.rules[]`) — now captured
   and served, so the client can render labels in the viewer's own UI
   language (`i18n.language`), not just Korean.

2. **The deeper issue**: Overture's Divisions theme has two feature types —
   `division_area` (real boundary polygons) and `division` (the entity's
   own representative point, present even with no boundary mapped).
   Checked nationwide: 21,596 `division`-point rows carry subtype
   `locality` vs. only 3,600 `division_area` polygons at that subtype —
   most of Korea's named localities (백현동/수내동 in Bundang/Pangyo,
   confirmed by name) have NO mapped polygon at all, only a point. Point-
   in-polygon reverse geocoding structurally can't find these. Fixed by
   ingesting `division` points too (`overture_division_points`) and, when
   no fine-subtype polygon covers a point, looking up the nearest division
   point _inside_ whatever coarser polygon was already matched (not a bare
   radius — avoids leaking across a real boundary) and appending it as an
   extra, finer part marked `approx: true`. Confirmed live: 삼평동/백현동-
   area pages now resolve to a real 4-level "경기도 성남시 분당구 X동"
   label instead of stopping at 분당구.

   Known imprecision, not fixed: this can also surface a _parallel_ naming
   variant rather than a genuine finer level — e.g. Jongno's 사직동
   (행정동) got 신문로2가 (a 법정동 covering roughly the same area, not a
   sub-district of it) appended alongside it. Overture's schema doesn't
   distinguish "genuinely nested subdivision" from "alternate naming
   system for the same area," so this can't be resolved without a data
   source that encodes that distinction — left as-is rather than
   suppressed, since the label is still more informative than not.

Sources: [Overture attribution page](https://docs.overturemaps.org/attribution/) · [Overture Places guide](https://docs.overturemaps.org/guides/places/) · [Overture terms of use](https://overturemaps.org/terms-of-use/) · [GEDTM30 (PeerJ, 2025)](https://peerj.com/articles/19673/) · [FABDEM licensing (Bristol)](https://data.bris.ac.uk/data/dataset/s5hqmjcdj8yo2ibzi9b4ew3sn)
