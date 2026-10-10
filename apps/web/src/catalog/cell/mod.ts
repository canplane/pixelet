// @pxl/web/catalog/cell/mod.ts

/**
 * The cell catalog as the shell shows it. Cubes and decor are held by the
 * kernel's numbers; names are looked up only for display.
 */

import type { u16 } from "@xpute/core/abi/word.ts";
import palette from "./color.spec.ts";
import { cube_types, ground_kind, type Kind } from "./cube_type.spec.ts";
import { decor_types, type DecorId } from "./decor.spec.ts";

/** A cube by its number in the catalog's one value space: a ground look or a placed color. */
export type PlaceableCube = u16;

/** The catalog's group a cube's name is in. */
type Group = "ground" | "object";

/** The drawer's grouping, and the texture pack's prefix: a look by its kind, a color as a block. */
export type CubeSection = Kind | "block";

function groupOf(cube: PlaceableCube): Group {
  return cube in ground_kind ? "ground" : "object";
}

export function placeableCubeSection(cube: PlaceableCube): CubeSection {
  return ground_kind[cube] ?? "block";
}

const NAME = Object.fromEntries(Object.entries(cube_types).flatMap(([, values]) => Object.entries(values).map(([name, n]) => [n, name]))) as Record<number, string>;

export function placeableCubeType(cube: PlaceableCube): string {
  return NAME[cube];
}

export function placeableCubeId(cube: PlaceableCube): string {
  return `${groupOf(cube)}.${placeableCubeType(cube)}`;
}

export function placeableCubeHex(cube: PlaceableCube): string | null {
  const entries = palette[groupOf(cube)] as Record<string, { hex: string } | undefined>;
  return entries[placeableCubeType(cube)]?.hex ?? null;
}

/** The placed colors first, then the looks in paint order. */
export const placeableCubes: readonly PlaceableCube[] = [
  ...Object.values(cube_types.object).filter((n) => n !== cube_types.object.air),
  ...Object.values(cube_types.ground),
];

const DECOR_NAME = Object.fromEntries(Object.entries(decor_types).map(([name, n]) => [n, name])) as Record<number, DecorId>;

export const decorKinds: readonly u16[] = Object.values(decor_types).sort((a, b) => a - b);

export function decorName(kind: u16): DecorId {
  return DECOR_NAME[kind];
}

/** The drawer's grouping of decor, the shell's to choose. */
const DECOR_SECTION = { tree: "nature", boat: "vehicle", car: "vehicle", train: "vehicle" } as const satisfies Record<DecorId, string>;

export type DecorSection = (typeof DECOR_SECTION)[DecorId];

export function decorSection(kind: u16): DecorSection {
  return DECOR_SECTION[decorName(kind)];
}
