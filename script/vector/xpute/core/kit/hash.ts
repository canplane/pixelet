// script/vector/xpute/core/kit/hash.ts
//
// FNV over strings (surrogate pairs included) and bytes, for
// crates/xpute/core/src/kit/hash.test.rs.

import { fnv1a64_str, fnv1a_64_hex } from "@xpute/core/kit/hash.ts";
import { bytes_to_hex } from "@xpute/core/kit/encoding.ts";

export function vectors() {
  const strings = ["", "a", "pixelet", "한글 text", "emoji 🌏🧱", "long ".repeat(40)];
  const bytes = [[], [0], [1, 2, 3, 4, 5, 250, 251], Array.from({ length: 300 }, (_, i) => (i * 37) & 255)].map((a) => new Uint8Array(a));
  return {
    strings: strings.map((s) => ({ s, fnv: fnv1a64_str(s) })),
    bytes: bytes.map((b) => ({ b: bytes_to_hex(b), hex: fnv1a_64_hex(b) })),
  };
}
