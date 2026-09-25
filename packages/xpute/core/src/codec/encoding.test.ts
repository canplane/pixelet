// @xpute/core/codec/encoding.test.ts

/**
 * The hex here is what the wire tests read a record through, so a fault in it
 * would let the three of them pass on the wrong bytes. That is the reason
 * these exist and the reason they check the refusals as closely as the
 * round trips: a decoder that quietly took what it could not read would turn
 * a mismatch into a match.
 */

import { assertEquals, assertThrows } from "@std/assert";

import { b64url_to_bytes, b64url_to_uuid, bytes_to_b64url, bytes_to_hex, ct_eq, generate_uuid, hex_to_bytes, is_b64url, uuid_equals, uuid_to_b64url } from "@xpute/core/codec/encoding.ts";
import { MarshalError } from "@xpute/core/status/error.ts";

const bytes = (...b: number[]) => new Uint8Array(b);

Deno.test("hex round trips, pads a byte under 0x10, and reads no half a pair", () => {
  assertEquals(bytes_to_hex(bytes(0, 1, 15, 16, 127, 128, 255)), "00010f107f80ff");
  assertEquals(Array.from(hex_to_bytes("00010f107f80ff")), [0, 1, 15, 16, 127, 128, 255]);
  assertEquals(bytes_to_hex(bytes()), "");
  assertEquals(hex_to_bytes("").length, 0);
  // Either case reads, and what comes back is the one the encoder writes.
  assertEquals(Array.from(hex_to_bytes("AbCdEf")), [0xab, 0xcd, 0xef]);
  assertEquals(bytes_to_hex(hex_to_bytes("AbCdEf")), "abcdef");
});

Deno.test("hex refuses what it cannot read whole rather than taking what it can", () => {
  // `parseInt` would read "1g" as 1 and "zz" as NaN, which is the fault these
  // guards exist for: a packet's length taken from half a pair reads as a
  // shorter packet instead of as an error.
  assertThrows(() => hex_to_bytes("0"), MarshalError);
  assertThrows(() => hex_to_bytes("abc"), MarshalError);
  assertThrows(() => hex_to_bytes("1g"), MarshalError);
  assertThrows(() => hex_to_bytes("zz"), MarshalError);
  assertThrows(() => hex_to_bytes("00 11"), Error);
});

Deno.test("base64url round trips and carries no padding or plus or slash", () => {
  for (const n of [0, 1, 2, 3, 15, 16, 17, 64]) {
    const b = bytes(...Array.from({ length: n }, (_, i) => (i * 37 + 11) & 0xff));
    const s = bytes_to_b64url(b);
    assertEquals(is_b64url(s), true, `${s} is not base64url`);
    assertEquals(Array.from(b64url_to_bytes(s)), Array.from(b), `${n} bytes did not round trip`);
  }
  // The three characters base64url exists to avoid, none of which a URL or a
  // bucket key may carry.
  assertEquals(is_b64url("a+b"), false);
  assertEquals(is_b64url("a/b"), false);
  assertEquals(is_b64url("ab=="), false);
  assertEquals(is_b64url("a-b_C9"), true);
});

Deno.test("a uuid is 16 bytes, 22 characters, and its own", () => {
  const a = generate_uuid();
  const b = generate_uuid();
  assertEquals(a.length, 16);
  assertEquals(uuid_to_b64url(a).length, 22);
  assertEquals(uuid_equals(a, a), true);
  assertEquals(uuid_equals(a, b), false, "two uuids came out the same");
  assertEquals(Array.from(b64url_to_uuid(uuid_to_b64url(a))), Array.from(a));
});

Deno.test("ct_eq answers on the bytes and not on where they first differ", () => {
  assertEquals(ct_eq(bytes(1, 2, 3), bytes(1, 2, 3)), true);
  assertEquals(ct_eq(bytes(1, 2, 3), bytes(1, 2, 4)), false);
  // A difference in the first byte and in the last are the same answer; what
  // this guards is that they take the same path to it.
  assertEquals(ct_eq(bytes(9, 2, 3), bytes(1, 2, 3)), false);
  assertEquals(ct_eq(bytes(1, 2), bytes(1, 2, 3)), false, "a prefix compared equal");
  assertEquals(ct_eq(bytes(), bytes()), true);
});
