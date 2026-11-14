// @xpute/kit/codec/encoding.test.ts

/** The wire tests read their golden records through this hex codec. */

import { test } from "vitest";
import assert from "node:assert/strict";

import {
  b64url_to_bytes,
  b64url_to_uuid,
  bytes_to_b64url,
  bytes_to_hex,
  ct_eq,
  generate_uuid,
  hex_to_bytes,
  is_b64url,
  uuid_equals,
  uuid_to_b64url,
} from "@xpute/kit/codec/encoding.ts";
import { XputeError } from "@xpute/kit/status/error.ts";

const bytes = (...b: number[]) => new Uint8Array(b);

test("hex round trips, pads a byte under 0x10, and reads no half a pair", () => {
  assert.deepStrictEqual(
    bytes_to_hex(bytes(0, 1, 15, 16, 127, 128, 255)),
    "00010f107f80ff",
  );
  assert.deepStrictEqual(
    Array.from(hex_to_bytes("00010f107f80ff")),
    [0, 1, 15, 16, 127, 128, 255],
  );
  assert.deepStrictEqual(bytes_to_hex(bytes()), "");
  assert.deepStrictEqual(hex_to_bytes("").length, 0);
  // Either case is read, and encoding the bytes again gives the lower case the
  // encoder writes.
  assert.deepStrictEqual(
    Array.from(hex_to_bytes("AbCdEf")),
    [0xab, 0xcd, 0xef],
  );
  assert.deepStrictEqual(bytes_to_hex(hex_to_bytes("AbCdEf")), "abcdef");
});

test("hex refuses what it cannot read whole rather than taking what it can", () => {
  // A decoder built on `parseInt` alone would read "1g" as 1 and "zz" as NaN.
  assert.throws(() => hex_to_bytes("0"), XputeError);
  assert.throws(() => hex_to_bytes("abc"), XputeError);
  assert.throws(() => hex_to_bytes("1g"), XputeError);
  assert.throws(() => hex_to_bytes("zz"), XputeError);
  assert.throws(() => hex_to_bytes("00 11"), Error);
});

test("base64url round trips and carries no padding or plus or slash", () => {
  for (const n of [0, 1, 2, 3, 15, 16, 17, 64]) {
    const b = bytes(
      ...Array.from({ length: n }, (_, i) => (i * 37 + 11) & 0xff),
    );
    const s = bytes_to_b64url(b);
    assert.deepStrictEqual(is_b64url(s), true, `${s} is not base64url`);
    assert.deepStrictEqual(
      Array.from(b64url_to_bytes(s)),
      Array.from(b),
      `${n} bytes did not round trip`,
    );
  }
  // Base64url exists to avoid these three characters, which a URL or a
  // bucket key cannot carry as they are.
  assert.deepStrictEqual(is_b64url("a+b"), false);
  assert.deepStrictEqual(is_b64url("a/b"), false);
  assert.deepStrictEqual(is_b64url("ab=="), false);
  assert.deepStrictEqual(is_b64url("a-b_C9"), true);
});

test("a uuid is 16 bytes, 22 characters, and its own", () => {
  const a = generate_uuid();
  const b = generate_uuid();
  assert.deepStrictEqual(a.length, 16);
  assert.deepStrictEqual(uuid_to_b64url(a).length, 22);
  assert.deepStrictEqual(uuid_equals(a, a), true);
  assert.deepStrictEqual(
    uuid_equals(a, b),
    false,
    "two uuids came out the same",
  );
  assert.deepStrictEqual(
    Array.from(b64url_to_uuid(uuid_to_b64url(a))),
    Array.from(a),
  );
});

test("ct_eq answers on the bytes and not on where they first differ", () => {
  assert.deepStrictEqual(ct_eq(bytes(1, 2, 3), bytes(1, 2, 3)), true);
  assert.deepStrictEqual(ct_eq(bytes(1, 2, 3), bytes(1, 2, 4)), false);
  assert.deepStrictEqual(ct_eq(bytes(9, 2, 3), bytes(1, 2, 3)), false);
  assert.deepStrictEqual(
    ct_eq(bytes(1, 2), bytes(1, 2, 3)),
    false,
    "a prefix compared equal",
  );
  assert.deepStrictEqual(ct_eq(bytes(), bytes()), true);
});
