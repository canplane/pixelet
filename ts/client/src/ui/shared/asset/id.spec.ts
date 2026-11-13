// @pxl/client/ui/shared/asset/id.spec.ts
//
// GENERATED from spec/spec/asset/id.json (xpute-gen data.rs)
//
// An asset's id is a u16: its kind over its index within that kind.
// `id >> IDX_BITS` is the kind, the low bits the index. The kind is what an
// asset is as a shape: a cube is the cell itself, decor a model with a size of
// its own, a marker flat to the camera. What stands in a cell of the world's
// grid is stored as its asset's id (pxl-spec object.rs).

export const IDX_BITS = 10 as number;

export const KIND = {
  cube: 0 as number,
  decor: 1 as number,
  marker: 2 as number,
} as const;

export const Kind = {
  cube: 0,
  decor: 1,
  marker: 2,
} as const;
export type Kind = (typeof Kind)[keyof typeof Kind];

/** The name of the `Kind` member numbered `n`, or undefined for a number
 * no member has. */
export function kind_name(n: number): keyof typeof Kind | undefined {
  return (Object.keys(Kind) as (keyof typeof Kind)[]).find(
    (k) => Kind[k] === n,
  );
}

/** The `Kind` member `n` is, or null for a number no member has. */
export function kind_of(n: number): Kind | null {
  return kind_name(n) === undefined ? null : (n as Kind);
}
