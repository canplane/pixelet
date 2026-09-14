// @pxl/web/render/gpu/layer_table.ts

/**
 * The palette as tables: every (channel, type) the cube catalogue colours,
 * numbered as the layers of the texture pack's arrays (layers.ts builds
 * the arrays in this order), and the placed cubes' colours numbered after
 * them. What the module is taught for the terrain (terrain/teach.ts) and
 * the objects (object/teach.ts) reads from here, as does the reference
 * paint, so the module, the reference and the arrays cannot disagree about
 * what a type_id means.
 */

import type { u32, u8 } from "@xpute/core/kit/word.ts";
import type { U8Array } from "@xpute/core/kit/array.ts";

import { cube_colors, cube_types } from "@pxl/shared/spec/cell/cube.ts";
import { cube_ids, type CubeColorId } from "@pxl/shared/spec/cell/mod.ts";
import { GROUND_CH_KEYS, type GroundChannelKey } from "@pxl/shared/bake/format.ts";

const KEYS = <T extends object>(v: T): Array<keyof T> => Object.keys(v) as Array<keyof T>;

const IS_VALID_HEX = (hex: string): boolean => /^#([A-Fa-f0-9]{3,4}|[A-Fa-f0-9]{6}|[A-Fa-f0-9]{8})$/.test(hex);

/** A coloured type of a channel, and the array layer it is. */
export type TerrainLayerMeta = {
  ch_key: GroundChannelKey;
  type_key: string;
  type_id: u8;
  hex_color: string;
  layer_index: number;
};

/** One array layer per (channel, type) pair, in the catalogue's order. */
function layer_meta(): TerrainLayerMeta[] {
  const out: TerrainLayerMeta[] = [];
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
        out.push({ ch_key, type_key, type_id, hex_color, layer_index: out.length });
      }
    });
  });
  return out;
}

export const hex_to_rgb255 = (hex: string): [number, number, number] => {
  const h = hex.length <= 5 ? hex.slice(1, 4).split("").map((c) => c + c).join("") : hex.slice(1, 7);
  const num = parseInt(h, 16);
  return [(num >> 16) & 255, (num >> 8) & 255, num & 255];
};

/** Static — cube_colors/cube_types never change at runtime, so this is built
 * once at module load rather than per-chunk. */
export const GROUND_LAYER_META: readonly TerrainLayerMeta[] = layer_meta();

/**
 * A channel's colours by type, as flat u8-indexed arrays rather than a
 * `Map<u8, [r,g,b]>`: confirmed live via profiling that the per-cell
 * decode, called once per newly-built chunk, was dominated by its inner
 * Map.get() lookups (one L2 column's cells * GROUND_CH_KEYS.length of them
 * per chunk, 4096*4=16384 at the current SHAMTs). Direct u8-indexed array
 * reads are the standard V8 fix for a hot per-texel Map lookup like this
 * one — no hashing/boxing per access. `has` is a separate 0/1 flag array
 * (not e.g. a sentinel color) because [0,0,0] is itself sometimes a real
 * mapped color, so "unmapped" can't be encoded in the rgb array alone.
 */
type FlatChannelLookup = { rgb: U8Array; has: U8Array };
export type FlatGroundColorLookup = Partial<Record<GroundChannelKey, FlatChannelLookup>>;

function build_flat_color_lookup(meta: readonly TerrainLayerMeta[]): FlatGroundColorLookup {
  const out: FlatGroundColorLookup = {};
  for (const m of meta) {
    let entry = out[m.ch_key];
    if (!entry) {
      entry = { rgb: new Uint8Array(256 * 3), has: new Uint8Array(256) };
      out[m.ch_key] = entry;
    }
    const [r, g, b] = hex_to_rgb255(m.hex_color);
    entry.rgb[m.type_id * 3] = r;
    entry.rgb[m.type_id * 3 + 1] = g;
    entry.rgb[m.type_id * 3 + 2] = b;
    entry.has[m.type_id] = 1;
  }
  return out;
}

export const GROUND_COLOR_LOOKUP_FLAT: FlatGroundColorLookup = build_flat_color_lookup(GROUND_LAYER_META);

/**
 * `(ch_key, type_id) -> layer_index`, direct u8-indexed for the same reason
 * GROUND_COLOR_LOOKUP_FLAT is. `has` mirrors its reasoning: layer_index 0
 * is itself a real, valid slot.
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

/** Object colors, numbered starting right after every ground layer —
 * layers.ts builds the array in this order, and object/material.ts reads
 * this to resolve a placed cube's layer_idx the same way the terrain reads
 * GROUND_LAYER_INDEX_LOOKUP_FLAT. Here rather than beside the texture
 * because it is a numbering, not a texture: what a placed cube's layer is
 * must be answerable where no canvas exists.
 *
 * cube_ids (cell/mod.ts) is only ever the original 15 flat colors — the
 * terrain-styled catalog (2026-09-06's own cell/mod.ts, derived from
 * cube_types' own land/water/structure/transport channels) is a completely
 * separate numbering, deliberately never mixed into this one: those ids are
 * meant to look identical to an existing ground layer, not get a new one of
 * their own — object/material.ts's resolve_solid_material reads
 * GROUND_LAYER_INDEX_LOOKUP_FLAT directly for them instead of this table, so
 * cube_ids never needs filtering here. */
export const OBJECT_LAYER_IDX: Readonly<Partial<Record<CubeColorId, u32>>> = Object.fromEntries(cube_ids.map((color_id, i) => [color_id, GROUND_LAYER_META.length + i]));
