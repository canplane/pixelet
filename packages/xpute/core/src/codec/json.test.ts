// @xpute/core/codec/json.test.ts

/** golden/codec/json.tsv as `JSON.parse` judges it; the guests' readers are held to the same record. */

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
