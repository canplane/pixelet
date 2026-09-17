# POI (world-anchored-billboard tier)

Places as badges over the terrain — one of the rendering tiers in
[issue/20260907-world-rendering-tiers.md](/issue/20260907-world-rendering-tiers.md),
rebuilt on 2026-09-12/13 into the shape below.

- **`store.rs`** — the resident pages as a `PageTable` (`../page_table.rs`):
  an `IndexedMap` of page keys in the core memory, each holding a
  `ResidentPoiPage` and the pool block its lanes were copied into. A page is
  the XTP record the API emits (`pxl_shared::bake::format`'s
  `encode_poi_page`): x, y, z, category, rank and the id hash as lanes, the
  strings as `str` leaves read one at a time.
- **`repository.rs`** / **`use_poi_fetcher.rs`** — transport and demand,
  mirroring terrain's: a `Demand` per page through the fetch scheduler, the
  published static prefix when configured (`poi/<PUBLISHED_VERSION.poi>/…`
  from `pxl_shared::bake::published`, a sibling of terrain's own prefix
  in the same bucket), the live API otherwise.
- **`poi_index.rs`** — a spatial hash over the disc around the camera, as
  lanes: position, cell, page and record, grounded against the terrain at
  build time. Rebuilt a range of records per unit in the frame's content
  phase while the previous build keeps answering; swapped whole.
- **`poi_layer.rs`** — placement as a job of the content phase: query the
  index, file the hits into an implicit lattice (the tier follows the
  camera distance, on the cube ladder), offer the nodes with estimated
  widths, greedy placement on a screen grid in distance order (a covered
  badge is absorbed into the one standing on its pixels, which becomes a
  count), build the badges, commit the field whole. Throttled to one run
  per `PLACEMENT_MS` while the inputs change, and once after they stop.
- **`poi_badge.rs`** — the badge bitmaps, drawn into one atlas
  (`../billboard/atlas.rs`) and cached by id hash; the width a candidate
  offers is estimated from its characters, the badge measured exactly when
  drawn.
- **`/crates/pxl/core/src/world/billboard.rs` and `/crates/pxl/render/src/gpu/badges.rs`** —
  every badge as one instanced quad: anchor, size and sheet rectangle per
  instance; fit, fade and the occlusion twin in the vertex shader
  (`/crates/pxl/render/src/shader/badge.wgsl`).
- **`poi_theme.rs`** — category → icon/colour. `OVERTURE_CATEGORY_MAP`
  (`/apps/api/src/bake/overture/model/category_map.ts`) translates Overture's
  `categories.primary` into this vocabulary.

**Server side**: `/apps/api/src/bake/poi/{provider.ts,provider/overture_provider.ts}`
and `/apps/api/src/module/tile/poi/service.ts` (the dev-only live
endpoint). `PoiProvider` is the seam behind which the data source can be
swapped; Overture Places is CDLA-Permissive-2.0. The per-page query is
uncapped on purpose — a dense page is thousands of rows, and the rendering
budget is enforced client-side by the placement, not by dropping data.

**Static bake**: `/apps/api/src/bake/poi/tool/bake_poi_tiles.ts` writes a
packet per page (`repack_poi_tiles.ts` re-encodes an earlier JSON bake),
published to R2 under that prefix and refreshed monthly
(`/.github/workflows/overture-monthly-refresh.yml`).

## Not done

- No click-to-select and no hover state.
- No day/night variant on the badge.
- A glyph atlas in place of a badge atlas — the sheet is bounded by the
  data today, and a glyph sheet would be bounded by the script; it belongs
  after instancing, which is now in, and before a much denser dataset.
