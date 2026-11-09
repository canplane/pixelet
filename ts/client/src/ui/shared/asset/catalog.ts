// @pxl/client/ui/shared/asset/catalog.ts

import type { u16 } from "@xpute/kit/abi/word.ts";
import { CUBE_HEX } from "./color.spec.ts";
import { CUBE_LABEL, CUBE_SECTION, type CubeSection } from "./cube.spec.ts";
import { decor, DECOR_LABEL } from "./decor.spec.ts";
import { IDX_BITS, KIND } from "./id.spec.ts";

/** An asset's number: its kind shifted left by IDX_BITS, with its index in
 * the low bits. */
export type AssetId = u16;

/** The marker's kind has no other index. */
export const MARKER: AssetId = KIND.marker << IDX_BITS;

export function kindOf(id: AssetId): number {
  return id >> IDX_BITS;
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

/** Names are not translated: every locale shows the same one. */
export function decorLabel(id: AssetId): string {
  return DECOR_LABEL[id] ?? "";
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

export function decorSection(id: AssetId): DecorSection {
  return DECOR_SECTION[id];
}
