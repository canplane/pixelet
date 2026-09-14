// script/vector/pxl/shared/record/shipped.ts
//
// The shipped palette, row for row, and a hash of its packet, for
// crates/pxl/shared/src/record/shipped.test.rs.

import { encode_palette } from "@pxl/shared/record/palette.ts";
import { SHIPPED_PALETTE } from "@pxl/shared/record/shipped.ts";
import { fnv1a_64_hex } from "@xpute/core/kit/hash.ts";
import { f64_bits } from "../../../../vector.ts";

export function vectors() {
  return {
    packet_fnv: fnv1a_64_hex(encode_palette(SHIPPED_PALETTE)),
    rows: SHIPPED_PALETTE.map((r) => ({
      id: r.id,
      kind: r.kind,
      name: r.name,
      model_hash: r.model_hash,
      model_key: r.model_key || "none",
      skin_key: r.skin_key ?? "none",
      extent: r.extent.map(f64_bits),
      size: f64_bits(r.size),
      license: r.license || "none",
      author: r.author || "none",
      color: r.color ?? "none",
      group: r.group ?? "none",
    })),
  };
}
