// @xpute/spec/rpc/rid.ts

import { U16 } from "@xpute/spec/lib/kit/word.ts";
import type { Bytes } from "@xpute/spec/lib/kit/array.ts";
import { type Base64URL, bytes_to_b64url } from "@xpute/spec/lib/kit/encoding.ts";
import { fnv1a_64_bytes, fnv1a_64_final, fnv1a_64_init, fnv1a_64_u16le } from "@xpute/spec/lib/kit/hash.ts";

import type { Vector } from "@xpute/spec/ipc/frame.ts";

/**
 * RID: Request identity hash used for request matching/deduplication.
 *
 * Format: 11-char base64url string
 * Hash: FNV-1a 64-bit (8 bytes)
 */
export type RID = Base64URL;

/**
 * Generate RID from (vec, tlv).
 *
 * Contract:
 * - tlv is the TLV stream for the request identity payload.
 * - reply payload MUST NOT participate in RID derivation.
 * - It SHOULD be exactly the request-side TLV argument bytes used for RID derivation.
 * - vec is mixed as 2 bytes little-endian (no TLV overhead).
 */
export function generate_rid(vec: Vector, tlv: Bytes): RID {
  const h = fnv1a_64_init();

  // vec as LE u16
  fnv1a_64_u16le(h, U16(vec));
  fnv1a_64_bytes(h, tlv);

  const out = fnv1a_64_final(h);
  return bytes_to_b64url(out);
}
