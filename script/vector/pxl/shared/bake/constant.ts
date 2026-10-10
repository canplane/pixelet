// script/vector/pxl/shared/bake/constant.ts
//
// The unit, grid and elevation constants and the object layout, for
// crates/pxl/shared/src/bake/constant.test.rs.

import * as c from "@pxl/shared/bake/constant.ts";
import { f64_bits } from "../../../../vector.ts";

export function vectors() {
  return {
    CELL_SHAMT: c.CELL_SHAMT,
    U_PER_CELL: f64_bits(c.U_PER_CELL),
    XY_CELL_PER_M: f64_bits(c.XY_CELL_PER_M),
    Z_CELL_PER_M: [0, 1234567.8, -4500000, 12000000].map((y) => f64_bits(c.Z_CELL_PER_M(y))),
    PAGE_CELL_SPAN: c.PAGE_CELL_SPAN,
    CHUNK_CELL_SPAN: c.CHUNK_CELL_SPAN,
    PAGE_ELE_SPAN: c.PAGE_ELE_SPAN,
    ELE_PAD: c.ELE_PAD,
    ELE_PADDED_SPAN: c.ELE_PADDED_SPAN,
    OBJECT_LAYOUT: c.OBJECT_LAYOUT,
  };
}
