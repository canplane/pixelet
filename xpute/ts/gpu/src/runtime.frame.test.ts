// @xpute/gpu/runtime.frame.test.ts

/** These tests hold the web runtime's uncaptured errors and unfinished
 * frames, as it tells them in the device's block (spec DEVICE). A turn whose stream took a surface's
 * texture counts one, and the queue's `onSubmittedWorkDone`, asked after that
 * turn, takes it off. No GPU is used: the device, its queue and the canvas are
 * stand-ins, and each `onSubmittedWorkDone` resolves only when the test
 * resolves it. */

import { afterEach, test, vi } from "vitest";
import assert from "node:assert/strict";

import { op, STATUS } from "./api.spec.ts";
import {
  ERRORS_AT,
  STATUS_AT,
  UNFINISHED_AT,
  WORDS,
} from "./api/device.spec.ts";
import { GpuRuntime } from "./runtime.ts";
import { OWN_WORDS } from "@xpute/runtime/io/stream.spec.ts";

/** The stream is at 0, and its head is the stream's own words alone; the
 * block lies past it. */
const HEAD = OWN_WORDS;
const BLOCK_AT = 256;
const SURFACE = 1;

afterEach(() => {
  vi.unstubAllGlobals();
});

function setup(slow_ms: number | null = null) {
  vi.stubGlobal("navigator", {
    gpu: { getPreferredCanvasFormat: () => "bgra8unorm" },
  });
  vi.stubGlobal("GPUBufferUsage", { MAP_WRITE: 2, COPY_SRC: 4 });
  vi.stubGlobal("GPUMapMode", { WRITE: 2 });
  const mem = new ArrayBuffer(4096);
  const words = new Uint32Array(mem);
  const done: (() => void)[] = [];
  const errs: ((e: { error: { message: string } }) => void)[] = [];
  /** What reached the queue, in order: each copy a write staged, and each
   * submit as the count of command buffers it took. */
  const queued: string[] = [];
  const device = {
    lost: new Promise<never>(() => {}),
    addEventListener: (_: string, on: (typeof errs)[number]) => errs.push(on),
    limits: {},
    features: new Set<string>(),
    createBuffer: ({ size }: { size: number }) => ({
      getMappedRange: () => new ArrayBuffer(size),
      unmap: () => queued.push("unmap"),
      mapAsync: () => new Promise<void>(() => {}),
    }),
    createCommandEncoder: () => ({
      copyBufferToBuffer: (
        _s: unknown,
        at: number,
        _d: unknown,
        to: number,
        n: number,
      ) => queued.push(`copy ${at}->${to} ${n}`),
      finish: () => ({}),
    }),
    queue: {
      submit: (bufs: unknown[]) => queued.push(`submit ${bufs.length}`),
      onSubmittedWorkDone: () =>
        new Promise<void>((resolve) => done.push(resolve)),
    },
  } as unknown as GPUDevice;
  const runtime = new GpuRuntime(
    device,
    () => mem,
    () => {},
    { upload_chunk_bytes: 64, slow_ms },
  );
  const texture = { createView: () => ({}), format: "bgra8unorm" };
  runtime.bind(SURFACE, {
    getCurrentTexture: () => texture,
  } as unknown as GPUCanvasContext);
  /** Runs a turn's records, each given as its op and words, as the runtime
   * runs a turn. */
  const turn = (...records: number[][]) => {
    let at = HEAD;
    for (const [code, ...args] of records) {
      words.set([code, args.length, ...args], at);
      at += 2 + args.length;
    }
    words[1] = HEAD;
    Atomics.store(words, 0, at - HEAD);
    runtime.run(0);
  };
  const block = (k: number) => Atomics.load(words, BLOCK_AT / 4 + k);
  return {
    turn,
    raise: (message: string) =>
      errs.forEach((on) => on({ error: { message } })),
    done,
    queued,
    errors: () => block(ERRORS_AT),
    unfinished: () => block(UNFINISHED_AT),
    status: () => block(STATUS_AT),
  };
}

/** Lets whatever the queue's promises have resolved run. */
const settle = () => new Promise((r) => setTimeout(r, 0));

test("a frame drawn is unfinished until the queue's work is done", async () => {
  const { turn, done, unfinished, status } = setup();
  turn([op.DEVICE, BLOCK_AT, WORDS]);
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
  turn([op.DEVICE, BLOCK_AT, WORDS]);
  assert.equal(unfinished(), 1);
  done[0]();
  await settle();
  assert.equal(unfinished(), 0);
});

test("an uncaptured error is counted, before the block and after", () => {
  const { turn, raise, errors } = setup();
  const said = vi.spyOn(console, "log").mockImplementation(() => {});
  raise("before");
  turn([op.DEVICE, BLOCK_AT, WORDS]);
  assert.equal(errors(), 1);
  raise("after");
  assert.equal(errors(), 2);
  assert.equal(said.mock.calls.length, 2, "each logged");
  said.mockRestore();
});

test("a record or a run slower than the program's slow_ms is said with its time, and a quick one is not", () => {
  const SLOW = 1000;
  const { turn } = setup(SLOW);
  const said = vi.spyOn(console, "log").mockImplementation(() => {});
  const now = vi.spyOn(performance, "now");
  // The run's start, the record's start and end, the run's end.
  for (const t of [0, 0, SLOW + 5.5, SLOW + 7.5]) now.mockReturnValueOnce(t);
  turn([op.CURRENT, 10, SURFACE]);
  for (const t of [0, 0, SLOW, SLOW]) now.mockReturnValueOnce(t);
  turn([op.CURRENT, 11, SURFACE]);
  assert.deepEqual(said.mock.calls, [
    [`[gpu] op ${op.CURRENT} took ${SLOW + 5} ms`],
    [`[gpu] run took ${SLOW + 7} ms`],
  ]);
  now.mockRestore();
  said.mockRestore();
});

test("a buffer write is staged in a mapped chunk, a chunk's room at a time, and copied ahead of the turn's own work", () => {
  const { turn, queued } = setup();
  turn(
    [op.BUFFER, 5, 256, 0, 0],
    [op.WRITE_BUFFER, 5, 16, 8, 1, 2],
    [op.WRITE_BUFFER, 5, 0, 100, ...new Array<number>(25).fill(0)],
  );
  // 64-byte chunks: the second write fills the first chunk's rest from its
  // aligned 8 and takes a second chunk for the 44 bytes left. The run's end
  // unmaps both and submits the copies.
  assert.deepEqual(queued, [
    "copy 0->16 8",
    "copy 8->0 56",
    "copy 0->56 44",
    "unmap",
    "unmap",
    "submit 1",
  ]);
});

test("with no slow_ms no record is timed", () => {
  const { turn } = setup();
  const now = vi.spyOn(performance, "now");
  turn([op.CURRENT, 10, SURFACE]);
  assert.equal(now.mock.calls.length, 0);
  now.mockRestore();
});
