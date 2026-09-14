// script/vector/xpute/core/kit/encoding.ts
//
// base64url, hex and the UTF-8 codec, for
// crates/xpute/core/src/kit/encoding.test.rs.

import { b64url_to_bytes, bytes_to_b64url, bytes_to_hex, hex_to_bytes, is_b64url, td, te } from "@xpute/core/kit/encoding.ts";

export function vectors() {
  const blobs = [[], [0], [255], [1, 2], [1, 2, 3], [0xfb, 0xff, 0xbf], Array.from({ length: 64 }, (_, i) => (i * 97 + 13) & 255)].map((a) => new Uint8Array(a));
  const texts = ["", "abc", "한글", "🌏", "a b"];
  return {
    blobs: blobs.map((b) => {
      const s = bytes_to_b64url(b);
      return { hex: bytes_to_hex(b), b64url: s, back: bytes_to_hex(b64url_to_bytes(s)), is: is_b64url(s), hex_back: bytes_to_hex(hex_to_bytes(bytes_to_hex(b))) };
    }),
    texts: texts.map((t) => ({ t, utf8: bytes_to_hex(te.encode(t)), back: td.decode(te.encode(t)) === t })),
    invalid: ["ab+c", "ab/c", "ab=", "ok-_"].map((s) => ({ s, is: is_b64url(s) })),
  };
}
