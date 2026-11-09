// @pxl/client/ui/shared/asset/id.spec.ts
//
// GENERATED from spec/spec/asset/id.json
//
// An asset's id is a u16: its kind over its index within that kind.
// `id >> IDX_BITS` is the kind, the low bits the index. The kind is what an
// asset is as a shape: a cube is the cell itself, decor a model with a size of
// its own, a marker flat to the camera. What stands in a cell of the world's
// grid is stored as its asset's id (pxl-spec object.rs).

import type { u32 } from "@xpute/kit/abi/word.ts";
import * as KIND from "./id/kind.spec.ts";

export const IDX_BITS: u32 = 10 as number;

export { KIND };
