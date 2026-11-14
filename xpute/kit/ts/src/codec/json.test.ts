// @xpute/kit/codec/json.test.ts

/**
 * Holds golden/codec/json.tsv to what the runtime's `JSON.parse` accepts. The
 * Rust and C++ readers are held to the same record, so all three accept the
 * same texts.
 */

import { test } from "vitest";
import assert from "node:assert/strict";

import { Golden } from "@xpute/golden";

const utf8 = (hex: string): string =>
  new TextDecoder().decode(
    Uint8Array.from(hex.match(/../g) ?? [], (b) => parseInt(b, 16)),
  );

test("the runtime's JSON.parse takes what the record says is JSON", async () => {
  const v = await Golden.load("kit/spec/golden/codec/json.tsv");
  for (let k = 0; v.has(`grammar.${k}.text`); k++) {
    let got = "json";
    try {
      JSON.parse(utf8(v.s(`grammar.${k}.text`)));
    } catch {
      got = "refused";
    }
    assert.deepStrictEqual(got, v.s(`grammar.${k}.read`), `grammar.${k}`);
  }
});

test("the runtime's JSON.parse reads a number past the doubles as an infinity or a zero", async () => {
  const v = await Golden.load("kit/spec/golden/codec/json.tsv");
  for (let k = 0; v.has(`number.${k}.text`); k++) {
    const bits = new BigUint64Array(
      new Float64Array([JSON.parse(v.s(`number.${k}.text`))]).buffer,
    )[0];
    assert.deepStrictEqual(
      bits.toString(16).padStart(16, "0"),
      v.s(`number.${k}.bits`),
      `number.${k}`,
    );
  }
});
