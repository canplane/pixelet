// script/vector/xpute/core/io/marshal/tlv.ts
//
// What TLVWriter writes for its explicit writers and write_all's
// dispatch, and what TLVReader reads back, for
// crates/xpute/core/src/io/marshal/tlv.test.rs.

import { TLVReader, TLVWriter } from "@xpute/core/io/marshal/tlv.ts";
import { bytes_to_hex } from "@xpute/core/kit/encoding.ts";

export function vectors() {
  const w = new TLVWriter(4);
  w.u8(200).i8(-3).u16(65535).i16(-2).u32(4000000000).i32(-7).u64(1n << 63n).i64(-(1n << 40n)).f32(0.25).f64(-1.5).bool(true).str("한글").bytes(new Uint8Array([9, 8, 7]));
  w.write_all([true, 5, -2147483648, 2147483648, 9007199254740991, 1.5, NaN, -(2n ** 63n), "s", new Uint8Array([1])]);
  const packet = w.finish();
  const read = new TLVReader(packet).read_all().map((v) => {
    if (v instanceof Uint8Array) return `bytes:${bytes_to_hex(v)}`;
    if (typeof v === "bigint") return `bigint:${v}`;
    if (typeof v === "number") return `number:${Number.isNaN(v) ? "NaN" : v}`;
    return `${typeof v}:${v}`;
  });
  return { packet: bytes_to_hex(packet), read };
}
