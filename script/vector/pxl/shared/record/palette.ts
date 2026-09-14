// script/vector/pxl/shared/record/palette.ts
//
// A palette record with optional columns empty and set, as the TypeScript
// writes it, for crates/pxl/shared/src/record/palette.test.rs.

import { encode_palette, type PaletteEntry, PaletteTable } from "@pxl/shared/record/palette.ts";
import { bytes_to_hex } from "@xpute/core/kit/encoding.ts";
import { f64_bits } from "../../../../vector.ts";

export function vectors() {
  const rows: PaletteEntry[] = [
    { id: 1, kind: "cube", name: "white", model_hash: 0n, model_key: "", extent: [1, 1, 1], size: 1, license: "", author: "", color: "#ffffff" },
    {
      id: 0x1000,
      kind: "decor",
      name: "tree:pine",
      model_hash: 0xfeedn,
      model_key: "asset/x",
      skin_key: "asset/y",
      extent: [1.18, 1.33, 2.3],
      size: 0.25,
      license: "CC0-1.0",
      author: "Kenney",
      group: "tree",
    },
    { id: 0x2000, kind: "actor", name: "cat", model_hash: 1n, model_key: "asset/z", extent: [NaN, 2, 1], size: 1, license: "", author: "" },
  ];
  const pkt = encode_palette(rows);
  const p = new PaletteTable(pkt);
  return {
    packet: bytes_to_hex(pkt),
    extent_max: rows.map((_, i) => f64_bits(p.extent_max(i))),
    entries: rows.map((_, i) => {
      const e = p.entry(i);
      return { kind: e.kind, skin_key: e.skin_key ?? "none", color: e.color ?? "none", group: e.group ?? "none", extent: e.extent.map(f64_bits) };
    }),
  };
}
