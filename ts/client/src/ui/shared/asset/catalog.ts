// @pxl/client/ui/shared/asset/catalog.ts

import type { u16 } from "@xpute/kit/abi/word.ts";
import { CUBE_RGB } from "./color.spec.ts";
import { ground, object } from "@/abi/cube.spec.ts";
import {
  DECOR_BOAT,
  DECOR_CAR,
  DECOR_LABEL,
  DECOR_TRAIN,
  DECOR_TREE,
} from "./decor.spec.ts";
import { IDX_BITS, KIND_MARKER } from "./id.spec.ts";
import { hex } from "@/ui/shared/component/theme.ts";

/** An asset's number: its kind shifted left by `IDX_BITS`, with its index
 * in the low bits. */
export type AssetId = u16;

/** The marker's kind has no other index. */
export const MARKER: AssetId = KIND_MARKER << IDX_BITS;

export function kind_of(id: AssetId): number {
  return id >> IDX_BITS;
}

/** A cube is a ground cube or a placed color by its asset id, and both are
 * numbered in one value space. */
export type Cube = u16;

/** The placeable cubes, each its number, its section and its label: the
 * placed colors first, then the ground's cubes in paint order. */
const CUBE_RECS = [...object, ...ground];

export const CUBES: readonly Cube[] = CUBE_RECS.map((c) => c.number);

/** The catalog's section a placeable cube is listed under, as Carto groups the
 * ground's cubes. */
export type CubeSection = (typeof CUBE_RECS)[number]["section"];

const CUBE_SECTION = new Map(CUBE_RECS.map((c) => [c.number, c.section]));
const CUBE_LABEL = new Map(CUBE_RECS.map((c) => [c.number, c.label]));

export function cube_section(cube: Cube): CubeSection {
  return CUBE_SECTION.get(cube) as CubeSection;
}

export function cube_hex(cube: Cube): string | null {
  const rgb = CUBE_RGB[cube];
  return rgb === undefined ? null : hex(rgb);
}

/** Names are not translated: every locale shows the same one. */
export function cube_label(cube: Cube): string {
  return CUBE_LABEL.get(cube) ?? "";
}

/** Names are not translated: every locale shows the same one. */
export function decor_label(id: AssetId): string {
  return DECOR_LABEL[id] ?? "";
}

/** The UI chooses the drawer's sections of decor; the specs do not define
 * them. */
export type DecorSection = "nature" | "vehicle";

const DECOR_SECTION: Readonly<Record<number, DecorSection>> = {
  [DECOR_TREE]: "nature",
  [DECOR_BOAT]: "vehicle",
  [DECOR_CAR]: "vehicle",
  [DECOR_TRAIN]: "vehicle",
};

export function decor_section(id: AssetId): DecorSection {
  return DECOR_SECTION[id];
}
