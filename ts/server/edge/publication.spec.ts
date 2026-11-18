// @pxl/edge/publication.spec.ts
//
// GENERATED from spec/spec/bake/publication.json5 (pxl-tool spec/bake.rs)

/** The base layers, each a kind a baked page is published under. */
export const BASE_KINDS = ["terrain", "place", "division", "imagery"] as const;

export type BaseKind =
  | typeof BASE_KIND_TERRAIN
  | typeof BASE_KIND_PLACE
  | typeof BASE_KIND_DIVISION
  | typeof BASE_KIND_IMAGERY;
export const BASE_KIND_TERRAIN = 0;
export const BASE_KIND_PLACE = 1;
export const BASE_KIND_DIVISION = 2;
export const BASE_KIND_IMAGERY = 3;

/** The member `val` is, or null for a number no member has. */
export function of_BaseKind(val: number): BaseKind | null {
  return val === BASE_KIND_TERRAIN ||
    val === BASE_KIND_PLACE ||
    val === BASE_KIND_DIVISION ||
    val === BASE_KIND_IMAGERY
    ? val
    : null;
}

/** The member's name, as its source spells it. */
export function name_BaseKind(val: BaseKind): string {
  switch (val) {
    case BASE_KIND_TERRAIN:
      return "terrain";
    case BASE_KIND_PLACE:
      return "place";
    case BASE_KIND_DIVISION:
      return "division";
    case BASE_KIND_IMAGERY:
      return "imagery";
  }
}

/** A tile's object under its kind, `{version}/{name}`, as a pattern. */
export const TILE_OBJECT = String.raw`[^\/]+\/[0-3]+`;

/** The imagery's global image, in place of a tile's name under the imagery
 * layer's version: no tile of the pyramid. */
export const TILE_GLOBAL = "global";
