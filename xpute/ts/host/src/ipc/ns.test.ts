// @xpute/host/ipc/ns.test.ts

import { assertEquals, assertThrows } from "@std/assert";

import type { U8Array } from "@xpute/kit/abi/array.ts";
import { Errno } from "@xpute/kit/status/errno.spec.ts";
import { Namespace } from "./ns.ts";
import { FileError } from "./sys.ts";

const signal = new AbortController().signal;

Deno.test("a root answers by what is bound there, and refuses as nothing bound or read-only", () => {
  const kept: [string, U8Array][] = [];
  const ns = new Namespace([
    [1, { open: (path) => new TextEncoder().encode(path) }],
    [2, {
      open: () => new Uint8Array(),
      commit: (path, bytes) => kept.push([path, bytes]),
    }],
  ]);
  assertEquals(
    new TextDecoder().decode(ns.open(1, "a/b", signal) as U8Array),
    "a/b",
  );
  ns.commit(2, "c", new Uint8Array([7]));
  assertEquals(kept, [["c", new Uint8Array([7])]]);
  const errnum = (f: () => unknown) =>
    (assertThrows(f, FileError) as FileError).errnum;
  assertEquals(errnum(() => ns.open(3, "a", signal)), Errno.ENOENT);
  assertEquals(errnum(() => ns.commit(1, "a", new Uint8Array())), Errno.EROFS);
});
