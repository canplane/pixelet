// @xpute/runtime/ipc/atomic.native.test.ts

import { expect, test, vi } from "vitest";

const words = vi.hoisted(() => new Uint32Array(8));

// The native module as an app has it, over a memory of eight words.
vi.mock("@xpute/native", () => ({
  XPUTE: {
    load: (at: number) => words[at / 4],
    store: (at: number, word: number) => void (words[at / 4] = word),
  },
}));

import { load_acquire, store_release } from "./atomic.native.ts";

test("atomic - a word is named by its place in the memory, past the view's own offset", () => {
  const view = new Uint32Array(words.buffer, 8, 4);
  store_release(view, 2, 0xffff_ffff);
  expect(words[4]).toBe(0xffff_ffff);
  expect(load_acquire(view, 2)).toBe(0xffff_ffff);
});
