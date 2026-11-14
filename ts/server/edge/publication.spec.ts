// @pxl/edge/publication.spec.ts
//
// GENERATED from spec/spec/bake/publication.json (pxl-tool spec/bake.rs)

/** The base layers, each a kind a baked page is published under. */
export const BASE_LAYERS = ["terrain", "place", "division", "earth"] as const;

export type BaseLayer =
  | typeof BASE_LAYER_TERRAIN
  | typeof BASE_LAYER_PLACE
  | typeof BASE_LAYER_DIVISION
  | typeof BASE_LAYER_EARTH;
export const BASE_LAYER_TERRAIN = 0;
export const BASE_LAYER_PLACE = 1;
export const BASE_LAYER_DIVISION = 2;
export const BASE_LAYER_EARTH = 3;

/** The member `val` is, or null for a number no member has. */
export function of_BaseLayer(val: number): BaseLayer | null {
  return val === BASE_LAYER_TERRAIN ||
    val === BASE_LAYER_PLACE ||
    val === BASE_LAYER_DIVISION ||
    val === BASE_LAYER_EARTH
    ? val
    : null;
}

/** A tile's object under its kind, `{version}/{name}`, as a pattern. */
export const TILE_OBJECT = String.raw`[^\/]+\/[0-3]+`;

/** The Earth's picture whole, in place of a tile's name under the earth
 * layer's version: the sphere's, no tile of the pyramid. */
export const TILE_SPHERE = "sphere";
