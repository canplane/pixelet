// @pxl/client/shell/catalog/asset/decor.spec.ts
//
// GENERATED from spec/spec/asset/decor.json
//
// The decors, the cube catalog's sibling: drawn as a model, not a cell the rays meet.
//
// Numbered in the decor kind (id.json), so a placed leaf's value says it is
// decor: an object value is a u16.
//
// Each decor is said here once: its number, its models, and how the ground
// generates it on its own. A decor's `seed` names its scatter, so changing it
// reshuffles every one the ground generates; `density` is its mean candidates per
// cell at the cell's zoom, the intensity of the Poisson process that scatters
// it (pxl-spec-core scatter.rs); `on` the looks it may stand on; `orientation`
// a random heading, or along the road or rail its cell lies on. A variant's
// skin is its pack's colormap, kept outside the glb.
//
// A variant's `size` is the box it claims, in its decor's cells: every cell
// of it is the decor's. Its model is drawn as made, a glb unit a unit, its
// origin at the middle of the box's base, where it stands: no origin is
// moved, and glTF fixes none, so a model must be made with it there. It may
// reach a little past the box. A
// decor's cell is a unit, a z24 cell, `scale_steps` zooms coarser (finer
// where negative, as a marker's, marker.json), one for every variant,
// since a placement lands on the cell before its variant is drawn. The
// ground generates a decor only on its own looks, and no two decors share one, so
// two decors' boxes never meet.
//
// A decor thing carries `meta`, the fields its id gives it beside where it
// stands: which variant of its decor, its heading in quarter turns (an odd
// one swaps its box's sides) and its scale. The ground evaluates them from
// its cell; a placed one's are the placement's.
export const decor = {
  tree: 1024,
  boat: 1025,
  car: 1026,
  train: 1027,
} as const satisfies Record<string, number>;

export type DecorId = keyof typeof decor;
