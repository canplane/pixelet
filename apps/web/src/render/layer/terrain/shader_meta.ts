// @pxl/web/render/layer/terrain/shader_meta.ts

import type { u32, u8 } from "@xpute/core/kit/word.ts";
import type { U8Array } from "@xpute/core/kit/array.ts";

import { cube_colors, cube_types } from "@pxl/shared/spec/cell/cube.ts";
import { GROUND_CH_KEYS, GroundChannelKey } from "@pxl/shared/bake/format.ts";

export const KEYS = <T extends object>(v: T): Array<keyof T> => Object.keys(v) as Array<keyof T>;

export const IS_VALID_HEX = (hex: string): boolean => /^#([A-Fa-f0-9]{3,4}|[A-Fa-f0-9]{6}|[A-Fa-f0-9]{8})$/.test(hex);

export const gen = {
  // shader local variable
  CH: (ch_key: GroundChannelKey): string => `ch_${ch_key}`,

  // texture uniform
  U_CH: (ch_key: GroundChannelKey): string => `u_ch_${ch_key}`,

  // color uniform
  U_COLOR: (ch_key: GroundChannelKey, type_key: string): string => `u_color_ch_${ch_key}_${type_key}`,

  FUNC_COLOR: (ch_key: GroundChannelKey): string => `get_color_ch_${ch_key}`,

  // layer-array index lookup (see get_layer_meta/build_ground_layer_resolver below)
  FUNC_LAYER: (ch_key: GroundChannelKey): string => `get_layer_ch_${ch_key}`,
};

export type TerrainColorMeta = {
  ch_key: GroundChannelKey;
  type_key: string;
  type_id: u8;
  uniform_name: string;
  hex_color: string;
};
export const get_color_meta = (): TerrainColorMeta[] => {
  const out: TerrainColorMeta[] = [];

  GROUND_CH_KEYS.forEach((ch_key) => {
    // cube_types[ch_key] (land/water/structure/transport) carries only its
    // own real types — VOID/AIR/UNKNOWN live in cube_types.common instead
    // (cube.ts's own top comment) and aren't spread in, so this channel's
    // own real cell values wouldn't otherwise include them. Merged in only
    // for this iteration, not the data itself: AIR never actually resolves
    // to a color for any of these channels (no seed defines one, see
    // cube.ts's own RASTER_SEED comment), so it's filtered out below same
    // as any other type with no real color; VOID and UNKNOWN do.
    const types = { ...cube_types.common, ...cube_types[ch_key] };
    const colors = cube_colors[ch_key];
    KEYS(types).forEach((type_key) => {
      const type_id = types[type_key];
      const hex_color = colors[type_id];
      if (hex_color && IS_VALID_HEX(hex_color)) {
        out.push({
          ch_key,
          type_key,
          type_id,
          uniform_name: gen.U_COLOR(ch_key, type_key),
          hex_color,
        });
      }
    });
  });

  return out;
};

export const build_color_mappings = (meta: TerrainColorMeta[]): {
  color_uniforms_decl: string;
  color_mapping_funcs: string;
} => {
  let color_uniforms_decl = meta
    .map((x) => `uniform vec3 ${x.uniform_name};`)
    .join("\n  ");
  if (color_uniforms_decl) color_uniforms_decl += "\n  ";

  let color_mapping_funcs = "";
  GROUND_CH_KEYS.forEach((ch_key) => {
    const cases = meta
      .filter((x) => x.ch_key === ch_key)
      .map((x) => `if (abs(type - ${x.type_id}.0) < 0.1) return ${x.uniform_name};`)
      .join("\n    ");
    if (cases) {
      color_mapping_funcs += `vec3 ${gen.FUNC_COLOR(ch_key)}(float type) { ${cases} return vec3(0.0); }\n`;
    } else {
      color_mapping_funcs += `vec3 ${gen.FUNC_COLOR(ch_key)}(float type) { return vec3(0.0); }\n`;
    }
  });

  return { color_uniforms_decl, color_mapping_funcs };
};

// ---- CPU-side ground color decode (for the mipmapped color texture) ----
//
// GLSL's color_mapping_funcs above (get_color_ch_X) are the shader-side half
// of this same type_id -> color mapping; this is the JS-side half, used to
// pre-resolve every texel to a real color before three.js builds a mipmap
// chain over the result (see decode_ground_rgba's own comment for why that
// order matters). Kept next to get_color_meta/build_color_mappings since all
// three read the same cube_colors/cube_types tables and must agree.

// Exported so render/layer/'s WorldLayer adapters (terrain/adapter.ts,
// objects/adapter.ts) can resolve a hex color to bytes the same way this
// file's own decode_ground_rgba/build_layer_texture_data already do,
// instead of a second hand-rolled copy of the same parse.
export const hex_to_rgb255 = (hex: string): [number, number, number] => {
  const h = hex.length <= 5 ? hex.slice(1, 4).split("").map((c) => c + c).join("") : hex.slice(1, 7);
  const num = parseInt(h, 16);
  return [(num >> 16) & 255, (num >> 8) & 255, num & 255];
};

export type GroundColorLookup = Partial<Record<GroundChannelKey, Map<u8, [number, number, number]>>>;

export const build_color_lookup = (meta: TerrainColorMeta[]): GroundColorLookup => {
  const out: GroundColorLookup = {};
  meta.forEach((x) => {
    const map = out[x.ch_key] ?? (out[x.ch_key] = new Map());
    map.set(x.type_id, hex_to_rgb255(x.hex_color));
  });
  return out;
};

/** Static — cube_colors/cube_types never change at runtime, so this is built once at module load rather than per-chunk. */
export const GROUND_COLOR_LOOKUP: GroundColorLookup = build_color_lookup(get_color_meta());

/**
 * Same data as GROUND_COLOR_LOOKUP, indexed by flat Uint8Array instead of
 * `Map<u8, [r,g,b]>` — confirmed live via profiling that decode_ground_rgba
 * (below), called once per newly-built chunk, was dominated by its inner
 * Map.get() lookups (up to one L2 column's cells * GROUND_CH_KEYS.length of
 * them per chunk, 4096*4=16384 at the current SHAMTs). Direct u8-indexed
 * array reads are the standard V8 fix for a hot per-texel Map lookup like
 * this one — no hashing/boxing per access. `has` is a separate 0/1 flag
 * array (not e.g. a sentinel color) because [0,0,0] is itself sometimes a
 * real mapped color, so "unmapped" can't be encoded in the rgb array alone
 * — this preserves decode_ground_rgba's original `rgb ? ... : (leave
 * unchanged)` semantics exactly.
 */
type FlatChannelLookup = { rgb: U8Array; has: U8Array };
export type FlatGroundColorLookup = Partial<Record<GroundChannelKey, FlatChannelLookup>>;

function build_flat_color_lookup(lookup: GroundColorLookup): FlatGroundColorLookup {
  const out: FlatGroundColorLookup = {};
  for (const ch_key of GROUND_CH_KEYS) {
    const map = lookup[ch_key];
    if (!map) continue;

    const rgb = new Uint8Array(256 * 3);
    const has = new Uint8Array(256);
    for (const [type_id, color] of map) {
      rgb[type_id * 3] = color[0];
      rgb[type_id * 3 + 1] = color[1];
      rgb[type_id * 3 + 2] = color[2];
      has[type_id] = 1;
    }
    out[ch_key] = { rgb, has };
  }
  return out;
}

export const GROUND_COLOR_LOOKUP_FLAT: FlatGroundColorLookup = build_flat_color_lookup(GROUND_COLOR_LOOKUP);

/**
 * Decodes the raw per-channel category-code rasters (grass=3, road=7, ...)
 * into one flat RGBA buffer, one real color per texel — the necessary step
 * before three.js can build a real mipmap chain over the result (see
 * tex.ts's create_ground_color_texture). Order matters: these are discrete
 * category ids, not a continuous quantity like elevation, so averaging the
 * raw ids (what mipmap generation would do if it ran directly on the
 * category-code rasters) lands on a meaningless id that maps to no real
 * type. Resolving every texel to its real color *first* means the mipmap
 * pyramid three.js builds afterward is averaging actual colors, which is
 * always meaningful — same fix shape as elevation's hand-rolled 4-tap
 * average, just moved to tile-load time and done exactly instead of via 4
 * sparse taps, since color (unlike elevation) doesn't need to stay in an
 * exact-integer format the GPU can't filter.
 *
 * This is what gives shader.ts's low-spec render mode real cross-category
 * color blending at coarse LOD (a hardware mip pyramid built over real
 * per-cell colors naturally averages neighboring different-colored cells
 * together) — the texture-pack mode's per-fragment single-layer resolve
 * (shader_meta.ts's build_ground_layer_resolver) can't reproduce that,
 * since picking one winning category per sample point is inherent to
 * showing a real tileable image rather than a flat blended color. Low-spec
 * keeps this older per-chunk bake specifically because it's still the
 * cheaper, smoother-looking path for flat-fill content; texture-pack trades
 * the smoothness away deliberately, for the ability to show real texture
 * detail at all.
 *
 * show_structures, not a fixed key list: STRUCTURE joins the same LAND ->
 * WATER -> STRUCTURE -> TRANSPORT override priority
 * build_ground_layer_resolver's GLSL version uses (GROUND_CH_KEYS' own
 * order) whenever the debug toggle has it on, instead of always being
 * excluded — chunk.tsx now includes it in this useMemo's own dependency
 * array, so toggling that (rare, dev-only) button re-bakes the handful of
 * currently-loaded chunks' textures instead of needing two textures kept
 * around per chunk just to flip one dev button.
 */
export const decode_ground_rgba = (
  ground: Record<GroundChannelKey, U8Array>,
  row_stride: u32,
  show_structures: boolean = false,
  lookup: FlatGroundColorLookup = GROUND_COLOR_LOOKUP_FLAT,
): Uint8Array => {
  const base: u8 = cube_types.land.BARE;
  const base_flat = lookup[GroundChannelKey.LAND];
  const base_rgb: [number, number, number] = base_flat?.has[base] ? [base_flat.rgb[base * 3], base_flat.rgb[base * 3 + 1], base_flat.rgb[base * 3 + 2]] : [0, 0, 0];
  const n = row_stride * row_stride;
  const out = new Uint8Array(n * 4);

  for (let i = 0; i < n; i++) {
    let r = base_rgb[0], g = base_rgb[1], b = base_rgb[2];
    for (const ch_key of GROUND_CH_KEYS) {
      if (ch_key === GroundChannelKey.STRUCTURE && !show_structures) continue;
      const type_id = ground[ch_key][i];
      if (type_id === 0) continue;
      const flat = lookup[ch_key];
      if (flat && flat.has[type_id]) {
        const off = type_id * 3;
        r = flat.rgb[off];
        g = flat.rgb[off + 1];
        b = flat.rgb[off + 2];
      }
    }
    out[i * 4] = r;
    out[i * 4 + 1] = g;
    out[i * 4 + 2] = b;
    out[i * 4 + 3] = 255;
  }
  return out;
};

// ---- texture-array ground layers (low-spec / texture-pack shared source) ----
//
// Replaces decode_ground_rgba/GROUND_COLOR_LOOKUP's single pre-baked mip
// texture with one shared sampler2DArray (one layer per type, built once
// at module load — see mat.ts) plus a resolver function included in both
// the vertex shader (low-spec mode: samples once per vertex at a fixed UV,
// same cost shape decode_ground_rgba's approach had) and the fragment
// shader (texture-pack mode: samples once per fragment with a real tiling
// UV, so a future real texture image actually shows its own detail across
// a cell's face). Every layer is a flat color fill today — visually
// identical to the old baked approach — but swapping in real per-layer
// images later is then just replacing build_layer_texture_data's output,
// no shader change required either mode.

export type TerrainLayerMeta = TerrainColorMeta & { layer_index: number };

/** One array layer per (channel, type) pair, in get_color_meta's own order
 * — includes STRUCTURE this time (decode_ground_rgba's old single-texture
 * bake deliberately left it out to avoid needing two texture variants for
 * one debug toggle; a texture array has no such problem, since
 * resolve_ground_layer below just skips selecting STRUCTURE's layer when
 * show_structures is false, the same way it already skips WATER/TRANSPORT
 * for a cell not marked as either). */
export const get_layer_meta = (color_meta: TerrainColorMeta[] = get_color_meta()): TerrainLayerMeta[] => color_meta.map((x, layer_index) => ({ ...x, layer_index }));

/** Static — same "never changes at runtime" reasoning as GROUND_COLOR_LOOKUP_FLAT. */
export const GROUND_LAYER_META: readonly TerrainLayerMeta[] = get_layer_meta();

/**
 * CPU-side `(ch_key, type_id) -> layer_index` lookup, direct u8-indexed
 * array reads for the same reason GROUND_COLOR_LOOKUP_FLAT is (a hot
 * per-cell lookup, not per-texel-of-a-whole-texture like decode_ground_
 * rgba's own consumer, but still hot enough at real chunk counts to want
 * this shape rather than a Map). `has` mirrors GROUND_COLOR_LOOKUP_FLAT's
 * own reasoning: layer_index 0 is itself a real, valid slot.
 */
export type FlatLayerIndexLookup = Partial<Record<GroundChannelKey, { index: U8Array; has: U8Array }>>;

function build_flat_layer_index_lookup(meta: readonly TerrainLayerMeta[]): FlatLayerIndexLookup {
  const out: FlatLayerIndexLookup = {};
  for (const m of meta) {
    let entry = out[m.ch_key];
    if (!entry) {
      entry = { index: new Uint8Array(256), has: new Uint8Array(256) };
      out[m.ch_key] = entry;
    }
    entry.index[m.type_id] = m.layer_index;
    entry.has[m.type_id] = 1;
  }
  return out;
}

export const GROUND_LAYER_INDEX_LOOKUP_FLAT: FlatLayerIndexLookup = build_flat_layer_index_lookup(GROUND_LAYER_META);

/** Per-channel `float type -> float layer_index` functions — the array
 * equivalent of build_color_mappings' get_color_ch_X, generated from the
 * same meta so the two can never disagree about what a type_id means. */
const build_layer_mapping_funcs = (meta: TerrainLayerMeta[]): string => {
  let out = "";
  GROUND_CH_KEYS.forEach((ch_key) => {
    const cases = meta
      .filter((x) => x.ch_key === ch_key)
      .map((x) => `if (abs(type - ${x.type_id}.0) < 0.1) return ${x.layer_index}.0;`)
      .join("\n    ");
    out += `float ${gen.FUNC_LAYER(ch_key)}(float type) { ${cases} return 0.0; }\n`;
  });
  return out;
};

/**
 * Shared "which layer does this cell mean" resolver — LAND (defaulting to
 * bare ground when unset, the same fallback decode_ground_rgba's own
 * base_rgb used) -> WATER -> STRUCTURE (only when shown) -> TRANSPORT, each
 * overriding the last, reproducing the exact priority the old single-pass
 * color-blend chain had. Takes already-resolved ch_X float values (from
 * shader.ts's ch_sampling_logic) rather than sampling anything itself, so
 * it's identical whether the caller is the vertex shader (samples once per
 * vertex) or the fragment shader (samples once per fragment) — only the
 * caller's own sampling cadence differs, never this priority logic.
 * Included verbatim in both create_vert and create_frag (see shader.ts).
 */
export const build_ground_layer_resolver = (meta: TerrainLayerMeta[]): string => {
  const land_base_type: u8 = cube_types.land.BARE;
  return `
  ${build_layer_mapping_funcs(meta)}
  float resolve_ground_layer(float ch_land, float ch_water, float ch_structure, float ch_transport, bool show_structures) {
    float layer = ${gen.FUNC_LAYER(GroundChannelKey.LAND)}(ch_land > 0.5 ? ch_land : ${land_base_type}.0);
    if (ch_water > 0.5) layer = ${gen.FUNC_LAYER(GroundChannelKey.WATER)}(ch_water);
    if (show_structures && ch_structure > 0.5) layer = ${gen.FUNC_LAYER(GroundChannelKey.STRUCTURE)}(ch_structure);
    if (ch_transport > 0.5) layer = ${gen.FUNC_LAYER(GroundChannelKey.TRANSPORT)}(ch_transport);
    return layer;
  }
  `;
};

/**
 * Flat RGBA texture-array source data — one size*size solid-color block per
 * layer (meta's own layer_index order). Real image data (not raw category
 * ids), so hardware mipmap generation/filtering just works, same reasoning
 * as decode_ground_rgba's own doc comment. size defaults to 64, matching
 * render/tex_array.ts's own LAYER_TEX_SZ (the active pack's tile
 * size) — small enough (64*64*4 bytes/layer) that even ~30 layers costs
 * nothing worth measuring in texture memory.
 */
export const build_layer_texture_data = (
  meta: TerrainLayerMeta[],
  size: u32 = 64,
): { data: Uint8Array; width: u32; height: u32; depth: u32 } => {
  const depth = meta.length as u32;
  const data = new Uint8Array(size * size * depth * 4);
  meta.forEach((m) => {
    const [r, g, b] = hex_to_rgb255(m.hex_color);
    const layer_offset = m.layer_index * size * size * 4;
    for (let i = 0; i < size * size; i++) {
      const o = layer_offset + i * 4;
      data[o] = r;
      data[o + 1] = g;
      data[o + 2] = b;
      data[o + 3] = 255;
    }
  });
  return { data, width: size, height: size, depth };
};
