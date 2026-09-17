// script/vector/pxl/shared/asset/texture.ts
//
// The texture key and sheet tile of every cube type on both faces, for
// crates/pxl/shared/src/asset/texture.test.rs.

import { SHIPPED_SHEET, shipped_texture_key, shipped_texture_tile } from "@pxl/shared/asset/texture.ts";
import { cube_types } from "@pxl/shared/spec/cell/cube.ts";

export function vectors() {
  const out = [];
  for (const [channel, types] of Object.entries(cube_types)) {
    for (const name of Object.keys(types)) {
      for (const face of ["top", "side"] as const) {
        const tile = shipped_texture_tile(channel, name, face);
        out.push({ channel, name, face, key: shipped_texture_key(channel, name, face) ?? "none", tile: tile ? `${tile.x},${tile.y}` : "none" });
      }
    }
  }
  return { sheet: { key: SHIPPED_SHEET.key, cell: SHIPPED_SHEET.cell, columns: SHIPPED_SHEET.columns }, faces: out };
}
