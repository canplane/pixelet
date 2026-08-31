// @pxl/shared/spec/cell/terrain.ts
// [Layer: terrain] Tile-loaded 2.5D canvas data

import { common_colors, common_types } from "./common.ts";
import { CELL_MASKS } from "./utils.ts";

import brick_colors_json from "@pxl/shared/assets/cell/palette/terrain/brick.json" with { type: "json" };
import soft_theme_colors_json from "@pxl/shared/assets/cell/palette/terrain/soft.json" with { type: "json" };
import carto_colors_json from "@pxl/shared/assets/cell/palette/terrain/carto.json" with { type: "json" };

// ============================================================================
// Type Definitions
// ============================================================================

export const terrain_types = {
  // ---- land ----

  land: {
    ...common_types,

    // ---- basic ----

    /** bare ground */
    BARE: 1,
    /** sea water (MSB-partitioned) */
    SEA: 1 | CELL_MASKS.SEA_FLAG,

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
    ...common_types,

    /** flowing water */
    RIVER: 1,
    /** still water */
    LAKE: 2,
    /** wetland */
    WETLAND: 3,
  },

  // ---- structure ----

  structure: {
    ...common_types,

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
    ...common_types,

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
} as const;

// ============================================================================
// Color Definitions — loaded from JSON (see ../../assets/cell/palette/)
// ============================================================================
//
// Color values (and the human-readable `note` next to each one) live in
// JSON files instead of inline `as const` object literals so they can be
// edited without touching TypeScript. JSON object keys can't reference
// another module's values the way `[terrain_types.land.BARE]` could, so
// each JSON file keys by the type's own NAME string ("BARE", "SEA", ...)
// instead of its numeric id directly — build_colors_from_json below does
// that name -> type_id resolution once, here, against terrain_types, and
// throws immediately on a typo'd name rather than silently dropping a color.

type ColorJsonEntry = { hex: string; note?: string };
/** Root-level, sits alongside the channel maps — provenance for the palette
 * as a whole (where the colors came from), as opposed to `note` which
 * documents one specific entry. Optional: older/simpler files can omit it. */
type ColorJsonMeta = { description?: string; source?: string };
// index signature has to cover both branches (a plain string-keyed lookup
// can't tell _meta and the channel maps apart by type alone) — narrowed back
// to Record<string, ColorJsonEntry> below, after _meta is skipped.
type ColorJsonFile = { _meta?: ColorJsonMeta; [ch_key: string]: ColorJsonMeta | Record<string, ColorJsonEntry> | undefined };

function build_colors_from_json(json: ColorJsonFile): Record<string, Record<number, string | null>> {
  const out: Record<string, Record<number, string | null>> = {};

  Object.keys(json).forEach((ch_key) => {
    if (ch_key === "_meta") return;

    const types_for_ch = (terrain_types as Record<string, Record<string, number>>)[ch_key];
    if (!types_for_ch) {
      throw new Error(`terrain color JSON references unknown channel "${ch_key}" — check terrain_types`);
    }

    const ch_out: Record<number, string | null> = { ...common_colors };
    const ch_entries = json[ch_key] as Record<string, ColorJsonEntry>;
    Object.entries(ch_entries).forEach(([type_name, entry]) => {
      const type_id = types_for_ch[type_name];
      if (type_id === undefined) {
        throw new Error(`terrain color JSON references unknown type "${ch_key}.${type_name}" — check terrain_types`);
      }
      ch_out[type_id] = entry.hex;
    });

    out[ch_key] = ch_out;
  });

  return out;
}

/** BrickLink-sourced palette — superseded by the OSM-style palette below as
 * the active default, kept as a named backup/reference rather than
 * deleted. */
export const terrain_colors_brick: Record<string, Record<number, string | null>> = build_colors_from_json(
  brick_colors_json,
);

/** The original pastel "Soft World Theme" — also superseded, kept as a
 * named backup/reference (same reasoning as terrain_colors_brick above). */
export const terrain_colors_soft: Record<string, Record<number, string | null>> = build_colors_from_json(
  soft_theme_colors_json,
);

/** Active palette — every terrain-color consumer (shader_meta.ts,
 * minimap_atlas.ts, mod.ts's cell_colors) reads this one. Sourced from
 * OpenStreetMap Carto (openstreetmap.org's own default renderer,
 * gravitystorm/openstreetmap-carto) — "OSM" alone would be ambiguous, since
 * the OSM *data* has no inherent colors; Carto is the specific style whose
 * color choices this file replicates. */
export const terrain_colors: Record<string, Record<number, string | null>> = build_colors_from_json(
  carto_colors_json,
);
