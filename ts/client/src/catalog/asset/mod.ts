// @pxl/client/catalog/asset/mod.ts

/**
 * The asset catalog as the shell shows it. Cubes and decor are held by the
 * kernel's numbers; names are looked up only for display.
 */

import type { u16 } from "@xpute/kit/abi/word.ts";
import palette from "./color.spec.ts";
import { cube, cube_section, type CubeSection } from "./cube.spec.ts";
import { decor, type DecorId } from "./decor.spec.ts";

/** A cube by its number in the catalog's one value space: a ground look or a placed color. */
export type Cube = u16;

export type { CubeSection };

/** The catalog's group a cube's name is in. */
type Group = "ground" | "object";

const GROUND: ReadonlySet<number> = new Set(Object.values(cube.ground));

function groupOf(cube: Cube): Group {
  return GROUND.has(cube) ? "ground" : "object";
}

/** The catalog's section the drawer lists a cube under. */
export function cubeSection(cube: Cube): CubeSection {
  return cube_section[cube];
}

const NAME = Object.fromEntries(Object.entries(cube).flatMap(([, values]) => Object.entries(values).map(([name, n]) => [n, name]))) as Record<number, string>;

function cubeName(cube: Cube): string {
  return NAME[cube];
}

export function cubeId(cube: Cube): string {
  return `${groupOf(cube)}.${cubeName(cube)}`;
}

export function cubeHex(cube: Cube): string | null {
  const entries = palette[groupOf(cube)] as Record<string, { hex: string } | undefined>;
  return entries[cubeName(cube)]?.hex ?? null;
}

/** The placed colors first, then the looks in paint order. */
export const allCubes: readonly Cube[] = [
  ...Object.values(cube.object).filter((n) => n !== cube.object.air),
  ...Object.values(cube.ground),
];

const DECOR_NAME = Object.fromEntries(Object.entries(decor).map(([name, n]) => [n, name])) as Record<number, DecorId>;

export const decorIds: readonly u16[] = Object.values(decor).sort((a, b) => a - b);

export function decorName(kind: u16): DecorId {
  return DECOR_NAME[kind];
}

/** The drawer's grouping of decor, the shell's to choose. */
const DECOR_SECTION = { tree: "nature", boat: "vehicle", car: "vehicle", train: "vehicle" } as const satisfies Record<DecorId, string>;

export type DecorSection = (typeof DECOR_SECTION)[DecorId];

export function decorSection(kind: u16): DecorSection {
  return DECOR_SECTION[decorName(kind)];
}
