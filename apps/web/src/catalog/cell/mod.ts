// @pxl/web/catalog/cell/mod.ts

/**
 * The cell catalog as the shell shows it. Cubes and decor are held by the
 * kernel's numbers; names are looked up only for display.
 */

import type { u8 } from "@xpute/core/abi/word.ts";
import palette from "./color.spec.ts";
import { cube_types, ground_pack, type GroundChannelKey } from "./cube_type.spec.ts";
import { decor_types, type DecorId } from "./decor.spec.ts";

export interface PlaceableCube {
  readonly channel: GroundChannelKey | null;
  readonly type: u8;
}

export function sameCube(a: PlaceableCube, b: PlaceableCube): boolean {
  return a.channel === b.channel && a.type === b.type;
}

export type ChannelName = keyof typeof cube_types;

export function placeableCubeChannel(cube: PlaceableCube): ChannelName {
  return cube.channel === null ? "block" : ground_pack.channels[cube.channel];
}

const TYPE_NAME = Object.fromEntries(
  Object.entries(cube_types).map(([channel, types]) => [channel, Object.fromEntries(Object.entries(types).map(([name, n]) => [n, name]))]),
) as Record<ChannelName, Record<number, string>>;

export function placeableCubeType(cube: PlaceableCube): string {
  return TYPE_NAME[placeableCubeChannel(cube)][cube.type];
}

export function placeableCubeId(cube: PlaceableCube): string {
  return `${placeableCubeChannel(cube)}.${placeableCubeType(cube)}`;
}

export function placeableCubeHex(cube: PlaceableCube): string | null {
  const entries = (palette as unknown as Record<string, Record<string, { hex: string }> | undefined>)[placeableCubeChannel(cube)];
  return entries?.[placeableCubeType(cube)]?.hex ?? null;
}

export const placeableCubes: readonly PlaceableCube[] = [
  ...Object.values(cube_types.block).sort((a, b) => a - b).map((type) => ({ channel: null, type })),
  ...ground_pack.channels.flatMap((name, channel) => Object.values(cube_types[name]).map((type) => ({ channel: channel as GroundChannelKey, type }))),
];

const DECOR_NAME = Object.fromEntries(Object.entries(decor_types).map(([name, n]) => [n, name])) as Record<number, DecorId>;

export const decorKinds: readonly u8[] = Object.values(decor_types).sort((a, b) => a - b);

export function decorName(kind: u8): DecorId {
  return DECOR_NAME[kind];
}
