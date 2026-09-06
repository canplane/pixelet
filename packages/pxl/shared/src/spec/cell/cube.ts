// @pxl/shared/spec/cell/cube.ts
// [Layer: cube] The one numeric-id/color catalog for every placeable cell
// type — both the baked 2.5D terrain raster's own four channels
// (land/water/structure/transport, filled by the bake pipeline, one
// independent value per channel per cell) and the flat "block" colors a
// user paints directly. These used to live in two near-identical files
// (terrain.ts + objects.ts); there's no real reason for that (2026-09-06) —
// every channel below is just a Record<name, number>, same shape, whether
// the bake pipeline treats it as real map data or a user-placed palette.
// mod.ts is the one place a placed cube's own single sparse-tree byte gets
// derived FROM these channels (banding land/water/structure/transport into
// disjoint ranges, since a real terrain cell can hold a value from every
// channel at once but a placed cube is exactly one of them) — nothing here
// needs to know about that; this file stays pure cell-definition data.
//
// common.ts used to hold VOID/UNKNOWN as their own separate file/export —
// folded in here as cube_types.common (2026-09-06). Unlike an earlier pass
// at this same merge, land/water/structure/transport/block do NOT spread
// common's keys into their own — common is a shared *registry* of sentinel
// values (VOID: nothing painted; AIR: explicit object-layer erase; UNKNOWN:
// missing/invalid data), not something every channel actually needs. Only
// land/water/structure/transport ever really see VOID/UNKNOWN (real raw
// bytes a baked raster can legitimately hold); AIR only ever means anything
// within the object-placement encoding (mod.ts), never as a raw terrain
// value — spreading it into land/water/etc anyway would've been a sentinel
// that channel can never actually produce, present only for symmetry.
// shader_meta.ts's get_color_meta is the one place that still needs to
// treat a channel's real keys and common's shared ones as one set (for
// VOID/UNKNOWN specifically) — it merges the two explicitly there instead
// of relying on the data itself already being merged.

import { build_colors_from_json } from "./type.ts";

// One shared palette module for every channel below (2026-09-06) — mirrors
// cube_types' own merge of terrain.ts + objects.ts into one file. A plain
// .ts export, not a real .json file imported via `with { type: "json" }`:
// see that module's own top comment for why (a real, reproduced
// @deno/vite-plugin limitation, not a style preference).
import cube_palette_json from "@pxl/shared/assets/cell/palette/cube.ts";

export const cube_types = {
  common: {
    /** unset — no data painted at this cell for this channel; also the
     * object-placement sparse tree's own absence marker (a cell with
     * genuinely no entry at all reads back as this same 0, since a sparse
     * tree's own "nothing stored here" and a dense raster's own "nothing
     * painted here" are the same concept either way). */
    VOID: 0x00,
    /** explicit object-layer air: retain this leaf, erase lower layers —
     * a real, present leaf, unlike VOID above. Only ever a real value
     * within the object-placement encoding (mod.ts) — no baked terrain
     * channel below can ever actually hold this byte, it's registered
     * here purely as a shared constant. */
    AIR: 0x80,
    /** missing / invalid / unresolved data */
    UNKNOWN: 0xff,
  },

  // ---- land ----

  land: {
    /** bare ground */
    BARE: 1,
    /** sea water (MSB-partitioned) */
    SEA: 1 | 0x80,

    // ---- natural ----

    /** deep green / forest / mountain */
    GREEN_DEEP: 10,
    /** light green / park / grass */
    GREEN_LIGHT: 11,
    /** sand / rock */
    SAND: 12,

    // ---- landuse ----

    /** residential area */
    RESIDENTIAL: 20,
    /** commercial area */
    COMMERCIAL: 21,
    /** industrial area */
    INDUSTRIAL: 22,
    /** sports area */
    SPORTS: 23,
    /** education area */
    EDUCATION: 24,
    /** medical area */
    MEDICAL: 25,
    /** public area */
    PUBLIC: 26,

    // ---- aeroway areas ----

    /** apron */
    APRON: 30,
    /** helipad */
    HELIPAD: 31,
  },

  // ---- water ----

  water: {
    /** flowing water */
    RIVER: 1,
    /** still water */
    LAKE: 2,
    /** wetland */
    WETLAND: 3,
  },

  // ---- structure ----

  structure: {
    // ---- buildings ----

    /** generic building */
    BUILDING: 1,
    /** station / terminal building */
    BUILDING_STATION: 2,
    /** education building */
    BUILDING_EDUCATION: 3,
    /** medical building */
    BUILDING_MEDICAL: 4,
    /** public building */
    BUILDING_PUBLIC: 5,
  },

  // ---- transport ----

  transport: {
    // ---- roads ----

    /** default road */
    ROAD_DEFAULT: 1,
    /** motorway */
    ROAD_MOTORWAY: 2,
    /** primary road */
    ROAD_PRIMARY: 3,
    /** secondary road */
    ROAD_SECONDARY: 4,
    /** pedestrian path */
    PEDESTRIAN: 5,
    /** parking area */
    PARKING: 6,

    // ---- rails ----

    /** main rail */
    RAIL_MAIN: 10,
    /** subway rail */
    RAIL_SUBWAY: 11,

    // ---- aeroway lines ----

    /** runway */
    RUNWAY: 20,
    /** taxiway */
    TAXIWAY: 21,
  },

  // ---- block ----
  // Flat user-painted colors only — no sentinels of its own anymore.
  // cube_types.common.VOID (sparse-tree absence) and .AIR (explicit erase)
  // already cover every non-color case a placed cube's own byte can mean;
  // every real key here is a real, pickable color.

  block: {
    white: 130,
    grey: 131,
    dark_grey: 132,
    black: 133,
    red: 134,
    orange: 135,
    brown: 136,
    yellow: 137,
    green: 138,
    dark_green: 139,
    cyan: 140,
    blue: 141,
    violet: 142,
    magenta: 143,
    pink: 144,
  },
} as const satisfies Record<string, Record<string, number>>;

// ============================================================================
// Color Definitions — loaded from JSON (see ../../assets/cell/palette/)
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
const RASTER_SEED: Record<number, string | null> = {
  [cube_types.common.VOID]: null,
  [cube_types.common.UNKNOWN]: "#D9D0CC",
};

// build_colors_from_json processes every channel present in the JSON it's
// handed, regardless of what `types` restricts to — so the raster call
// below needs cube_palette_json's own "block" section stripped first, or
// it'd throw "unknown channel block" against a `types` that only lists
// land/water/structure/transport.
const { block: block_palette_json, ...raster_palette_json } = cube_palette_json;

export const cube_colors: Record<string, Record<number, string | null>> = {
  common: {
    [cube_types.common.VOID]: null,
    [cube_types.common.AIR]: null,
    [cube_types.common.UNKNOWN]: "#D9D0CC",
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
