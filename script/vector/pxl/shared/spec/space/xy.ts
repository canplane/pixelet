// script/vector/pxl/shared/spec/space/xy.ts
//
// TO_XY and the WITH_ setters, for crates/pxl/shared/src/spec/space/xy.test.rs.

import { CELL_CENTER_U, TO_XY, WITH_X, WITH_Y, WITH_Z } from "@pxl/shared/spec/space/xy.ts";
import { ivec } from "@pxl/shared/spec/space/pvf3/mod.ts";
import { f64_bits } from "../../../../../vector.ts";

export function vectors() {
  const words = [0n, ivec.gather(-1n, 2n, -3n), ivec.gather((1n << 23n) - 1n, -(1n << 23n), 32767n), 0xffff_ffff_ffff_ffffn];
  return {
    words: words.map((i) => ({ i, TO_XY: TO_XY(i), WITH_X: WITH_X(-7n, i), WITH_Y: WITH_Y(9n, i), WITH_Z: WITH_Z(-32768n, i) })),
    CELL_CENTER_U: f64_bits(CELL_CENTER_U(1)),
  };
}
