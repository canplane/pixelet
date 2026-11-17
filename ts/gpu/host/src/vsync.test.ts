// @pxl/gpu/host/src/vsync.test.ts

import { afterEach, test, vi } from "vitest";
import assert from "node:assert/strict";

import { mark } from "@xpute/host";
import { ERRNO_EINPROGRESS } from "@xpute/kit/status/errno.spec.ts";
import { Lift, Lower } from "@xpute/link/cabi.ts";
import type { Args, LinkObject, Side, Tag } from "@xpute/link/link.ts";
import { type Frame, load_Frame, NEXT } from "./abi/vsync.ts";
import { mount_vsync } from "./vsync.ts";

afterEach(() => vi.unstubAllGlobals());

/** The display's frames this many ms apart. */
const FRAME_MS = 1000 / 60;

/** The web vsync over a link that keeps each answer by its request's tag,
 * on animation frames the test runs by hand (`frame`). */
function mounted(): {
  wait: (tag: Tag, after: bigint) => void;
  frame: (k: number) => void;
  frame_cnt: () => number;
  answers: Map<Tag, Frame>;
} {
  const frames: ((timestamp: number) => void)[] = [];
  vi.stubGlobal("requestAnimationFrame", (f: (t: number) => void) =>
    frames.push(f),
  );
  const answers = new Map<Tag, Frame>();
  let object: LinkObject | undefined;
  const link = {
    mount_named: (_: number, o: LinkObject) => (object = o),
    task_return: (t: Tag, _: number, data: Args) => {
      const bytes = new Uint8Array(data![0]);
      const w = new Lower(bytes, data![0]);
      data![1](w);
      answers.set(t, load_Frame(new Lift(bytes), 0));
    },
    on_cancel: () => {},
  };
  mount_vsync(link as unknown as Side);
  return {
    wait: (tag, after) => {
      const args = new Uint8Array(8);
      new DataView(args.buffer).setBigUint64(0, after, true);
      const ret = new Lower(new Uint8Array(0), 0);
      const sent = object!.call(NEXT, tag, new Lift(args), ret);
      assert.equal(sent, ERRNO_EINPROGRESS);
    },
    frame: (k) => frames.shift()!(k * FRAME_MS),
    frame_cnt: () => frames.length,
    answers,
  };
}

/** Waits for the event loop's next pass, by when the frame's answers, laid
 * as microtasks after it, have landed. */
const next_pass = () => new Promise((done) => setTimeout(done));

test("a wait is answered at the first frame from its after, with the gap since the frame before", async () => {
  const v = mounted();
  v.wait(1, 0n);
  v.wait(2, mark(2.5 * FRAME_MS));
  v.frame(0);
  await next_pass();
  assert.deepEqual(v.answers.get(1), { at: mark(0), gap: 0n });
  assert.equal(v.answers.has(2), false);
  v.frame(1);
  v.frame(2);
  await next_pass();
  assert.equal(v.answers.has(2), false);
  v.frame(3);
  await next_pass();
  assert.deepEqual(v.answers.get(2), {
    at: mark(3 * FRAME_MS),
    gap: mark(3 * FRAME_MS) - mark(2 * FRAME_MS),
  });
});

test("the host sees every frame, so a wait met by its first frame still has the gap", async () => {
  const v = mounted();
  v.frame(0);
  v.frame(1);
  v.wait(1, 0n);
  v.frame(2);
  await next_pass();
  assert.deepEqual(v.answers.get(1), {
    at: mark(2 * FRAME_MS),
    gap: mark(2 * FRAME_MS) - mark(FRAME_MS),
  });
});

test("the host keeps one animation frame asked at a time, asking the next as each runs", () => {
  const v = mounted();
  for (let k = 0; k < 3; k++) {
    assert.equal(v.frame_cnt(), 1);
    v.frame(k);
  }
  assert.equal(v.frame_cnt(), 1);
});
