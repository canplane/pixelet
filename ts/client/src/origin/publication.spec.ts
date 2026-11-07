// @pxl/client/origin/publication.spec.ts
//
// GENERATED from spec/spec/bake/publication.json

/** The base layers by the number the core names one by: the root a bake's objects are opened below (xpute's OPENAT). */
export const BASE_LAYERS = ["terrain", "place", "division", "earth"] as const;

export type BaseLayer = (typeof BASE_LAYERS)[number];

/** The longest version a declaration names: one segment of a tile's URL, before its name (ts/client/src/origin/route.spec.ts `tileObject`). */
export const VERSION_BYTES = 32;

/** What a version is written in: one segment of a tile's URL, with no character it would have to escape. */
export const VERSION_CHARS =
  "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789._-";

/** A kind's object in a published set, the one place its segments are
 * ordered: the version its source declares, then the tile's name. A version
 * is a folder its tiles go up in whole (script/publish.py), so it comes
 * first. Under its kind it is a tile's address below the tile root
 * (route.json `TILE_ROOT`), and its key in the bucket. */
export function tileObject(version: string, name: string): string {
  return `${version}/${name}`;
}

/** The Earth's picture whole, in place of a tile's name under the earth
 * layer's version: the sphere's, no tile of the pyramid. */
export const TILE_SPHERE = "sphere";
