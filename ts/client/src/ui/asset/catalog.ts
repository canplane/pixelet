// @pxl/client/ui/asset/catalog.ts

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

/** An asset's number: its kind shifted left by SHAPE_BITS, with its shape in
 * the low bits. */
export type AssetId = u16;

/** The marker's kind has no other shape. */
export const MARKER: AssetId = KIND.marker << SHAPE_BITS;

export function kindOf(id: AssetId): number {
  return id >> SHAPE_BITS;
}

/** A cube is a ground palette entry or a placed color, and both are numbered
 * in one value space. */
export type Cube = u16;

export function cubeSection(cube: Cube): CubeSection {
  return CUBE_SECTION[cube];
}

export function cubeHex(cube: Cube): string | null {
  return CUBE_HEX[cube] ?? null;
}

/** Names are not translated: every locale shows the same one. */
export function cubeLabel(cube: Cube): string {
  return CUBE_LABEL[cube] ?? "";
}

/** Every cube: the placed colors first, then the palette entries in paint
 * order. */
export const allCubes: readonly Cube[] = CUBES;

export const decorIds: readonly u16[] = DECORS;

/** Names are not translated: every locale shows the same one. */
export function decorLabel(kind: u16): string {
  return DECOR_LABEL[kind] ?? "";
}

/** The UI chooses the drawer's sections of decor; the specs do not define
 * them. */
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
