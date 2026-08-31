// @pxl/shared/assets/cell/terrain_images.ts

// Lives alongside its own ./textures/ (and object_images.ts's own
// ./textures/, one level up in this same assets/cell/ tree) rather than
// with spec/cell/terrain.ts's type-numbering/logic — this file's whole
// job is bridging real image assets to typed URLs, so it sits with the
// assets it bridges. Consumers (apps/web's texture_array.ts/tex.ts)
// import this file itself via the @pxl/shared/* bare specifier; only the
// PNG imports below stay relative, since the deno vite plugin's
// bare-specifier resolver doesn't support non-JS/JSON asset imports
// (confirmed directly:
// a PNG bare import failed resolution outright) — bare specifiers work
// fine for this package's own .ts/.json files, just not PNG/GLB. Vite
// still bundles each of these into a real, hashed asset URL at build
// time; the string each import resolves to is all
// load_real_ground_layer_images.ts (tex.ts) needs, since the real pixel
// decode happens in-browser (see its own doc comment for why — no
// PNG-decoding dependency needed anywhere in this pipeline).

// "cozy" pack — original 64x64 art (cozy pastel fantasy MMO / gentle
// life-sim mood references only, nothing copied from any existing game),
// downloaded and normalized by ./tools/download_texture_pack.ts from
// pixelet_ms2_texture_manifest.json at the repo root. Every OSM type gets
// its own distinct file — see GROUND_LAYER_IMAGE_MAP_COZY's own comment.
import cozy_land_bare from "./textures/terrain/land_bare.png";
import cozy_land_sea from "./textures/terrain/land_sea.png";
import cozy_land_green_deep from "./textures/terrain/land_green_deep.png";
import cozy_land_green_light from "./textures/terrain/land_green_light.png";
import cozy_land_sand from "./textures/terrain/land_sand.png";
import cozy_land_residential from "./textures/terrain/land_residential.png";
import cozy_land_commercial from "./textures/terrain/land_commercial.png";
import cozy_land_industrial from "./textures/terrain/land_industrial.png";
import cozy_land_sports from "./textures/terrain/land_sports.png";
import cozy_land_education from "./textures/terrain/land_education.png";
import cozy_land_medical from "./textures/terrain/land_medical.png";
import cozy_land_public from "./textures/terrain/land_public.png";
import cozy_land_apron from "./textures/terrain/land_apron.png";
import cozy_land_helipad from "./textures/terrain/land_helipad.png";
import cozy_water_river from "./textures/terrain/water_river.png";
import cozy_water_lake from "./textures/terrain/water_lake.png";
import cozy_water_wetland from "./textures/terrain/water_wetland.png";
import cozy_structure_building from "./textures/terrain/structure_building.png";
import cozy_structure_building_station from "./textures/terrain/structure_building_station.png";
import cozy_structure_building_medical from "./textures/terrain/structure_building_medical.png";
import cozy_structure_building_education from "./textures/terrain/structure_building_education.png";
import cozy_structure_building_public from "./textures/terrain/structure_building_public.png";
import cozy_transport_road_default from "./textures/terrain/transport_road_default.png";
import cozy_transport_road_motorway from "./textures/terrain/transport_road_motorway.png";
import cozy_transport_road_primary from "./textures/terrain/transport_road_primary.png";
import cozy_transport_road_secondary from "./textures/terrain/transport_road_secondary.png";
import cozy_transport_pedestrian from "./textures/terrain/transport_pedestrian.png";
import cozy_transport_parking from "./textures/terrain/transport_parking.png";
import cozy_transport_rail_main from "./textures/terrain/transport_rail_main.png";
import cozy_transport_rail_subway from "./textures/terrain/transport_rail_subway.png";
import cozy_transport_runway from "./textures/terrain/transport_runway.png";
import cozy_transport_taxiway from "./textures/terrain/transport_taxiway.png";
import cozy_side_bare from "./textures/terrain/side_bare.png";
import cozy_side_green_deep from "./textures/terrain/side_green_deep.png";
import cozy_side_green_light from "./textures/terrain/side_green_light.png";
import cozy_side_sports from "./textures/terrain/side_sports.png";
import cozy_side_sand from "./textures/terrain/side_sand.png";
import cozy_side_residential from "./textures/terrain/side_residential.png";
import cozy_side_commercial from "./textures/terrain/side_commercial.png";
import cozy_side_industrial from "./textures/terrain/side_industrial.png";
import cozy_side_education from "./textures/terrain/side_education.png";
import cozy_side_medical from "./textures/terrain/side_medical.png";
import cozy_side_public from "./textures/terrain/side_public.png";
import cozy_side_apron from "./textures/terrain/side_apron.png";
import cozy_side_helipad from "./textures/terrain/side_helipad.png";

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
 * requirement.
 *
 * Every OSM type gets its own distinct 64x64 image, on purpose:
 * RESIDENTIAL/COMMERCIAL/INDUSTRIAL/SPORTS/EDUCATION/MEDICAL/PUBLIC/APRON/
 * HELIPAD each read as a visually distinct zoning color, every BUILDING_*
 * reads as a distinct structure material, every road/rail tier reads as
 * its own material, and RIVER/LAKE/SEA never share a file even though
 * they're all water. Source of truth for which file backs which type is
 * pixelet_ms2_texture_manifest.json (repo root) — keep this map's keys and
 * that manifest's keys in lockstep. SEA and LAKE are both plain static
 * tiles here (no frame_height) — the manifest mandates real animation
 * stays out of this pack, and the field is optional precisely so a static
 * entry needs nothing extra. */
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

/** cozy's side overrides — 13 dedicated side files from the manifest, one
 * per land type that has its own (BARE/GREEN_DEEP/GREEN_LIGHT/SPORTS plus
 * SAND and every land-use zoning type: RESIDENTIAL/COMMERCIAL/INDUSTRIAL/
 * EDUCATION/MEDICAL/PUBLIC/APRON/HELIPAD), each distinct. Every other type
 * (water, structure, transport) falls back to its own top image via
 * build_side_image_map below — see its own comment for why a full map is
 * needed rather than staying sparse. Alpha-cutout decorative sprites are
 * unsafe here: this shader only ever reads `.rgb` (see shader.ts's
 * sample_ground_layer_color), so a transparent source pixel renders as
 * flat black instead of "see-through" — every image used anywhere in this
 * file has been verified (real browser canvas decode) to have no
 * significant transparency.
 *
 * BARE/SPORTS history, since it's easy to reintroduce by "helpfully"
 * matching filenames again: BARE's side used to be #28 (bare exposed
 * soil, no grass cap — a visible seam against BARE's own vivid-green top),
 * while SPORTS had #30 (grass-capped soil) purely because it was next in
 * an old override key list this manifest's side section originally
 * inherited — never checked against SPORTS's own top (a lighter mint/turf
 * green, distinct from #30's darker grass). Corrected: #30 is BARE's side
 * now; #28 is unused; SPORTS has its own dedicated asset (sourceCreationId
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
