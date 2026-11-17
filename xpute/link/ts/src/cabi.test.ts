// @xpute/link/cabi.test.ts

/** Holds the TypeScript packet to link/spec/golden/cabi.tsv, the record
 * every language's Canonical ABI lowering and lifting must match. */

import { test } from "vitest";
import assert from "node:assert/strict";

import { Golden } from "@xpute/golden";
import type { U8Array } from "@xpute/kit/abi/array.ts";
import { bytes_to_hex } from "@xpute/kit/codec/encoding.ts";
import { ERRNO_EBADMSG } from "@xpute/kit/status/errno.spec.ts";
import { XputeError } from "@xpute/kit/status/error.ts";
import { load_Lent, store_Lent, type Token } from "./abi/types.ts";
import { Lift, Lower } from "./cabi.ts";
import { load_uri, store_uri } from "./link.ts";

/** The record's packet: one of each number, a string, a list of numbers, a
 * URI and a lent buffer, as the Canonical ABI lays a tuple of them. */
const FIXED_SZ = 88;

const URI: Token[] = [
  { tag: "int", val: 5 },
  { tag: "str", val: "f" },
];

function lower(buf: U8Array): number {
  const w = new Lower(buf, FIXED_SZ);
  w.bool(0, true);
  w.i8(1, -3);
  w.u16(2, 65535);
  w.i32(4, -7);
  w.u64(8, 1n << 63n);
  w.f32(16, 0.25);
  w.f64(24, -1.5);
  w.u8(32, 200);
  w.i16(34, -2);
  w.u32(36, 4000000000);
  w.i64(40, -(1n << 40n));
  w.str(48, "€€");
  w.arr(56, new Uint16Array([1, 2, 65535]));
  store_uri(w, 64, URI);
  store_Lent(w, 72, { buf: 4096n, nbyte: 64 });
  return w.finish();
}

test("the packet lowers as the record holds it, and only into a buffer that holds it whole", async () => {
  const want = (await Golden.load("link/spec/golden/cabi.tsv")).bytes("pkt");
  const buf = new Uint8Array(want.length).fill(0xaa);
  assert.equal(lower(buf), want.length);
  assert.equal(bytes_to_hex(buf), bytes_to_hex(want));
  assert.equal(lower(new Uint8Array(want.length - 1)), -1);
});

test("the record's packet lifts as the values it was lowered from", async () => {
  const r = new Lift(
    (await Golden.load("link/spec/golden/cabi.tsv")).bytes("pkt"),
  );
  assert.equal(r.bool(0), true);
  assert.equal(r.i8(1), -3);
  assert.equal(r.u16(2), 65535);
  assert.equal(r.i32(4), -7);
  assert.equal(r.u64(8), 1n << 63n);
  assert.equal(r.f32(16), 0.25);
  assert.equal(r.f64(24), -1.5);
  assert.equal(r.u8(32), 200);
  assert.equal(r.i16(34), -2);
  assert.equal(r.u32(36), 4000000000);
  assert.equal(r.i64(40), -(1n << 40n));
  assert.equal(r.str(48), "€€");
  assert.deepEqual(r.arr(56, Uint16Array), new Uint16Array([1, 2, 65535]));
  assert.deepEqual(load_uri(r, 64), URI);
  assert.deepEqual(load_Lent(r, 72), { buf: 4096n, nbyte: 64 });
});

test("each recorded packet that does not hold a URI is refused as one, EBADMSG", async () => {
  const g = await Golden.load("link/spec/golden/cabi.tsv");
  g.each("refused", (k) => {
    assert.throws(
      () => load_uri(new Lift(g.bytes(k)), 0),
      (e) => e instanceof XputeError && e.errnum === ERRNO_EBADMSG,
      k,
    );
  });
});
