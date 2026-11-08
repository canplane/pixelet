// @pxl/client/service/sys/fs/voxel.spec.ts
//
// GENERATED from spec/spec/asset/cube.json
//
// One value space: a ground cell is a byte, a Carto palette entry numbered in the
// order Carto paints, and an object's value a u16 whose byte range is
// that palette and whose range past it is what only an object can be.

/** explicit air: retains this leaf and erases the layers under it; no ground cell can hold it, as it is past a byte */
export const AIR = 256;

/** Whether a value read into a word is a palette entry the palette gives out (pxl-content `Ground::grounds`). */
export function grounds(val: number): boolean {
  return (val >= 1 && val <= 55) || (val >= 64 && val <= 66);
}

/** Whether a value read into a word is one of the object's colors (pxl-content `Object::objects`). */
export function objects(val: number): boolean {
  return val >= 257 && val <= 271;
}
