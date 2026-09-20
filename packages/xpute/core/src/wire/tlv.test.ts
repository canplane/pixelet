// @xpute/core/wire/tlv.test.ts

/**
 * The half of xpute-core/wire/tlv.test.rs that runs here. TLV's bytes cross,
 * so the record is a claim about both languages and each side has to be held
 * to it: the Rust alone would only prove the Rust has not drifted.
 */

import { assertEquals } from "@std/assert";

import { Golden } from "@xpute/core/golden.ts";
import { Errno } from "@xpute/core/status/errno.spec.ts";
import { FaultError } from "@xpute/core/status/error.ts";
import { bytes_to_hex, hex_to_bytes } from "@xpute/core/codec/encoding.ts";
import { TlvReader, type TlvValue, TlvWriter } from "@xpute/core/wire/tlv.ts";

/** A read value as the record prints it: `typeof`, a colon, the value. */
function show(v: TlvValue): string {
  if (v instanceof Uint8Array) return `bytes:${bytes_to_hex(v)}`;
  return `${typeof v}:${v}`;
}

Deno.test("tlv computes what the typescript computes", async () => {
  const v = await Golden.load("wire/tlv.tsv");

  const w = new TlvWriter(4);
  w.u8(200).i8(-3).u16(65535).i16(-2).u32(4000000000).i32(-7);
  w.u64(1n << 63n).i64(-(1n << 40n)).f32(0.25).f64(-1.5).bool(true);
  w.str("한글").bytes(new Uint8Array([9, 8, 7]));
  w.write_all([true, 5, -2147483648, 2147483648, 9007199254740991, 1.5, NaN, -(1n << 63n), "s", new Uint8Array([1])]);
  const packet = w.finish();

  assertEquals(bytes_to_hex(packet), v.s("packet"));

  const read = new TlvReader(packet).read_all().map(show);
  const want: string[] = [];
  v.each("read", (k) => want.push(v.s(k)));
  assertEquals(read, want);

  // A length past what the packet holds: the reader refuses rather than
  // reading whatever follows.
  let huge = "ok";
  try {
    new TlvReader(hex_to_bytes(v.s("huge.packet"))).read_all();
  } catch (err) {
    huge = `errno:${(err as FaultError).errno ?? Errno.OK}`;
  }
  assertEquals(huge, v.s("huge.read"));
});
