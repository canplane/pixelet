# POI (world-anchored-billboard tier)

A minimal, first-pass POI revival — see
[discussions/20260907-world-rendering-tiers.md](../../../../../../discussions/20260907-world-rendering-tiers.md)
for the full reasoning this follows, and its "If/when POI comes back"
section specifically for why POI is wired the way it is here.

- **`store.ts`** — page-keyed POI cache, `poi_extractor.ts`'s own output per
  L3 page. Deliberately simpler than `terrain/store.ts` (no interests
  registry) since `poi_layer.tsx` is the only reader today.
- **`repository.ts`** / **`use_poi_fetcher.ts`** — client transport + demand
  scheduling, mirroring `terrain/tile_repository.ts` / `use_tile_fetcher.ts`'s
  shape: a pre-baked static origin when one is configured
  (`HAS_STATIC_POI`/`POI_TILE_BASE_URL`, `lib/env/config.ts`), the live API
  otherwise. POI still stays off *terrain's own* tile format — its static
  set is a sibling prefix in the same bucket (`poi/<version>/…`, not
  `<version>/…`), so shipping a POI re-bake never touches terrain's own
  version or requires re-uploading it (see the API side below).
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
- **`poi_theme.ts`** — category (OSM `amenity`/`shop`/`tourism` tag value) ->
  icon/color, drawn as the badge's icon circle.

**Server side**: `apps/api/src/modules/world/poi/{provider.ts,providers/osm_provider.ts,service.ts}`,
registered in `apps/api/src/modules/world/mod.ts`. `provider.ts`'s
`PoiProvider` interface (2026-09-07) is the seam behind which POI's data
source can be swapped — OSM's own re-bake cadence is a property of
`osm_provider.ts` specifically, not of POI as a concept, so a future
commercial provider (Naver/Kakao Local Search) with a live-query freshness
model instead of a periodic re-bake slots in without touching `service.ts`
or the bake tool. `osm_provider.ts`'s own doc comment explains why it reuses
`TerrainRepository.fetch_feat_rows` and `osm_normalizer.ts`'s `normalize`
directly rather than routing through `bake.ts`'s `bake_terrain`/`fetch_tile`.

**Static bake**: `apps/api/src/bake/poi/tools/bake_poi_tiles.ts` reuses
terrain's own `land_pages.json` classification (no separate "does this page
have POI" pre-pass — that would cost about as much as just fetching), calls
the same `PoiProvider` the live endpoint does, and skips writing an object
for a page with none (a local-only `.empty` marker instead, so a resume
doesn't re-query it — never uploaded). No shared "everything's the same"
fallback object exists the way terrain's `_ocean` tile does: a missing POI
key just means `[]`, directly.

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
- No cache eviction in `store.ts`/`use_poi_fetcher.ts` (terrain's own
  `CLEANUP_RADIUS`) — POI payloads are tiny JSON, not a real memory concern
  yet at single-session scale.
- The static POI set (`bake/poi/tools/bake_poi_tiles.ts`) is built but not
  yet published anywhere — no re-bake cadence has been decided (see
  `provider.ts`'s own doc comment on why that's a provider-specific
  question, not a POI-wide one), and `VITE_POI_TILE_BASE_URL` is unset in
  every deployed environment today, so production still resolves every POI
  fetch against a live API that isn't deployed there either (an existing
  gap, not a new one this pass introduces — see the discussion doc).
