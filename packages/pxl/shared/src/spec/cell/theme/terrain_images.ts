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

// "cozy" pack — original 64x64 art (cozy pastel fantasy MMO / gentle
// life-sim mood references only, nothing copied from any existing game),
// downloaded and normalized by ./tools/download_texture_pack.ts from
// pixelet_ms2_texture_manifest.json at the repo root. Deliberately the
// opposite of simplepixel's bank-of-shared-materials approach just below —
// see GROUND_LAYER_IMAGE_MAP_COZY's own comment for why every type here
// gets its own distinct file instead.
import cozy_land_bare from "./textures/terrain/cozy/land_bare.png";
import cozy_land_sea from "./textures/terrain/cozy/land_sea.png";
import cozy_land_green_deep from "./textures/terrain/cozy/land_green_deep.png";
import cozy_land_green_light from "./textures/terrain/cozy/land_green_light.png";
import cozy_land_sand from "./textures/terrain/cozy/land_sand.png";
import cozy_land_residential from "./textures/terrain/cozy/land_residential.png";
import cozy_land_commercial from "./textures/terrain/cozy/land_commercial.png";
import cozy_land_industrial from "./textures/terrain/cozy/land_industrial.png";
import cozy_land_sports from "./textures/terrain/cozy/land_sports.png";
import cozy_land_education from "./textures/terrain/cozy/land_education.png";
import cozy_land_medical from "./textures/terrain/cozy/land_medical.png";
import cozy_land_public from "./textures/terrain/cozy/land_public.png";
import cozy_land_apron from "./textures/terrain/cozy/land_apron.png";
import cozy_land_helipad from "./textures/terrain/cozy/land_helipad.png";
import cozy_water_river from "./textures/terrain/cozy/water_river.png";
import cozy_water_lake from "./textures/terrain/cozy/water_lake.png";
import cozy_water_wetland from "./textures/terrain/cozy/water_wetland.png";
import cozy_structure_building from "./textures/terrain/cozy/structure_building.png";
import cozy_structure_building_station from "./textures/terrain/cozy/structure_building_station.png";
import cozy_structure_building_medical from "./textures/terrain/cozy/structure_building_medical.png";
import cozy_structure_building_education from "./textures/terrain/cozy/structure_building_education.png";
import cozy_structure_building_public from "./textures/terrain/cozy/structure_building_public.png";
import cozy_transport_road_default from "./textures/terrain/cozy/transport_road_default.png";
import cozy_transport_road_motorway from "./textures/terrain/cozy/transport_road_motorway.png";
import cozy_transport_road_primary from "./textures/terrain/cozy/transport_road_primary.png";
import cozy_transport_road_secondary from "./textures/terrain/cozy/transport_road_secondary.png";
import cozy_transport_pedestrian from "./textures/terrain/cozy/transport_pedestrian.png";
import cozy_transport_parking from "./textures/terrain/cozy/transport_parking.png";
import cozy_transport_rail_main from "./textures/terrain/cozy/transport_rail_main.png";
import cozy_transport_rail_subway from "./textures/terrain/cozy/transport_rail_subway.png";
import cozy_transport_runway from "./textures/terrain/cozy/transport_runway.png";
import cozy_transport_taxiway from "./textures/terrain/cozy/transport_taxiway.png";
import cozy_side_bare from "./textures/terrain/cozy/side_bare.png";
import cozy_side_green_deep from "./textures/terrain/cozy/side_green_deep.png";
import cozy_side_green_light from "./textures/terrain/cozy/side_green_light.png";
import cozy_side_sports from "./textures/terrain/cozy/side_sports.png";
import cozy_side_sand from "./textures/terrain/cozy/side_sand.png";
import cozy_side_residential from "./textures/terrain/cozy/side_residential.png";
import cozy_side_commercial from "./textures/terrain/cozy/side_commercial.png";
import cozy_side_industrial from "./textures/terrain/cozy/side_industrial.png";
import cozy_side_education from "./textures/terrain/cozy/side_education.png";
import cozy_side_medical from "./textures/terrain/cozy/side_medical.png";
import cozy_side_public from "./textures/terrain/cozy/side_public.png";
import cozy_side_apron from "./textures/terrain/cozy/side_apron.png";
import cozy_side_helipad from "./textures/terrain/cozy/side_helipad.png";

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
//
// Kept as a named, non-default pack (see GROUND_LAYER_IMAGE_MAP export at
// the bottom of this file) rather than deleted now that cozy is active —
// still a real, selectable fallback.
export const GROUND_LAYER_IMAGE_MAP_SIMPLEPIXEL: Record<string, Record<string, GroundLayerImageEntry>> = {
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

/** "cozy" pack — every OSM type gets its own distinct 64x64 image, on
 * purpose: unlike simplepixel's small-shared-material-bank approach above,
 * this pack's whole point is that RESIDENTIAL/COMMERCIAL/INDUSTRIAL/SPORTS/
 * EDUCATION/MEDICAL/PUBLIC/APRON/HELIPAD each read as a visually distinct
 * zoning color, every BUILDING_* reads as a distinct structure material,
 * every road/rail tier reads as its own material, and RIVER/LAKE/SEA never
 * share a file even though they're all water. Source of truth for which
 * file backs which type is pixelet_ms2_texture_manifest.json (repo root) —
 * keep this map's keys and that manifest's keys in lockstep. SEA and LAKE
 * are both plain static tiles here (no frame_height) — the manifest
 * mandates real animation stays out of this pack, and the field is
 * optional precisely so a static entry needs nothing extra. */
export const GROUND_LAYER_IMAGE_MAP_COZY: Record<string, Record<string, GroundLayerImageEntry>> = {
  land: {
    BARE: { url: cozy_land_bare },
    SEA: { url: cozy_land_sea },
    GREEN_DEEP: { url: cozy_land_green_deep },
    GREEN_LIGHT: { url: cozy_land_green_light },
    SAND: { url: cozy_land_sand },
    RESIDENTIAL: { url: cozy_land_residential },
    COMMERCIAL: { url: cozy_land_commercial },
    INDUSTRIAL: { url: cozy_land_industrial },
    SPORTS: { url: cozy_land_sports },
    EDUCATION: { url: cozy_land_education },
    MEDICAL: { url: cozy_land_medical },
    PUBLIC: { url: cozy_land_public },
    APRON: { url: cozy_land_apron },
    HELIPAD: { url: cozy_land_helipad },
  },
  water: {
    RIVER: { url: cozy_water_river },
    LAKE: { url: cozy_water_lake },
    WETLAND: { url: cozy_water_wetland },
  },
  structure: {
    BUILDING: { url: cozy_structure_building },
    BUILDING_STATION: { url: cozy_structure_building_station },
    BUILDING_MEDICAL: { url: cozy_structure_building_medical },
    BUILDING_EDUCATION: { url: cozy_structure_building_education },
    BUILDING_PUBLIC: { url: cozy_structure_building_public },
  },
  transport: {
    ROAD_DEFAULT: { url: cozy_transport_road_default },
    ROAD_MOTORWAY: { url: cozy_transport_road_motorway },
    ROAD_PRIMARY: { url: cozy_transport_road_primary },
    ROAD_SECONDARY: { url: cozy_transport_road_secondary },
    PEDESTRIAN: { url: cozy_transport_pedestrian },
    PARKING: { url: cozy_transport_parking },
    RAIL_MAIN: { url: cozy_transport_rail_main },
    RAIL_SUBWAY: { url: cozy_transport_rail_subway },
    RUNWAY: { url: cozy_transport_runway },
    TAXIWAY: { url: cozy_transport_taxiway },
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
const GROUND_LAYER_SIDE_OVERRIDES_SIMPLEPIXEL: Partial<Record<string, Partial<Record<string, GroundLayerImageEntry>>>> = {
  land: {
    BARE: { url: default_dirt },
    GREEN_DEEP: { url: default_dirt },
    GREEN_LIGHT: { url: default_dirt },
    SPORTS: { url: default_dirt },
  },
};

/** cozy's side overrides — 13 dedicated side files from the manifest, one
 * per land type that has its own (BARE/GREEN_DEEP/GREEN_LIGHT/SPORTS plus
 * SAND and every land-use zoning type: RESIDENTIAL/COMMERCIAL/INDUSTRIAL/
 * EDUCATION/MEDICAL/PUBLIC/APRON/HELIPAD), each distinct — never a shared
 * dirt texture the way simplepixel's overrides above are. Every other
 * type (water, structure, transport) falls back to its own top image via
 * build_side_image_map, same as simplepixel.
 *
 * BARE/SPORTS history, since it's easy to reintroduce by "helpfully"
 * matching filenames again: BARE's side used to be #28 (bare exposed
 * soil, no grass cap — a visible seam against BARE's own vivid-green top),
 * while SPORTS had #30 (grass-capped soil) purely because it was next in
 * the old simplepixel override key list (BARE/GREEN_DEEP/GREEN_LIGHT/
 * SPORTS) this manifest's side section originally inherited — never
 * checked against SPORTS's own top (a lighter mint/turf green, distinct
 * from #30's darker grass). Corrected: #30 is BARE's side now; #28 is
 * unused; SPORTS has its own dedicated asset (sourceCreationId
 * d8d53fe2-... in the manifest), generated fresh once the mismatch was
 * found rather than reusing anything else. */
const GROUND_LAYER_SIDE_OVERRIDES_COZY: Partial<Record<string, Partial<Record<string, GroundLayerImageEntry>>>> = {
  land: {
    BARE: { url: cozy_side_bare },
    GREEN_DEEP: { url: cozy_side_green_deep },
    GREEN_LIGHT: { url: cozy_side_green_light },
    SPORTS: { url: cozy_side_sports },
    SAND: { url: cozy_side_sand },
    RESIDENTIAL: { url: cozy_side_residential },
    COMMERCIAL: { url: cozy_side_commercial },
    INDUSTRIAL: { url: cozy_side_industrial },
    EDUCATION: { url: cozy_side_education },
    MEDICAL: { url: cozy_side_medical },
    PUBLIC: { url: cozy_side_public },
    APRON: { url: cozy_side_apron },
    HELIPAD: { url: cozy_side_helipad },
  },
};

/** Merges a side-overrides map onto a copy of `top` — needs to be a full
 * map rather than staying sparse (see the overrides maps' own comments). */
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

export const GROUND_LAYER_SIDE_IMAGE_MAP_SIMPLEPIXEL: Record<string, Record<string, GroundLayerImageEntry>> = build_side_image_map(
  GROUND_LAYER_IMAGE_MAP_SIMPLEPIXEL,
  GROUND_LAYER_SIDE_OVERRIDES_SIMPLEPIXEL,
);

export const GROUND_LAYER_SIDE_IMAGE_MAP_COZY: Record<string, Record<string, GroundLayerImageEntry>> = build_side_image_map(
  GROUND_LAYER_IMAGE_MAP_COZY,
  GROUND_LAYER_SIDE_OVERRIDES_COZY,
);

// Active pack — every real consumer (texture_array.ts, apps/web) imports
// these two names only, same shape either pack provides; swap which named
// pack these point at to change the active pack, same swap-the-active-
// export pattern terrain.ts's own terrain_colors already uses across its
// brick/soft/carto palettes.
export const GROUND_LAYER_IMAGE_MAP: Record<string, Record<string, GroundLayerImageEntry>> = GROUND_LAYER_IMAGE_MAP_COZY;
export const GROUND_LAYER_SIDE_IMAGE_MAP: Record<string, Record<string, GroundLayerImageEntry>> = GROUND_LAYER_SIDE_IMAGE_MAP_COZY;
