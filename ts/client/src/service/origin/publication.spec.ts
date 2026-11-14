// @pxl/client/service/origin/publication.spec.ts
//
// GENERATED from spec/spec/bake/publication.json (xpute-gen data.rs)
//
// The format each kind is baked and read in, the code's. Which version of
// each is served is the bucket's alone: its `source`, which the client reads
// at boot and script/publish.py writes, so a publish changes nothing here.

/** The format each bake writes now, which a published name begins with.
 * It is bumped by a change to how the client reads a tile, or to what a
 * tile guarantees it. */
export const FORMAT = {
  terrain: 12 as number,
  place: 10 as number,
  division: 9 as number,
  earth: 2 as number,
} as const;

/** What a version is written in: one segment of a tile's URL, with no character it would
 * have to escape. */
export const VERSION_CHARS =
  "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789._-";

/** The longest version a declaration may name, in bytes: one segment of a tile's URL,
 * before its name (pxl-spec bake/source.rs `is_version`). */
export const VERSION_BYTES = 32 as number;

/** The Earth's picture whole, in place of a tile's name under the earth
 * layer's version: the sphere's, no tile of the pyramid. */
export const TILE_SPHERE = "sphere";

/** The base layers a version is published for: the kinds of `FORMAT`, numbered in their order. */
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

/** A kind's object in a published set, the one place its segments are
 * ordered: the version its source declares, then the tile's name. A version
 * is a folder its tiles go up in whole (script/publish.py), so it comes
 * first. Under its kind it is a tile's address below the tile root
 * (route.json `TILE_ROOT`), and its key in the bucket. */
export function tileObject(version: string, name: string): string {
  return `${version}/${name}`;
}
