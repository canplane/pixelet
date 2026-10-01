// @xpute/core/codec/json.test.ts

/**
 * The host's end of golden/codec/json.tsv: which texts are JSON at all, as
 * the platform's own `JSON.parse` judges them. The guests' readers
 * (xpute-core's codec/json.rs and its C++ twin) are held to the same
 * judgments, so a document a guest reads is one the host would have read.
 */

import { assertEquals } from "@std/assert";

import { Golden } from "@xpute/core/golden.ts";

const utf8 = (hex: string): string => new TextDecoder().decode(Uint8Array.from(hex.match(/../g) ?? [], (b) => parseInt(b, 16)));

Deno.test("the host's JSON.parse takes what the record says is JSON", async () => {
  const v = await Golden.load("golden/codec/json.tsv");
  for (let k = 0; v.has(`grammar.${k}.text`); k++) {
    let got = "json";
    try {
      JSON.parse(utf8(v.s(`grammar.${k}.text`)));
    } catch {
      got = "refused";
    }
    assertEquals(got, v.s(`grammar.${k}.read`), `grammar.${k}`);
  }
});
