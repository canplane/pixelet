// @pxl/client/driver/namespace/object.test.ts

import { test } from "vitest";
import assert from "node:assert/strict";
import * as Errno from "@xpute/kit/status/errno.spec.ts";
import { FileError } from "@xpute/host/ipc/sys.ts";

import { object_backend, object_key } from "./object.ts";
import type { Storage } from "./storage.ts";

function storage(): Storage & { map: Map<string, string> } {
  const map = new Map<string, string>();
  return {
    map,
    get: (k) => map.get(k) ?? null,
    set: (k, v) => void map.set(k, v),
  };
}

/** The tile "01", at z2: its digits two a zoom from bit 62 down, then a 1. */
const SHARD = (1n << 59n) | (1n << 58n);

function errno(f: () => unknown): number | undefined {
  try {
    f();
  } catch (err) {
    return err instanceof FileError ? err.errnum : undefined;
  }
  return undefined;
}

test("object_backend - a shard's file reads back as written, under its tile's name", () => {
  const s = storage();
  const b = object_backend(s);
  b.commit(SHARD, new Uint8Array([16, 0, 255]));
  assert.deepStrictEqual(object_key(SHARD), "pxl:object:01");
  assert.deepStrictEqual([...b.open(SHARD)], [16, 0, 255]);
});

test("object_backend - a shard never written, or written empty, has no file", () => {
  const s = storage();
  const b = object_backend(s);
  assert.deepStrictEqual(
    errno(() => b.open(SHARD)),
    Errno.ENOENT,
  );
  b.commit(SHARD, new Uint8Array([16]));
  b.commit(SHARD, new Uint8Array());
  assert.deepStrictEqual(
    errno(() => b.open(SHARD)),
    Errno.ENOENT,
  );
  assert.deepStrictEqual(
    errno(() => b.open(0n)),
    Errno.ENOENT,
  );
});
