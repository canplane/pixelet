// @pxl/shared/spec/cell/mod.ts
// Entry point for the cell model. Assembles and re-exports the modular definitions.

import { GROUND_CH_KEYS, GroundChannelKey } from "@pxl/shared/bake/format.ts";

import { cube_colors, cube_types, type CubeColorId } from "./cube.ts";
import type { Swatch } from "./type.ts";
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

/** Zero (cube_types.common.void) means sparse absence/fallback here, not
 * explicit air — the only real distinction left (2026-09-06) now that a
 * real value doesn't need to share a bit with every other real value; see
 * cube.ts's own top comment. */
export function object_value_is_present(value: number): boolean {
  return value !== cube_types.common.void;
}

/** Explicit object-layer air: retain this leaf and erase lower layers. */
export function object_value_is_air(value: number): boolean {
  return value === cube_types.common.air;
}

export function cube_num_id(id: CubeColorId): number {
  return cube_types.block[id];
}

export function cube_hex(id: CubeColorId): string {
  return cube_swatch(id).hex;
}

/** The same swatch packed `0xRRGGBB`, for the callers that light or blend with
 * it rather than write it down. */
export function cube_rgb(id: CubeColorId): number {
  return cube_swatch(id).rgb;
}

function cube_swatch(id: CubeColorId): Swatch {
  // block carries no sentinels of its own any more — every key is a real color.
  return cube_colors.block[cube_types.block[id]] as Swatch;
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
// issue/20260906-unified-block-palette.md). Its own local value
// (cube_types.land.green_deep) is whatever the bake pipeline already uses;
// the *global* sparse-tree byte a placed cube of this kind actually gets
// stored as is a separately assigned dense sequence below, since a real
// terrain cell can hold a value from every channel simultaneously but a
// placed cube's single global byte can only ever mean one of them.
// =============================================================================

/** Every channel a cube can be placed from — common is the shared sentinel
 * registry (VOID/AIR/UNKNOWN), which nothing places. */
type PlaceableChannelKey = Exclude<keyof typeof cube_types, "common">;
type TerrainChannelKey = Exclude<PlaceableChannelKey, "block">;
// No Exclude needed within a channel — land/water/structure/transport don't
// carry common's own VOID/AIR/UNKNOWN keys at all (cube.ts's own top
// comment), so every key a channel actually has is real.
type ChannelLocalKey<C extends PlaceableChannelKey> = keyof typeof cube_types[C] & string;

/** `"<channel>.<local key>"`, e.g. `"land.green_deep"`. */
export type TerrainStyleCubeId = { [C in TerrainChannelKey]: `${C}.${ChannelLocalKey<C>}` }[TerrainChannelKey];

const TERRAIN_CHANNELS = (Object.keys(cube_types) as (keyof typeof cube_types)[])
  .filter((ch): ch is TerrainChannelKey => ch !== "block" && ch !== "common");

/** Every type one channel gives out, as the numbers themselves — the closed
 * set cube_type.json declares, which is what a cube of that channel is. */
type CubeTypeOf<C extends keyof typeof cube_types> = (typeof cube_types)[C][keyof (typeof cube_types)[C]];

/** A placeable cube: a flat colour, or a stand-in for one baked terrain
 * type. Two numbers the catalog declares, never the words they are spelled
 * out with — those are what `placeable_cube_id` builds, for a label and for
 * an asset path, and nothing else reads them. */
export type PlaceableCube =
  | { readonly channel: "block"; readonly type: CubeTypeOf<"block"> }
  | { readonly channel: GroundChannelKey.LAND; readonly type: CubeTypeOf<"land"> }
  | { readonly channel: GroundChannelKey.WATER; readonly type: CubeTypeOf<"water"> }
  | { readonly channel: GroundChannelKey.STRUCTURE; readonly type: CubeTypeOf<"structure"> }
  | { readonly channel: GroundChannelKey.TRANSPORT; readonly type: CubeTypeOf<"transport"> };

/** The two numbers as one map key. */
function cube_key(channel: string, type: number): string {
  return `${channel}:${type}`;
}

/** Where a stored leaf's value stops being a number of its own and starts
 * being a terrain-styled cube's `(channel, type)`.
 *
 * Everything the catalogs number directly sits below this: VOID at 0, AIR at
 * 128, block at 130..144, decor at 228, UNKNOWN at 255 — all of one byte, and
 * the widest of them is the byte. Above it, a terrain-styled cube is the
 * channel it stands in for and the type within that channel, which is exactly
 * what it is, so the value can be read back without a table. */
export const TERRAIN_STYLE_BASE = 256;

/** Every terrain-styled cube, channel by channel, for the callers that want
 * the catalog walked. The numbering is no longer this walk's to give out:
 * counting one out as it was met meant a type inserted into cube_type.json
 * renumbered every type after it, in every other channel too, and a stored
 * placement then named a different cube than the one that was placed.
 *
 * A cube is built by widening `{channel, type}` from the catalog's own tables,
 * which is where the check that the pair is a real one happens — the cast says
 * the walk is that check. */
const TERRAIN_STYLE: PlaceableCube[] = TERRAIN_CHANNELS.flatMap((channel) =>
  Object.values(cube_types[channel] as Record<string, number>).map((type) => ({ channel, type }) as PlaceableCube)
);

/** The slot a ground channel is, which is the enum's own order — a bake's
 * lanes are in it already, so it is a number the format has fixed rather than
 * one chosen here. */
function channel_slot(channel: GroundChannelKey): number {
  return GROUND_CH_KEYS.indexOf(channel);
}

/** Each channel's numbers back to the name cube_type.json gives them. */
const TYPE_NAME = new Map<string, string>();
for (const [channel, types] of Object.entries(cube_types)) {
  for (const [name, num] of Object.entries(types as Record<string, number>)) TYPE_NAME.set(cube_key(channel, num), name);
}

// =============================================================================
// Placeable cube catalog — everything mode_bar.tsx's hotbar (cube_ids alone)
// and drawer_modal.tsx's own full grid (this) can offer a user. A separate
// concept from cube_ids itself: the hotbar deliberately stays scoped to just
// the original 15 flat colors (2026-09-06 UX decision — the full catalog
// lives behind its own modal, not folded into the everyday quick-pick).
// =============================================================================

/** What a cube is spelled out as: `"<channel>.<local key>"` for every
 * channel alike, block included — the catalog gives block no standing the
 * other four don't have (cube_type.json lists it as one more channel), so
 * neither does the spelling, and neither do the locale entries or the
 * drawer's sections that are built from it. */
export type PlaceableCubeId = { [C in PlaceableChannelKey]: `${C}.${ChannelLocalKey<C>}` }[PlaceableChannelKey];

/** The id a cube is spelled out under — `"block.red"`, `"land.bare"`.
 * Exactly two consumers want the words: the locale entry
 * (`cubes.land.bare`, i18next resolving the dot itself) and the
 * shipped texture's path. It is what a cube is called, never what it is, so
 * nothing keys by it. */
export function placeable_cube_id(cube: PlaceableCube): PlaceableCubeId {
  return cube_id_of(cube.channel, TYPE_NAME.get(cube_key(cube.channel, cube.type))!);
}

/** How a catalog id is spelled, for the callers that hold the two words
 * already rather than a cube. The shipped palette is one: it walks every
 * channel's ENTRIES, `common` included, and `common` is not a `PlaceableCube`,
 * so it cannot come through the function above — but it must come out spelled
 * the same, and it was spelling it a second time instead. */
export function cube_id_of(channel: string, type_name: string): PlaceableCubeId {
  return `${channel}.${type_name}` as PlaceableCubeId;
}

/** The value a leaf holds the cube as: a flat colour's own catalog number, or
 * a terrain-styled cube's channel and type. */
export function placeable_cube_num_id(cube: PlaceableCube): number {
  return cube.channel === "block" ? cube.type : TERRAIN_STYLE_BASE * (channel_slot(cube.channel) + 1) + cube.type;
}

/** The cube a stored leaf value names, or null for a value that names no
 * cube — VOID, AIR, a decor kind. */
export function placeable_cube_of(num_id: number): PlaceableCube | null {
  if (num_id >= TERRAIN_STYLE_BASE) {
    const channel = GROUND_CH_KEYS[Math.floor(num_id / TERRAIN_STYLE_BASE) - 1];
    if (channel === undefined) return null;
    const type = num_id % TERRAIN_STYLE_BASE;
    return TYPE_NAME.has(cube_key(channel, type)) ? { channel, type } as PlaceableCube : null;
  }
  return TYPE_NAME.has(cube_key("block", num_id)) ? { channel: "block", type: num_id as CubeTypeOf<"block"> } : null;
}

/** Which baked (channel, type) the cube stands in for, null for a flat
 * colour, which stands in for nothing. */
export function terrain_style_cube_source(cube: PlaceableCube): { channel: GroundChannelKey; terrain_num_id: number } | null {
  return cube.channel === "block" ? null : { channel: cube.channel, terrain_num_id: cube.type };
}

/** The cube standing in for a real baked (channel, type) pair — the inverse
 * of terrain_style_cube_source, for callers that hold what the terrain shows
 * and want the cube that looks like it. */
export function terrain_style_cube_for(channel: GroundChannelKey, terrain_num_id: number): PlaceableCube | null {
  return TYPE_NAME.has(cube_key(channel, terrain_num_id)) ? { channel, type: terrain_num_id } as PlaceableCube : null;
}

/** The swatch colour the catalog gives it — cube_colors is keyed by channel
 * and number, so this is the palette's own row, not a second table. */
export function placeable_cube_hex(cube: PlaceableCube): string | null {
  return placeable_cube_swatch(cube)?.hex ?? null;
}

/** The same swatch as the one number it is, which is what everything that
 * lights or blends with a colour wants — and what nothing has to parse text to
 * find out, the catalog's JSON having been packed by the generator. */
export function placeable_cube_rgb(cube: PlaceableCube): number | null {
  return placeable_cube_swatch(cube)?.rgb ?? null;
}

function placeable_cube_swatch(cube: PlaceableCube): Swatch | null {
  return cube_colors[cube.channel]?.[cube.type] ?? null;
}

/** Every cube the catalogs give out, flat colours first in the catalog's own
 * numeric order, then every terrain-styled entry channel by channel. */
export const placeable_cubes: readonly PlaceableCube[] = [
  ...cube_ids.map((id) => ({ channel: "block", type: cube_types.block[id] }) as PlaceableCube),
  ...TERRAIN_STYLE,
];
