# POI (world-anchored-billboard tier)

A minimal, first-pass POI revival — see
[discussions/20260907-world-rendering-tiers.md](../../../../../../discussions/20260907-world-rendering-tiers.md)
for the full reasoning this follows, and its "If/when POI comes back"
section specifically for why POI is wired the way it is here.

- **`store.ts`** — page-keyed POI cache, `providers/overture_provider.ts`'s
  own output per L3 page (server side, below). Deliberately simpler than
  `terrain/store.ts` (no interests registry) since `poi_layer.tsx` is the
  only reader today.
- **`repository.ts`** / **`use_poi_fetcher.ts`** — client transport + demand
  scheduling, mirroring `terrain/tile_repository.ts` / `use_tile_fetcher.ts`'s
  shape: a pre-baked static origin when one is configured
  (`HAS_STATIC_POI`/`POI_TILE_BASE_URL`, `lib/env/config.ts`), the live API
  otherwise, and reusing terrain's own `CLEANUP_RADIUS` to evict cached
  pages once they fall out of range (see `use_poi_fetcher.ts`'s own doc
  comment for why this matters now that POI is Overture-sourced). POI still
  stays off _terrain's own_ tile format — its static set is a sibling
  prefix in the same bucket (`poi/<version>/…`, not `<version>/…`), so
  shipping a POI re-bake never touches terrain's own version or requires
  re-uploading it (see the API side below).
- **`poi_layer.tsx`** — the render: one `SpriteBillboard` (`../billboard/`)
  per POI, anchored via `AnchorNode` and grounded to the real terrain
  surface (a POI's own baked `center.z` is always 0 — see `poi_anchor`'s own
  doc comment). Selection is the nearest `MAX_RENDERED` POIs to `map_center`,
  globally (deduped by id — a feature near a page boundary can come back
  from more than one page's own bbox query), not a per-page rank cut. Size/
  opacity are driven by `../billboard/use_distance_fade.ts`, shared with
  `chat_bubble.tsx` (font size, world-unit scale, and the fade curve all
  live in `billboard/canvas_scale.ts`/`distance_fade.ts` so the two stay
  matched by construction). `select_nearby_pois`'s own distance-to-
  `map_center` cut is a separate, deliberately different distance from the
  camera-distance fade — see that function's own comment.
- **`poi_theme.ts`** — category -> icon/color, drawn as the badge's icon
  circle. Keys are still OSM-tag-shaped strings (`amenity`/`shop`/`tourism`
  vocabulary) even though POI's own data source is Overture Places now —
  `OVERTURE_CATEGORY_MAP` (server side, below) translates Overture's own
  `categories.primary` values into this same vocabulary, so this table
  didn't need to change shape when the source underneath it did.

**Server side**: `apps/api/src/modules/world/poi/{provider.ts,providers/overture_provider.ts,service.ts}`,
registered in `apps/api/src/modules/world/mod.ts`. `provider.ts`'s
`PoiProvider` interface (2026-09-07) is the seam behind which POI's data
source can be swapped — this replaced the original `osm_provider.ts`
(2026-09-08, see discussions/20260908-overture-poi-adoption.md): Overture
Places is CDLA-Permissive-2.0 (not OSM-derived, none of OSM's ODbL
share-alike or Google/Naver/Kakao's display restrictions) and measured
~5x denser than OSM in real Korean bboxes.
`providers/overture_provider.ts` reads `overture_places` (a table
`apps/api/src/bake/overture/tools/ingest_places.ts` populates from
Overture's own public S3 release, refreshed on a monthly GitHub Actions
cadence), mapping `categories.primary` through `OVERTURE_CATEGORY_MAP`
(`apps/api/src/bake/overture/model/category_map.ts`) into `poi_theme.ts`'s
own vocabulary above. The per-page query is uncapped on purpose — a dense
urban page can return thousands of rows, and that's real data, not
something to silently drop server-side; see `poi_layer.tsx`'s own
`select_nearby_pois` for where the actual rendering budget is enforced,
client-side, without discarding anything from the cache itself.

**Static bake**: `apps/api/src/bake/poi/tools/bake_poi_tiles.ts` reuses
terrain's own `land_pages.json` classification (no separate "does this page
have POI" pre-pass — that would cost about as much as just fetching), calls
the same `PoiProvider` the live endpoint does, and skips writing an object
for a page with none (a local-only `.empty` marker instead, so a resume
doesn't re-query it — never uploaded). No shared "everything's the same"
fallback object exists the way terrain's `_ocean` tile does: a missing POI
key just means `[]`, directly. Published to R2 (`poi/v2/*`) and live in
production (`vite.config.ts`'s own `mode === "production"` fallback
resolves `poiTileBase` to `/tiles/poi/v2` without needing
`VITE_POI_TILE_BASE_URL` set at all), refreshed on a monthly GitHub
Actions cadence (`.github/workflows/overture-monthly-refresh.yml`).

## Design reference

`checkpoint/20260107-float` (a surviving checkpoint branch, `git show
origin/checkpoint/20260107-float:apps/web/src/entities/geo/ui/pois/...`) has
the pre-DoD POI implementation this one partially revives:
`poi-theme-config.ts` (this dir's `poi_theme.ts` ports its category
icon/color table, minus the day/night variants), `poi-item.tsx` (the badge
shape — icon circle + name pill — this file's `build_poi_badge_canvas`
redraws as one canvas texture instead of a DOM element, per the tier-3
decision in the discussion doc above), and `poi-manager.tsx` (a genuine
two-tier LOD: the _entire_ loaded set — 10,000+ — rendered as a cheap
custom-shader instanced-dot background layer, with only the nearest ~20
promoted to the expensive per-item badge, re-sorted every 0.2s). That last
piece — a background layer showing "there's more here" for everything not
in the promoted set — is the real next step if `MAX_RENDERED` alone stops
being enough; it was deliberately not ported this pass (see below).

## Deliberately not done yet

- No click-to-select (`map/store.ts`'s `selected_poi` exists, nothing writes
  to it), no hover state (the old badge's lift/glow/expand-on-hover needs
  raycasting against sprites, not wired here).
- No day/night theme variant on the badge itself.
- No background "everything else" layer — `MAX_RENDERED` in `poi_layer.tsx`
  is a flat global cap (nearest-first), not the old two-tier LOD described
  above. Fine at today's scale; revisit if it stops being fine.
