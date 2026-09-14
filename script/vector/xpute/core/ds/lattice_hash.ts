// script/vector/xpute/core/ds/lattice_hash.ts
//
// The lattice hashes over signed addresses, for
// crates/xpute/core/src/ds/lattice_hash.test.rs.

import { lattice_hash3, lattice_hash4 } from "@xpute/core/ds/lattice_hash.ts";

export function vectors() {
  const vs = [0, 1, -1, 2147483647, -2147483648, 12345, -98765];
  return vs.flatMap((x) => vs.map((y) => ({ x, y, z: x ^ y, h3: lattice_hash3(x, y, x ^ y), h4: lattice_hash4(x, y, x ^ y, (x & 7) >>> 0) })));
}
