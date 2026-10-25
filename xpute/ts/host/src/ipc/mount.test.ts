// @xpute/host/ipc/mount.test.ts

import { assertEquals, assertThrows } from "@std/assert";

import type { U8Array } from "@xpute/kit/abi/array.ts";
import { Errno } from "@xpute/kit/status/errno.spec.ts";
import { MountTable } from "./mount.ts";
import { FileError } from "./sys.ts";

const signal = new AbortController().signal;

Deno.test("open and commit go to the handler mounted on the number", () => {
  const kept: [string, U8Array][] = [];
  const mounts = new MountTable([
    [1, { open: (path) => new TextEncoder().encode(path) }],
    [2, { open: () => new Uint8Array(), commit: (path, bytes) => kept.push([path, bytes]) }],
  ]);
  assertEquals(new TextDecoder().decode(mounts.open(1, "a/b", signal) as U8Array), "a/b");
  mounts.commit(2, "c", new Uint8Array([7]));
  assertEquals(kept, [["c", new Uint8Array([7])]]);
  const errno = (f: () => unknown) => (assertThrows(f, FileError) as FileError).errno;
  assertEquals(errno(() => mounts.open(3, "a", signal)), Errno.ENOENT);
  assertEquals(errno(() => mounts.commit(1, "a", new Uint8Array())), Errno.EROFS);
});
