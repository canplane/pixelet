// script/vector/pxl/shared/record/placement.ts
//
// Placement records with and without their ancillary columns, as the
// TypeScript writes them and reads them back, for
// crates/pxl/shared/src/record/placement.test.rs.

import { encode_placements, type Placement, PlacementTable } from "@pxl/shared/record/placement.ts";
import { bytes_to_hex } from "@xpute/core/kit/encoding.ts";
import { f64_bits } from "../../../../vector.ts";

export function vectors() {
  const bare: Placement[] = [{ id: 1n, x: 0.1, y: -2.5, z: 1e9, size: 0.25, heading: Math.PI, value: 228 }];
  const full: Placement[] = [
    ...bare,
    { id: 0xffffffffffffffffn, x: 3, y: 4, z: 5, size: 2, heading: -1, value: 7, scale: 1.1, rotation: [0.1, 0.2, 0.3, 0.9], tags: { a: "b", "1": "one" } },
    { id: 9n, x: 0, y: 0, z: 0, size: 1, heading: 0, value: 0, tags: { gone: undefined } },
  ];
  const read = (rows: Placement[]) => {
    const t = new PlacementTable(encode_placements(rows));
    return Array.from({ length: t.count }, (_, i) => {
      const r = t.row(i);
      return {
        x: f64_bits(r.x),
        heading: f64_bits(r.heading),
        scale: f64_bits(t.scale_of(i)),
        rotation: r.rotation ? r.rotation.map(f64_bits) : "none",
        tags: r.tags ? JSON.stringify(r.tags) : "none",
      };
    });
  };
  return { bare: bytes_to_hex(encode_placements(bare)), full: bytes_to_hex(encode_placements(full)), read_bare: read(bare), read_full: read(full) };
}
