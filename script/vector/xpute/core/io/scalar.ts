// script/vector/xpute/core/io/scalar.ts
//
// A stream written with every scalar writer, and the 96/128 packs, for
// crates/xpute/core/src/io/scalar.test.rs.

import * as s from "@xpute/core/io/scalar.ts";
import { bytes_to_hex } from "@xpute/core/kit/encoding.ts";

export function vectors() {
  const buf = new Uint8Array(64);
  const st = s.stream(buf);
  s.putu8(st, 0xab);
  s.puti8(st, -2);
  s.putu16(st, 0xbeef);
  s.puti16(st, -12345);
  s.putu32(st, 0xdeadbeef);
  s.puti32(st, -123456789);
  s.putu64(st, 0x0123456789abcdefn);
  s.puti64(st, -9876543210n);
  s.putf32(st, 1.1);
  s.putf64(st, -0.1);
  s.putu96(st, 1, 2, 0xffffffff);
  return {
    written: bytes_to_hex(buf),
    tell: s.tell(st),
    pack96: s.pack96(1, 2, 3),
    unpack96: s.unpack96((1n << 90n) + 12345n),
    pack128: s.pack128(0xffffffffffffffffn, 7n),
    unpack128: s.unpack128((5n << 64n) + 9n),
  };
}
