// @xpute/host/ipc/directory.test.ts

import { test } from "vitest";
import { assertEquals, assertThrows } from "@std/assert";

import { MarshalError } from "@xpute/kit/status/error.ts";
import { Directory } from "./directory.ts";
import { HEAD, key, MAGIC, VERSION } from "./directory.spec.ts";

function written(
  entries: [number, number][],
  head: [number, number] = [MAGIC, VERSION],
): ArrayBuffer {
  const mem = new ArrayBuffer(8 + 4 * (HEAD + 2 * entries.length));
  new Uint32Array(mem, 8).set([
    head[0],
    head[1],
    entries.length,
    0,
    ...entries.flat(),
  ]);
  return mem;
}

test("directory - a key the host does not know is passed over, and one it does is found past it", () => {
  const dir = new Directory(
    written([
      [key.SQ, 64],
      [0x1234, 7],
      [(key.PROGRAM | 9) >>> 0, 11],
      [key.CQ, 128],
    ]),
    8,
  );
  assertEquals(
    [dir.get(key.SQ), dir.get(key.CQ), dir.get(key.PROGRAM | 9)],
    [64, 128, 11],
  );
});

test("directory - a key written twice, a head that is not xpute's, and a version this host does not read are refused", () => {
  assertThrows(
    () =>
      new Directory(
        written([
          [key.SQ, 64],
          [key.SQ, 96],
        ]),
        8,
      ),
    MarshalError,
  );
  assertThrows(
    () => new Directory(written([[key.SQ, 64]], [0, VERSION]), 8),
    MarshalError,
  );
  assertThrows(
    () => new Directory(written([[key.SQ, 64]], [MAGIC, VERSION + 1]), 8),
    MarshalError,
  );
});

test("directory - a key the guest did not write is refused rather than read as 0", () => {
  assertThrows(
    () => new Directory(written([[key.SQ, 64]]), 8).get(key.CQ),
    MarshalError,
  );
});
