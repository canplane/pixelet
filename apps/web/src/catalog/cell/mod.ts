// @pxl/web/catalog/cell/mod.ts

/**
 * The cell catalog as the shell shows it. A cube and a decor kind are held the
 * way the kernel names them — a ground channel, or none for a flat color,
 * and the catalog's number — and their words are looked up only to be shown:
 * a cube's id for its locale entry and its texture's path, its color for a
 * swatch, a decor kind's name for its icon and its label. What a cube is
 * stored as is the kernel's (pxl-spec cell/object.rs).
 */

import type { u8 } from "@xpute/core/abi/word.ts";
import palette from "./color.spec.ts";
import { cube_types, ground_pack, type GroundChannelKey } from "./cube_type.spec.ts";
import { decor_types, type DecorId } from "./decor.spec.ts";

/** A placeable cube, as INPUT_POINTER carries it: the ground channel it
 * stands in for, none for a flat color, and its type by number. */
export interface PlaceableCube {
  readonly channel: GroundChannelKey | null;
  readonly type: u8;
}

export function sameCube(a: PlaceableCube, b: PlaceableCube): boolean {
  return a.channel === b.channel && a.type === b.type;
}

/** A catalog channel's name, as cube_type.json gives it. */
export type ChannelName = keyof typeof cube_types;

/** The channel a cube's type is numbered in, by name: `"block"` for a flat
 * color. */
export function placeableCubeChannel(cube: PlaceableCube): ChannelName {
  return cube.channel === null ? "block" : ground_pack.channels[cube.channel];
}

/** Each channel's types by number, back to the names cube_type.json gives
 * them. */
const TYPE_NAME = Object.fromEntries(
  Object.entries(cube_types).map(([channel, types]) => [channel, Object.fromEntries(Object.entries(types).map(([name, n]) => [n, name]))]),
) as Record<ChannelName, Record<number, string>>;

/** A cube's type by name, as its channel in cube_type.json gives it. */
export function placeableCubeType(cube: PlaceableCube): string {
  return TYPE_NAME[placeableCubeChannel(cube)][cube.type];
}

/** A cube's words, `"land.bare"`: its locale entry (i18next resolving the
 * dot), and nothing it is found by. */
export function placeableCubeId(cube: PlaceableCube): string {
  return `${placeableCubeChannel(cube)}.${placeableCubeType(cube)}`;
}

/** The color the catalog gives a cube, as written, for a swatch. */
export function placeableCubeHex(cube: PlaceableCube): string | null {
  const entries = (palette as unknown as Record<string, Record<string, { hex: string }> | undefined>)[placeableCubeChannel(cube)];
  return entries?.[placeableCubeType(cube)]?.hex ?? null;
}

/** Every cube the catalog gives out: the flat colors by number, then each
 * ground channel's types, the channels in their own order. */
export const placeableCubes: readonly PlaceableCube[] = [
  ...Object.values(cube_types.block).sort((a, b) => a - b).map((type) => ({ channel: null, type })),
  ...ground_pack.channels.flatMap((name, channel) => Object.values(cube_types[name]).map((type) => ({ channel: channel as GroundChannelKey, type }))),
];

const DECOR_NAME = Object.fromEntries(Object.entries(decor_types).map(([name, n]) => [n, name])) as Record<number, DecorId>;

/** Every decor kind, by number. */
export const decorKinds: readonly u8[] = Object.values(decor_types).sort((a, b) => a - b);

/** A decor kind's name, for its icon and its locale entry. */
export function decorName(kind: u8): DecorId {
  return DECOR_NAME[kind];
}
