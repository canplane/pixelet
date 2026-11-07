// @xpute/runtime/ipc/head.test.ts

import { test } from "vitest";
import assert from "node:assert/strict";

import { MarshalError } from "@xpute/kit/status/error.ts";
import { type Front, Head, laid, lay } from "./head.ts";
import { Ring } from "./ring.ts";

const FRONT: Front = {
  sq_cap: 4,
  cq_cap: 8,
  slot_bytes: 64,
  sq_at: 64,
  cq_at: 256,
  sq_slots_at: 512,
  cq_slots_at: 1024,
  gpu_at: 0,
};

test("head - what the runtime lays it reads back, and the rings open on it", () => {
  const mem = new ArrayBuffer(2048);
  assert.equal(laid(mem, 0), false);
  lay(mem, 0, FRONT);
  assert.equal(laid(mem, 0), true);
  const head = new Head(mem, 0);
  assert.deepStrictEqual(
    [head.sq_at, head.cq_at, head.gpu_at],
    [FRONT.sq_at, FRONT.cq_at, 0],
  );
  const sq = new Ring(mem, head.sq_at);
  assert.deepStrictEqual([sq.cap, sq.slot_bytes, sq.len], [4, 64, 0]);
});

test("head - a header that is not xpute's is refused", () => {
  const mem = new ArrayBuffer(2048);
  assert.throws(() => new Head(mem, 0), MarshalError);
});
