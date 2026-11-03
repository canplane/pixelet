// @pxl/client/shell/catalog/asset/mod.ts

/**
 * The asset catalog as the shell shows it, every cube and decor by the
 * kernel's number: its section, its color and its name, each read by that
 * number from the tables the specs generate.
 */

import type { u16 } from "@xpute/kit/abi/word.ts";
import { CUBE_HEX } from "./color.spec.ts";
import {
  CUBE_LABEL,
  CUBE_SECTION,
  CUBES,
  type CubeSection,
} from "./cube.spec.ts";
import { decor, DECOR_LABEL, DECORS } from "./decor.spec.ts";
import { KIND, SHAPE_BITS } from "./id.spec.ts";

/** An asset by its number in the catalog's one value space: `kind << SHAPE_BITS | shape`. */
export type AssetId = u16;

/** The one marker a placement puts up. */
export const MARKER: AssetId = KIND.marker << SHAPE_BITS;

/** An asset's kind, as its number says. */
export function kindOf(id: AssetId): number {
  return id >> SHAPE_BITS;
}

/** A cube by its number in the catalog's one value space: a ground palette entry or a placed color. */
export type Cube = u16;

/** The catalog's section the drawer lists a cube under. */
export function cubeSection(cube: Cube): CubeSection {
  return CUBE_SECTION[cube];
}

export function cubeHex(cube: Cube): string | null {
  return CUBE_HEX[cube] ?? null;
}

/** Its name, the one every locale shows. */
export function cubeLabel(cube: Cube): string {
  return CUBE_LABEL[cube] ?? "";
}

/** The placed colors first, then the palette entries in paint order. */
export const allCubes: readonly Cube[] = CUBES;

export const decorIds: readonly u16[] = DECORS;

/** Its name, the one every locale shows. */
export function decorLabel(kind: u16): string {
  return DECOR_LABEL[kind] ?? "";
}

/** The drawer's grouping of decor, the shell's to choose. */
export type DecorSection = "nature" | "vehicle";

const DECOR_SECTION: Readonly<Record<number, DecorSection>> = {
  [decor.tree]: "nature",
  [decor.boat]: "vehicle",
  [decor.car]: "vehicle",
  [decor.train]: "vehicle",
};

export function decorSection(kind: u16): DecorSection {
  return DECOR_SECTION[kind];
}
