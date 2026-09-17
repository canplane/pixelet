// script/vector/pxl/shared/asset/ingested.ts
//
// The ingested table and the sheet, row for row, so the generated Rust
// table is held to the generated TypeScript one, for
// crates/pxl/shared/src/asset/ingested.test.rs.

import { INGESTED, SHEET } from "@pxl/shared/asset/ingested.ts";
import { f64_bits } from "../../../../vector.ts";

export function vectors() {
  return {
    rows: Object.entries(INGESTED).map(([path, a]) => ({ path, key: a.key, bytes: a.bytes, extent: a.extent ? a.extent.map(f64_bits) : "none" })),
    sheet: { key: SHEET.key, bytes: SHEET.bytes, cell: SHEET.cell, columns: SHEET.columns, tiles: SHEET.tiles },
  };
}
