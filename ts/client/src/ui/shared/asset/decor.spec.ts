// @pxl/client/ui/shared/asset/decor.spec.ts
//
// GENERATED from spec/spec/asset/decor.json (pxl-tool spec/asset.rs)
//
// The decors, the cube catalog's sibling: drawn as a model, not a cell the rays meet.
//
// A decor's id is its place in `decors`, over the decor kind (id.json). A
// placed one is stored as that id, so a decor already here keeps its place
// and a new one goes at the end.
//
// Each decor is said here once: its models, and how the ground generates it
// on its own. A decor's `seed` names its scatter, so changing it reshuffles
// every one the ground generates; `density` is the chance a cell of its holds
// one, the mean of a tree of hashes of the cells (pxl-content scatter.rs);
// `on` the palette entries it may stand on; `orient` a random heading, or
// along the road or rail its cell lies on. `packs` are the model packs it is
// drawn from, in the order its variants are numbered: a pack's `skin` is its
// one colormap, kept outside the glb, and its `variants` its models.
//
// A variant's `size` is the box it claims, in its decor's cells: every cell
// of it is the decor's. Its model is drawn as made, a glb unit a unit, its
// origin at the middle of the box's base, where it stands: no origin is
// moved, and glTF fixes none, so a model must be made with it there. It may
// reach a little past the box. A decor's cell is a unit, a z24 cell. The
// ground generates a decor only on its own palette entries, and no two decors share one, so
// two decors' boxes never meet.
//
// A decor thing carries `meta`, the fields its id gives it beside where it
// stands: which variant of its decor and its heading in quarter turns (an
// odd one swaps its box's sides). The ground evaluates them from its cell; a
// placed one's are the placement's.

import * as decor from "./decor/decor.spec.ts";

export { decor };

/** The decors' ids, in the table's order. */
export const DECORS: readonly number[] = [1024, 1025, 1026, 1027];

/** Each decor's name, the one every locale shows: an item has one name. */
export const DECOR_LABEL: Readonly<Record<number, string>> = {
  1024: "Tree",
  1025: "Boat",
  1026: "Car",
  1027: "Train",
};
