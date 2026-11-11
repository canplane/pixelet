// @xpute/runtime/start.native.test.ts

import { test, vi } from "vitest";
import assert from "node:assert/strict";

import { MANIFEST } from "../../dev/example/manifest.spec.ts";
import * as batch from "./io/batch.spec.ts";
import * as frame from "./io/frame.ts";
import { QUIET } from "./runtime.ts";
import { FramePacing } from "./sched/frame_pacing.ts";
import { start } from "./start.native.ts";

/** The device's runtime as the UI reaches it (@xpute/native): what the UI
 * posts is kept, and a batch the test lays is handed over when told. */
const native = vi.hoisted(() => ({
  posted: [] as { tag: number; cmd: number; flags: number }[],
  down: new ArrayBuffer(0),
  notify: () => {},
}));

vi.mock("@xpute/native", () => ({
  XPUTE: {
    mem: () => new ArrayBuffer(4 << 20),
    post: (tag: number, cmd: number, flags: number) =>
      native.posted.push({ tag, cmd, flags }),
    take: () => native.down,
    addListener: (_event: string, on: () => void) => (native.notify = on),
    answer: () => {},
    callOn: () => {},
    pace: () => {},
    attach: () => {},
    pause: () => {},
    resume: () => {},
    wake: () => {},
    input: () => {},
    fsCompleteOpen: () => {},
  },
}));

/** A batch of one frame flagged RES, with no packet and `named` as the memory
 * it names (spec/io/batch.json). */
function reply(tag: number, cmd: number, named: number[]): ArrayBuffer {
  const pad = (n: number) => Math.ceil(n / batch.ALIGN) * batch.ALIGN;
  const down = new ArrayBuffer(4 * batch.HDR_WORDS + pad(named.length));
  const words = new Uint32Array(down, 0, batch.HDR_WORDS);
  words[batch.TAG] = tag;
  words[batch.CMD] = cmd;
  words[batch.FLAGS] = frame.RES;
  words[batch.NAMED] = named.length;
  new Uint8Array(down, 4 * batch.HDR_WORDS).set(named);
  return down;
}

test("start.native - a reply hands its request the memory it names", async () => {
  const pacing = new FramePacing({
    pace_hz: 60,
    margin_share: 0,
    batch_frames: 1,
    settle_ms: 0,
  });
  const link = await start(
    MANIFEST,
    () => ({ policy: pacing, submit: () => {} }),
    QUIET,
  );
  const answer = link.request(0x1001);
  const sent = native.posted.at(-1)!;
  assert.deepStrictEqual([sent.cmd, sent.flags], [0x1001, frame.ACKREQ]);
  native.down = reply(sent.tag, 0x1001, [7, 8, 9]);
  native.notify();
  const r = await answer;
  assert.deepStrictEqual([r.val, [...r.named]], [0, [7, 8, 9]]);
});
