// @xpute/gpu/host.frames.test.ts

/** The web host's frames unfinished, as it tells them in the device's block
 * (spec DEVICE): a turn whose stream took a surface's texture counts one,
 * and the queue's `onSubmittedWorkDone` asked after it takes it off. No
 * GPU: the device, its queue and the canvas are stand-ins, and each
 * `onSubmittedWorkDone` resolves only when the test resolves it. */

import { afterEach, test, vi } from "vitest";
import assert from "node:assert/strict";

import { op, STATUS } from "./api.spec.ts";
import { FEATURE_NAME, LIMIT_NAME } from "./enum.spec.ts";
import { GpuHost } from "./host.ts";

/** The stream at 0, its head the stream's own two words; the block past it. */
const HEAD = 2;
const BLOCK_AT = 256;
const BLOCK_WORDS = LIMIT_NAME.length + Math.ceil(FEATURE_NAME.length / 32) + 4;
const SURFACE = 1;

afterEach(() => {
  vi.unstubAllGlobals();
});

function setup() {
  vi.stubGlobal("navigator", {
    gpu: { getPreferredCanvasFormat: () => "bgra8unorm" },
  });
  const memory = new ArrayBuffer(4096);
  const words = new Uint32Array(memory);
  const done: (() => void)[] = [];
  const device = {
    lost: new Promise<never>(() => {}),
    limits: {},
    features: new Set<string>(),
    queue: {
      onSubmittedWorkDone: () =>
        new Promise<void>((resolve) => done.push(resolve)),
    },
  } as unknown as GPUDevice;
  const host = new GpuHost(
    device,
    () => memory,
    () => {},
  );
  const texture = { createView: () => ({}), format: "bgra8unorm" };
  host.bind(SURFACE, {
    getCurrentTexture: () => texture,
  } as unknown as GPUCanvasContext);
  /** A turn's records, each its op and words, run as the host runs one. */
  const turn = (...records: number[][]) => {
    let at = HEAD;
    for (const [code, ...args] of records) {
      words.set([code, args.length, ...args], at);
      at += 2 + args.length;
    }
    words[1] = HEAD;
    Atomics.store(words, 0, at - HEAD);
    host.run(0);
  };
  const block = (k: number) =>
    Atomics.load(words, BLOCK_AT / 4 + BLOCK_WORDS + k);
  return {
    turn,
    done,
    unfinished: () => block(-2),
    status: () => block(-1),
  };
}

/** Whatever the queue's promises have resolved, run. */
const settle = () => new Promise((r) => setTimeout(r, 0));

test("a frame drawn is unfinished until the queue's work is done", async () => {
  const { turn, done, unfinished, status } = setup();
  turn([op.DEVICE, BLOCK_AT, BLOCK_WORDS]);
  assert.equal(status(), STATUS.READY);
  assert.equal(unfinished(), 0, "the block's first frames unfinished");
  assert.equal(done.length, 0, "a turn that drew nothing counts none");

  turn([op.CURRENT, 10, SURFACE]);
  assert.equal(unfinished(), 1);
  turn([op.CURRENT, 11, SURFACE]);
  assert.equal(unfinished(), 2);
  turn();
  assert.equal(unfinished(), 2, "a turn with no records draws nothing");
  assert.equal(done.length, 2);

  done[0]();
  await settle();
  assert.equal(unfinished(), 1, "the first frame's work done");
  done[1]();
  await settle();
  assert.equal(unfinished(), 0);
});

test("a block asked late is told the frames already unfinished", async () => {
  const { turn, done, unfinished } = setup();
  turn([op.CURRENT, 10, SURFACE]);
  turn([op.DEVICE, BLOCK_AT, BLOCK_WORDS]);
  assert.equal(unfinished(), 1);
  done[0]();
  await settle();
  assert.equal(unfinished(), 0);
});
