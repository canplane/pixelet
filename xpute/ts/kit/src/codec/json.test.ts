// @xpute/kit/codec/json.test.ts

/** golden/codec/json.tsv as `JSON.parse` judges it; the guests' readers are held to the same record. */

import { test } from "vitest";
import { assertEquals } from "@std/assert";

import { Golden } from "@xpute/kit/golden.ts";

const utf8 = (hex: string): string =>
  new TextDecoder().decode(
    Uint8Array.from(hex.match(/../g) ?? [], (b) => parseInt(b, 16)),
  );

test("the host's JSON.parse takes what the record says is JSON", async () => {
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

test("the host's JSON.parse reads a number past the doubles as an infinity or a zero", async () => {
  const v = await Golden.load("golden/codec/json.tsv");
  for (let k = 0; v.has(`number.${k}.text`); k++) {
    const bits = new BigUint64Array(
      new Float64Array([JSON.parse(v.s(`number.${k}.text`))]).buffer,
    )[0];
    assertEquals(
      bits.toString(16).padStart(16, "0"),
      v.s(`number.${k}.bits`),
      `number.${k}`,
    );
  }
});
