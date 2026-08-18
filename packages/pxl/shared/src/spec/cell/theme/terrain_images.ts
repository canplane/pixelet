// @pxl/shared/spec/cell/theme/terrain_images.ts

// CC0 (StarNinjas, "simplepixel") — see ./textures/terrain/simplepixel/
// for the actual files. Lives in theme/ (not apps/web, where this used to
// sit) alongside object_images.ts and both palette/ + textures/ — "theme"
// since palette+textures+these image maps are all visual-presentation
// data, as opposed to the type-numbering/logic ../terrain.ts/../objects.ts
// carry one level up. Same-directory-tree relative imports like this are
// also the only form of asset import this repo's Vite/deno setup actually
// resolves across a package's own files — the @pxl/shared/* bare
// specifier the rest of this codebase uses for .ts/.json works fine, but
// the deno vite plugin's bare-specifier resolver doesn't support non-JS/
// JSON asset imports (confirmed directly: a PNG bare import failed
// resolution outright). Vite still bundles each of these into a real,
// hashed asset URL at build time; the string each import resolves to is
// all load_real_ground_layer_images.ts (tex.ts) needs, since the real
// pixel decode happens in-browser (see its own doc comment for why — no
// PNG-decoding dependency needed anywhere in this pipeline).
import default_dirt from "./textures/terrain/simplepixel/default_dirt.png";
import default_water_source_animated from "./textures/terrain/simplepixel/default_water_source_animated.png";
import default_grass from "./textures/terrain/simplepixel/default_grass.png";
import default_sand from "./textures/terrain/simplepixel/default_sand.png";
import default_moss from "./textures/terrain/simplepixel/default_moss.png";
import default_clay from "./textures/terrain/simplepixel/default_clay.png";
import default_river_water from "./textures/terrain/simplepixel/default_river_water.png";
import default_mossycobble from "./textures/terrain/simplepixel/default_mossycobble.png";
import default_gravel from "./textures/terrain/simplepixel/default_gravel.png";

export type GroundLayerImageEntry = {
  url: string;
  /** Set only for animated sprite sheets (source frames stacked
   * vertically, each frame_height tall, same width as the sheet) — crops
   * just the first frame instead of the whole sheet. Static single-frame
   * textures (the common case) omit this. */
  frame_height?: number;
};

/** ch_key -> type_key (matching terrain_colors.json's own naming) -> which
 * real image that type samples in texture-pack mode. A type with no entry
 * here just keeps its flat-fill placeholder (see tex.ts's
 * load_real_ground_layer_images) — this is additive, not a full mapping
 * requirement. */
// Deliberately a *small* bank of clean, saturated, iconic materials reused
// across many types — not one bespoke real-world-matched material per type.
// A distinct material per type produces a muddy patchwork of
// similar-but-not-quite grayish-browns fighting each other, the opposite of
// vanilla Minecraft's own look: a real Minecraft world is legible precisely
// *because* it's built from a handful of bold materials (grass, stone
// brick, sand, wood, water) repeated everywhere, reinterpreting many
// different real-world categories onto the same block rather than
// inventing a new one for each. Not every terrain type needs its own
// unique material here for the same reason. Raw default_dirt and
// default_stone specifically are avoided as top surfaces (dirt stays
// side-only, see GROUND_LAYER_SIDE_OVERRIDES below) — Minecraft itself
// doesn't lead with bare stone/dirt either; what you actually see above
// ground is grass, worked stone (brick/block), wood, or sand.
export const GROUND_LAYER_IMAGE_MAP: Record<string, Record<string, GroundLayerImageEntry>> = {
  land: {
    // dominant/background land type (whatever OSM leaves untagged) — real-
    // world "land with nothing specific tagged" is almost always just
    // grass/lawn anyway, same reasoning vanilla Minecraft itself uses
    // (ordinary undeveloped ground is grass, not a bespoke "blank land"
    // block).
    BARE: { url: default_grass },
    // 16x256 sheet, 16 animation frames stacked vertically — first frame only.
    SEA: { url: default_water_source_animated, frame_height: 16 },
    GREEN_DEEP: { url: default_moss },
    GREEN_LIGHT: { url: default_moss },
    SAND: { url: default_sand },
    // grass — every artificial land-use zoning category (RESIDENTIAL,
    // COMMERCIAL, INDUSTRIAL, PUBLIC, APRON below) reads as the same plain
    // grass now. None of these OSM landuse tags map to a real distinct
    // ground material the way SAND or the water types do — they're
    // administrative zoning, not terrain — so like BARE, they recede into
    // the same green background instead of getting their own bespoke
    // material.
    RESIDENTIAL: { url: default_grass },
    COMMERCIAL: { url: default_grass },
    INDUSTRIAL: { url: default_grass },
    SPORTS: { url: default_grass },
    EDUCATION: { url: default_grass },
    MEDICAL: { url: default_grass },
    PUBLIC: { url: default_grass },
    APRON: { url: default_grass },
    HELIPAD: { url: default_grass },
  },
  water: {
    RIVER: { url: default_river_water },
    LAKE: { url: default_water_source_animated, frame_height: 16 },
    WETLAND: { url: default_mossycobble },
  },
  structure: {
    // every building type collapsed to plain grass, same reasoning as the
    // land-use zoning types above (RESIDENTIAL/COMMERCIAL/etc.) — none of
    // these read as a visually distinct material worth its own texture.
    BUILDING: { url: default_grass },
    BUILDING_STATION: { url: default_grass },
    BUILDING_MEDICAL: { url: default_grass },
    BUILDING_EDUCATION: { url: default_grass },
    BUILDING_PUBLIC: { url: default_grass },
  },
  transport: {
    // motorway/primary get clay (a real paved-stone material distinct from
    // plain gravel); every other road tier is plain gravel — none of these
    // need their own distinct material from each other.
    ROAD_DEFAULT: { url: default_gravel },
    ROAD_MOTORWAY: { url: default_clay },
    ROAD_PRIMARY: { url: default_clay },
    ROAD_SECONDARY: { url: default_gravel },
    PEDESTRIAN: { url: default_gravel },
    PARKING: { url: default_gravel },
    // rail collapsed to plain grass, same "not every type needs a bespoke
    // material" reasoning as the land/structure types above.
    RAIL_MAIN: { url: default_grass },
    RAIL_SUBWAY: { url: default_grass },
    RUNWAY: { url: default_clay },
    TAXIWAY: { url: default_clay },
  },
};

/** Side-face overrides — vegetation-topped land types (GREEN_DEEP/
 * GREEN_LIGHT/SPORTS) get default_dirt as their side instead of their own
 * top image, matching vanilla Minecraft's own grass-block convention
 * (green top, dirt-brown exposed sides). Alpha-cutout decorative sprites
 * are unsafe here: this shader only ever reads `.rgb` (see shader.ts's
 * sample_ground_layer_color), so a transparent source pixel renders as
 * flat black instead of "see-through" — every image used anywhere in this
 * file has been verified (real browser canvas decode) to have no
 * significant transparency. Every other type's side face is visually
 * identical to its top, so mat.ts derives the *full* per-layer side map by
 * falling back to GROUND_LAYER_IMAGE_MAP's own top entry wherever this
 * sparse map has nothing — see build_side_image_map below. */
const GROUND_LAYER_SIDE_OVERRIDES: Partial<Record<string, Partial<Record<string, GroundLayerImageEntry>>>> = {
  land: {
    BARE: { url: default_dirt },
    GREEN_DEEP: { url: default_dirt },
    GREEN_LIGHT: { url: default_dirt },
    SPORTS: { url: default_dirt },
  },
};

/** Merges GROUND_LAYER_SIDE_OVERRIDES onto a copy of `top` — see the
 * overrides map's own comment for why this needs to be a full map rather
 * than staying sparse. */
function build_side_image_map(
  top: Record<string, Record<string, GroundLayerImageEntry>>,
  overrides: Partial<Record<string, Partial<Record<string, GroundLayerImageEntry>>>>,
): Record<string, Record<string, GroundLayerImageEntry>> {
  const out: Record<string, Record<string, GroundLayerImageEntry>> = {};
  Object.keys(top).forEach((ch_key) => {
    const ch_out: Record<string, GroundLayerImageEntry> = {};
    Object.keys(top[ch_key]).forEach((type_key) => {
      ch_out[type_key] = overrides[ch_key]?.[type_key] ?? top[ch_key][type_key];
    });
    out[ch_key] = ch_out;
  });
  return out;
}

export const GROUND_LAYER_SIDE_IMAGE_MAP: Record<string, Record<string, GroundLayerImageEntry>> = build_side_image_map(
  GROUND_LAYER_IMAGE_MAP,
  GROUND_LAYER_SIDE_OVERRIDES,
);
