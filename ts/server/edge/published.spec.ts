// @pxl/edge/published.spec.ts
//
// GENERATED from spec/spec/bake/published.json

/** The base layers, each a kind a baked page is published under. */
export const BASE_LAYERS = ["terrain", "place", "division", "earth"] as const;

/** A tile's object under its kind, `{version}/{name}`, as a pattern. */
export const TILE_OBJECT = String.raw`[^\/]+\/[0-3]+`;

/** The Earth's picture whole, in place of a tile's name under the earth
 * layer's version: the sphere's, no tile of the pyramid. */
export const TILE_SPHERE = "sphere";
