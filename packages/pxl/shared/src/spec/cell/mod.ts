// @pxl/shared/spec/cell/mod.ts
// Entry point for the cell model. Assembles and re-exports the modular definitions.

import { GROUND_CH_KEYS, type GroundChannelKey } from "@pxl/shared/bake/format.ts";

import { cube_colors, cube_types, type CubeColorId } from "./cube.ts";
import { decor_types, type DecorId } from "./decor.ts";

export { cube_colors, cube_types, type CubeColorId } from "./cube.ts";
export { decor_types, type DecorId } from "./decor.ts";

// =============================================================================
// Aggregated Types
// =============================================================================

export const cell_types = {
  /** Terrain raster + user-placed block colors (cube_types.common holds the
   * shared VOID/AIR/UNKNOWN sentinel registry every other channel/consumer
   * references as needed — no separate top-level slot for it here, see
   * cube.ts's own top comment). */
  cube: cube_types,

  /** User-placed decor kinds */
  decor: decor_types,
} as const;

// =============================================================================
// Aggregated Colors
// =============================================================================

export const cell_colors = {
  /** Terrain raster + user-placed block colors */
  cube: cube_colors,
} as const;

// =============================================================================
// Object-layer codec — cube.ts/decor.ts are pure data (their own top doc
// comments); the actual encode/decode logic for their shared sparse-tree
// storage (object/store.ts, apps/web) lives here instead, the one place
// that already aggregates every cell-definition module together.
// =============================================================================

/** Zero (cube_types.common.VOID) means sparse absence/fallback here, not
 * explicit air — the only real distinction left (2026-09-06) now that a
 * real value doesn't need to share a bit with every other real value; see
 * cube.ts's own top comment. */
export function object_value_is_present(value: number): boolean {
  return value !== cube_types.common.VOID;
}

/** Explicit object-layer air: retain this leaf and erase lower layers. */
export function object_value_is_air(value: number): boolean {
  return value === cube_types.common.AIR;
}

export function cube_num_id(id: CubeColorId): number {
  return cube_types.block[id];
}

export function cube_hex(id: CubeColorId): string {
  // block carries no sentinels of its own any more — every key is a real color.
  return cube_colors.block[cube_types.block[id]] as string;
}

const NUM_ID_TO_CUBE: readonly CubeColorId[] = (() => {
  const out: CubeColorId[] = [];
  for (const name of Object.keys(cube_types.block) as CubeColorId[]) out[cube_types.block[name]] = name;
  return out;
})();

/** Numeric id -> cube color name — the reverse of cube_num_id, used to decode a packed cube channel back into a PlacedObject's own CubeColorId. */
export function cube_id_from_num(num_id: number): CubeColorId {
  const id = NUM_ID_TO_CUBE[num_id];
  if (id === undefined) throw new Error(`no cube color registered for numeric id ${num_id}`);
  return id;
}

/** Every real cube color id in encoded numeric-id order; `.filter()` compacts the sparse reverse table. */
export const cube_ids: readonly CubeColorId[] = NUM_ID_TO_CUBE.filter(() => true);

const DECOR_NUM_IDS: ReadonlySet<number> = new Set(Object.values(decor_types));

/** A cube-catalog value and a decor-catalog one share the same numeric
 * range (both plain small numbers, no distinguishing bit) — this is the
 * one place that tells the two apart, same role object_value_is_air plays
 * for AIR within the cube catalog itself. */
export function object_value_is_decor(value: number): boolean {
  return DECOR_NUM_IDS.has(value);
}

export function decor_num_id(id: DecorId): number {
  return decor_types[id];
}

const NUM_ID_TO_DECOR: readonly DecorId[] = (() => {
  const out: DecorId[] = [];
  for (const name of Object.keys(decor_types) as DecorId[]) out[decor_types[name]] = name;
  return out;
})();

/** Numeric id -> decor name — the reverse of decor_num_id, used to decode a packed decor leaf back into a PlacedDecor's own DecorId. */
export function decor_id_from_num(num_id: number): DecorId {
  const id = NUM_ID_TO_DECOR[num_id];
  if (id === undefined) throw new Error(`no decor kind registered for numeric id ${num_id}`);
  return id;
}

/** Every real decor id in encoded numeric-id order — mode_bar.tsx's decor buttons iterate this, same shape as cube_ids. */
export const decor_ids: readonly DecorId[] = NUM_ID_TO_DECOR.filter(() => true);

// =============================================================================
// Terrain-styled cube catalog — user-placeable references into cube_types'
// own land/water/structure/transport channels, derived generically (no
// hand-typed per-type catalog to keep in sync — 2026-09-06, see
// issue/20260906-unified-block-palette.md). A terrain-styled id is
// just `"<channel>.<local key>"`, e.g. "land.GREEN_DEEP" — its own local
// value (cube_types.land.GREEN_DEEP) is whatever the bake pipeline already
// uses; the *global* sparse-tree byte a placed cube of this kind actually
// gets stored as is a separately assigned dense sequence below, since a
// real terrain cell can hold a value from every channel simultaneously but
// a placed cube's single global byte can only ever mean one of them.
// =============================================================================

type TerrainChannelKey = Exclude<keyof typeof cube_types, "block" | "common">;
// No Exclude needed here any more — land/water/structure/transport don't
// carry common's own VOID/AIR/UNKNOWN keys at all (cube.ts's own top
// comment), so every key a terrain channel actually has is real.
type ChannelLocalKey<C extends TerrainChannelKey> = keyof typeof cube_types[C] & string;

export type TerrainStyleCubeId = { [C in TerrainChannelKey]: `${C}.${ChannelLocalKey<C>}` }[TerrainChannelKey];

const TERRAIN_CHANNELS = (Object.keys(cube_types) as (keyof typeof cube_types)[])
  .filter((ch): ch is TerrainChannelKey => ch !== "block" && ch !== "common");

/**
 * Which cube_types channel+local-value each terrain-styled id stands in
 * for. The one thing every real consumer needs: render/layer/object/
 * adapter.ts reads this to resolve a placed cube's own real color/layer_idx
 * from render/layer/terrain/shader_meta.ts's own lookup tables (the same
 * ones terrain rendering itself uses).
 */
const TERRAIN_STYLE_SOURCE = new Map<TerrainStyleCubeId, { channel: GroundChannelKey; terrain_num_id: number }>();
const TERRAIN_STYLE_TO_NUM = new Map<TerrainStyleCubeId, number>();
const NUM_TO_TERRAIN_STYLE = new Map<number, TerrainStyleCubeId>();

(() => {
  let next_num = 1;
  for (const channel of TERRAIN_CHANNELS) {
    const local_types = cube_types[channel] as Record<string, number>;
    for (const local_key of Object.keys(local_types)) {
      const id = `${channel}.${local_key}` as TerrainStyleCubeId;
      TERRAIN_STYLE_SOURCE.set(id, { channel: channel as unknown as GroundChannelKey, terrain_num_id: local_types[local_key] });
      TERRAIN_STYLE_TO_NUM.set(id, next_num);
      NUM_TO_TERRAIN_STYLE.set(next_num, id);
      next_num++;
    }
  }
})();

// Real swatch colors for every entry above, read straight from cube_colors'
// own already-computed palette (no second hand-authored copy to keep in sync).
const TERRAIN_STYLE_HEX = new Map<TerrainStyleCubeId, string | null>();
for (const [id, { channel, terrain_num_id }] of TERRAIN_STYLE_SOURCE) {
  TERRAIN_STYLE_HEX.set(id, cube_colors[channel]?.[terrain_num_id] ?? null);
}

// =============================================================================
// Placeable cube catalog — everything mode_bar.tsx's hotbar (cube_ids alone)
// and palette_modal.tsx's own full grid (this) can offer a user. A separate
// concept from cube_ids itself: the hotbar deliberately stays scoped to just
// the original 15 flat colors (2026-09-06 UX decision — the full catalog
// lives behind its own modal, not folded into the everyday quick-pick).
// =============================================================================

export type PlaceableCubeId = CubeColorId | TerrainStyleCubeId;

export function is_terrain_style_cube(id: PlaceableCubeId): id is TerrainStyleCubeId {
  return TERRAIN_STYLE_TO_NUM.has(id as TerrainStyleCubeId);
}

/** channel/terrain_num_id a terrain-styled placeable id resolves to — render/layer/object/adapter.ts's own material resolver. */
export function terrain_style_cube_source(id: TerrainStyleCubeId): { channel: GroundChannelKey; terrain_num_id: number } {
  const source = TERRAIN_STYLE_SOURCE.get(id);
  if (!source) throw new Error(`no terrain-style source registered for "${id}"`);
  return source;
}

/** The terrain-styled id standing for a real baked (channel, type) pair —
 * the inverse of terrain_style_cube_source, for callers that hold what the
 * terrain shows and want the cube that looks like it. */
export function terrain_style_cube_for(channel: GroundChannelKey, terrain_num_id: number): TerrainStyleCubeId | null {
  for (const [id, source] of TERRAIN_STYLE_SOURCE) {
    if (source.channel === channel && source.terrain_num_id === terrain_num_id) return id;
  }
  return null;
}

export function placeable_cube_hex(id: PlaceableCubeId): string | null {
  return is_terrain_style_cube(id) ? TERRAIN_STYLE_HEX.get(id) ?? null : cube_hex(id);
}

export function placeable_cube_num_id(id: PlaceableCubeId): number {
  return is_terrain_style_cube(id) ? TERRAIN_STYLE_TO_NUM.get(id)! : cube_num_id(id);
}

export function placeable_cube_id_from_num(num_id: number): PlaceableCubeId {
  const terrain_style = NUM_TO_TERRAIN_STYLE.get(num_id);
  return terrain_style ?? cube_id_from_num(num_id);
}

/** Every placeable id, flat colors first (cube_ids' own order) then every
 * terrain-styled entry, channel by channel (GROUND_CH_KEYS' own order) —
 * mode_bar.tsx's hotbar keeps using cube_ids alone; this is
 * palette_modal.tsx's own full-catalog source. */
export const placeable_cube_ids: readonly PlaceableCubeId[] = [
  ...cube_ids,
  ...GROUND_CH_KEYS.flatMap((channel) =>
    (Object.keys(cube_types[channel]) as string[])
      .map((local_key) => `${channel}.${local_key}` as TerrainStyleCubeId)
  ),
];
