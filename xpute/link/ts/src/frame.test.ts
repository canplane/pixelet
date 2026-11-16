// @xpute/link/frame.test.ts

/** Holds the TypeScript frame to link/spec/golden/frame.tsv, the record
 * every language's frame must match. */

import { test } from "vitest";
import assert from "node:assert/strict";

import { Golden } from "@xpute/golden";
import { bytes_to_hex } from "@xpute/kit/codec/encoding.ts";
import { type Frame, frame, HDR_SZ, lay, read } from "./frame.ts";

test("each recorded frame's fields lay its words, and its words read back as its fields", async () => {
  const g = await Golden.load("link/spec/golden/frame.tsv");
  g.each("", (k) => {
    const f: Frame = {
      tag: g.u32(`${k}.tag`),
      res: g.bool(`${k}.res`),
      handle: g.u32(`${k}.handle`),
      method: g.u32(`${k}.method`),
      result: g.i32(`${k}.result`),
      pkt_len: g.u32(`${k}.pkt_len`),
    };
    const words = new Uint8Array(HDR_SZ);
    lay(new DataView(words.buffer), 0, f);
    assert.equal(bytes_to_hex(words), g.s(`${k}.words`), `${k}.words`);
    const words_view = new DataView(g.bytes(`${k}.words`).buffer);
    assert.deepEqual(read(words_view, 0, frame()), f, k);
  });
});
