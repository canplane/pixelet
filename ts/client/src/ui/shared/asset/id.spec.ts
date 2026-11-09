// @pxl/client/ui/shared/asset/id.spec.ts
//
// GENERATED from spec/spec/asset/id.json
//
// An asset's id is a u16: its kind over which shape of that kind it is.
// `id >> SHAPE_BITS` is the kind, the low bits the shape. The kind is what an
// asset is drawn and struck as, and what its metadata is: a cube a cell the rays meet,
// with none, decor a model with a variant, heading and scale, a marker drawn
// flat to the camera from a design, as the pin someone puts up at a place,
// a character a person or a pet, a model with clips, and sky what hangs in the
// sky, the clouds. What stands in a cell of the world's grid is stored
// as its asset's id (pxl-spec object.rs); a character or the sky stands in
// no cell, so no cell ever holds either.

import type { u32 } from "@xpute/kit/abi/word.ts";
import * as KIND from "./id/kind.spec.ts";

export const SHAPE_BITS: u32 = 10 as number;

export { KIND };
