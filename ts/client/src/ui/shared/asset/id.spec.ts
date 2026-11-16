// @pxl/client/ui/shared/asset/id.spec.ts
//
// GENERATED from spec/spec/asset/id.json5 (xpute-gen data.rs)
//
// An asset's id is a u16: its kind over its index within that kind.
// `id >> IDX_BITS` is the kind, the low bits the index. The kind is what an
// asset is as a shape: a cube is the cell itself, decor a model with a size of
// its own, a marker flat to the camera. What stands in a cell of the world's
// grid is stored as its asset's id (pxl-spec object.rs).

export const IDX_BITS = 10 as number;

export type Kind = typeof KIND_CUBE | typeof KIND_DECOR | typeof KIND_MARKER;
export const KIND_CUBE = 0;
export const KIND_DECOR = 1;
export const KIND_MARKER = 2;

/** The member `val` is, or null for a number no member has. */
export function of_Kind(val: number): Kind | null {
  return val === KIND_CUBE || val === KIND_DECOR || val === KIND_MARKER
    ? val
    : null;
}

/** The member's name, as its source spells it. */
export function name_Kind(val: Kind): string {
  switch (val) {
    case KIND_CUBE:
      return "cube";
    case KIND_DECOR:
      return "decor";
    case KIND_MARKER:
      return "marker";
  }
}
