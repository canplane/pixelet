// @pxl/web/catalog/cell/cube.ts
// [Layer: cube] The one numeric-id/color catalog for every placeable cell
// type — both the baked 2.5D terrain raster's own four channels
// (land/water/structure/transport, filled by the bake pipeline, one
// independent value per channel per cell) and the flat "block" colors a
// user paints directly. Every channel is a name-to-number table of the same
// shape, whether the bake pipeline treats it as real map data or a user's
// palette.
//
// `common` is a shared registry of sentinel values — VOID (nothing painted),
// AIR (an explicit object-layer erase) and UNKNOWN (missing or invalid data)
// — and the other channels do not spread its keys into their own. Only
// land/water/structure/transport can hold VOID or UNKNOWN, those being raw
// bytes a baked raster legitimately produces; AIR means something only
// within the object-placement encoding (mod.ts), never as a terrain value,
// so a channel that can never produce it does not list it.
//
// mod.ts is where a placed cube's own value is derived from these channels,
// banding the terrain channels into disjoint ranges — a real terrain cell
// can hold one value from every channel at once, where a placed cube is
// exactly one of them. Nothing here needs to know that; this file is cell
// definitions and nothing else.

import { build_colors_from_json, type Swatch } from "./type.ts";

// One shared palette module for every channel below (2026-09-06) — mirrors
// cube_types' own merge of terrain.ts + objects.ts into one file. A plain
// .ts export, not a real .json file imported via `with { type: "json" }`:
// see that module's own top comment for why (a real, reproduced
// @deno/vite-plugin limitation, not a style preference).
import cube_palette_json from "./color_spec.ts";

import { cube_types } from "./cube_type_spec.ts";

export { cube_types };

// ============================================================================
// Color Definitions — the palette's colour column (see ./color.ts)
// ============================================================================

/** VOID/UNKNOWN's own shared colors, seeded into every raster channel below
 * (land/water/structure/transport) so a real cell holding either sentinel
 * still resolves to *some* real color — VOID to nothing (null, the same
 * "don't render this" signal build_colors_from_json already uses
 * elsewhere) and UNKNOWN to a visible neutral gray, so an unresolved cell
 * reads as "we don't know" instead of silently showing as whatever
 * happened to render last. AIR is deliberately not included: no raster
 * channel can ever actually hold that byte (see cube_types' own top
 * comment), so seeding a color for it here would be dead data. */
const RASTER_SEED: Record<number, Swatch | null> = {
  [cube_types.common.void]: null,
  [cube_types.common.unknown]: { hex: "#D9D0CC", rgb: 0xd9d0cc },
};

// build_colors_from_json processes every channel present in the JSON it's
// handed, regardless of what `types` restricts to — so the raster call
// below needs cube_palette_json's own "block" section stripped first, or
// it'd throw "unknown channel block" against a `types` that only lists
// land/water/structure/transport.
const { block: block_palette_json, ...raster_palette_json } = cube_palette_json;

export const cube_colors: Record<string, Record<number, Swatch | null>> = {
  common: {
    [cube_types.common.void]: null,
    [cube_types.common.air]: null,
    [cube_types.common.unknown]: { hex: "#D9D0CC", rgb: 0xd9d0cc },
  },
  ...build_colors_from_json(
    { land: cube_types.land, water: cube_types.water, structure: cube_types.structure, transport: cube_types.transport },
    raster_palette_json,
    RASTER_SEED,
  ),
  // No seed at all for block — every one of its keys is a real color
  // straight from the palette file, nothing to pre-populate.
  ...build_colors_from_json({ block: cube_types.block }, { block: block_palette_json }, {}),
};

/** A real, user-paintable flat color — every key of cube_types.block, no
 * exclusion needed (block carries no sentinels of its own any more). */
export type CubeColorId = keyof typeof cube_types.block;
