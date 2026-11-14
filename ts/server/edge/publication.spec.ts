// @pxl/edge/publication.spec.ts
//
// GENERATED from spec/spec/bake/publication.json5 (pxl-tool spec/bake.rs)

/** The base layers, each a kind a baked page is published under. */
export const BASE_KINDS = ["terrain", "place", "division", "earth"] as const;

export type BaseKind =
  | typeof BASE_KIND_TERRAIN
  | typeof BASE_KIND_PLACE
  | typeof BASE_KIND_DIVISION
  | typeof BASE_KIND_EARTH;
export const BASE_KIND_TERRAIN = 0;
export const BASE_KIND_PLACE = 1;
export const BASE_KIND_DIVISION = 2;
export const BASE_KIND_EARTH = 3;

/** The member `val` is, or null for a number no member has. */
export function of_BaseKind(val: number): BaseKind | null {
  return val === BASE_KIND_TERRAIN ||
    val === BASE_KIND_PLACE ||
    val === BASE_KIND_DIVISION ||
    val === BASE_KIND_EARTH
    ? val
    : null;
}

/** A tile's object under its kind, `{version}/{name}`, as a pattern. */
export const TILE_OBJECT = String.raw`[^\/]+\/[0-3]+`;

/** The Earth's picture whole, in place of a tile's name under the earth
 * layer's version: the sphere's, no tile of the pyramid. */
export const TILE_SPHERE = "sphere";
