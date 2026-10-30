// @xpute/host/ipc/ns.test.ts

import { test } from "vitest";
import assert from "node:assert/strict";

import type { U8Array } from "@xpute/kit/abi/array.ts";
import { Errno } from "@xpute/kit/status/errno.spec.ts";
import { Namespace } from "./ns.ts";
import { FileError } from "./sys.ts";

const signal = new AbortController().signal;

test("a root answers by what is bound there, and refuses as nothing bound or read-only", () => {
  const kept: [string, U8Array][] = [];
  const ns = new Namespace([
    [1, { open: (path) => new TextEncoder().encode(path) }],
    [
      2,
      {
        open: () => new Uint8Array(),
        commit: (path, bytes) => kept.push([path, bytes]),
      },
    ],
  ]);
  assert.deepStrictEqual(
    new TextDecoder().decode(ns.open(1, "a/b", signal) as U8Array),
    "a/b",
  );
  ns.commit(2, "c", new Uint8Array([7]));
  assert.deepStrictEqual(kept, [["c", new Uint8Array([7])]]);
  const errnum = (f: () => unknown): number => {
    try {
      f();
    } catch (err) {
      assert.ok(err instanceof FileError);
      return err.errnum;
    }
    assert.fail("nothing was thrown");
  };
  assert.deepStrictEqual(
    errnum(() => ns.open(3, "a", signal)),
    Errno.ENOENT,
  );
  assert.deepStrictEqual(
    errnum(() => ns.commit(1, "a", new Uint8Array())),
    Errno.EROFS,
  );
});
