// @xpute/kit/wire/tlv.test.ts

/** Holds the TypeScript TLV to spec/golden/wire/tlv.tsv, the record every
 * language's TLV must match. */

import { test } from "vitest";
import assert from "node:assert/strict";

import { Golden } from "@xpute/golden";
import { ERRNO_OK } from "@xpute/kit/status/errno.spec.ts";
import { XputeError } from "@xpute/kit/status/error.ts";
import { bytes_to_hex, hex_to_bytes } from "@xpute/kit/codec/encoding.ts";
import { TlvReader, type TlvValue, TlvWriter } from "@xpute/kit/wire/tlv.ts";

function show(v: TlvValue): string {
  if (v instanceof Uint8Array) return `bytes:${bytes_to_hex(v)}`;
  return `${typeof v}:${v}`;
}

test("tlv computes what the typescript computes", async () => {
  const v = await Golden.load("kit/spec/golden/wire/tlv.tsv");

  const w = new TlvWriter(4);
  w.u8(200).i8(-3).u16(65535).i16(-2).u32(4000000000).i32(-7);
  w.u64(1n << 63n)
    .i64(-(1n << 40n))
    .f32(0.25)
    .f64(-1.5)
    .bool(true);
  w.str("€€").bytes(new Uint8Array([9, 8, 7]));
  w.write_all([
    true,
    5,
    -2147483648,
    2147483648,
    9007199254740991,
    1.5,
    NaN,
    -(1n << 63n),
    "s",
    new Uint8Array([1]),
  ]);
  const pkt = w.finish();

  assert.deepStrictEqual(bytes_to_hex(pkt), v.s("pkt"));

  const read = new TlvReader(pkt).read_all().map(show);
  const want: string[] = [];
  v.each("read", (k) => want.push(v.s(k)));
  assert.deepStrictEqual(read, want);

  // Each case gives the values read before the sequence stopped, then how it
  // stopped: END read, the input run out between two values, or a fault.
  for (let k = 0; v.has(`end.${k}.pkt`); k++) {
    const r = new TlvReader(hex_to_bytes(v.s(`end.${k}.pkt`)));
    let n = 0;
    let stop: string;
    try {
      for (const _ of r) n++;
      stop = r.end() ?? "none";
    } catch (err) {
      stop = `errno:${(err as XputeError).errnum ?? ERRNO_OK}`;
    }
    assert.deepStrictEqual(`${n}:${stop}`, v.s(`end.${k}.read`), `end.${k}`);
  }

  // A length larger than the rest of the packet: the reader refuses it
  // instead of reading whatever lies after the packet.
  let huge = "ok";
  try {
    new TlvReader(hex_to_bytes(v.s("huge.pkt"))).read_all();
  } catch (err) {
    huge = `errno:${(err as XputeError).errnum ?? ERRNO_OK}`;
  }
  assert.deepStrictEqual(huge, v.s("huge.read"));
});
