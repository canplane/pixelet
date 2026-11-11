// @xpute/runtime/io/head.test.ts

import { test } from "vitest";
import assert from "node:assert/strict";

import { MarshalError } from "@xpute/kit/status/error.ts";
import { type Front, Head, laid, lay } from "./head.ts";

const FRONT: Front = { pkt_bytes: 256, stream_at: 4096 };

test("head - what the runtime lays it reads back", () => {
  const mem = new ArrayBuffer(2048);
  assert.equal(laid(mem, 0), false);
  lay(mem, 0, FRONT);
  assert.equal(laid(mem, 0), true);
  const head = new Head(mem, 0);
  assert.deepStrictEqual([head.pkt_bytes, head.stream_at], [256, 4096]);
});

test("head - a header that is not xpute's is refused", () => {
  const mem = new ArrayBuffer(2048);
  assert.throws(() => new Head(mem, 0), MarshalError);
});
