// @pxl/client/driver/namespace/object.test.ts

import { test } from "vitest";
import assert from "node:assert/strict";
import * as Errno from "@xpute/kit/status/errno.spec.ts";
import { FileError } from "@xpute/host/ipc/sys.ts";

import { format, object } from "@/abi/host.spec.ts";
import {
  decode_top,
  decode_writes,
  encode_writes,
  object_backend,
  object_key,
  TOP_KEY,
} from "./object.ts";
import type { Leaf } from "./object_tree.ts";
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
const TILE = (1n << 59n) | (1n << 58n);
/** Shard 0 at `SHARD_ZOOM`, by subject. */
const SHARD = 70368744177664n;
const EDITS = BigInt(object.EDITS);
const TOP = BigInt(object.TOP);

/** A cube at the first zoom past shard 0, of the palette's third entry; a
 * marker at a finer zoom beside it. */
const CUBE: Leaf = {
  key: 0x00000800000000000000000000000000n,
  id: 3,
  text: null,
};
const MARKER: Leaf = {
  key: 0x00001100000000000000000000000000n,
  id: 2048,
  text: "here",
};

function errno(f: () => unknown): number | undefined {
  try {
    f();
  } catch (err) {
    return err instanceof FileError ? err.errnum : undefined;
  }
  return undefined;
}

test("object_key - a shard's file under its tile's name, the top under its own", () => {
  assert.deepStrictEqual(object_key(TILE), "pxl:object:01");
  assert.deepStrictEqual(object_key(TOP), TOP_KEY);
  assert.deepStrictEqual(
    errno(() => object_key(EDITS)),
    Errno.ENOENT,
  );
});

test("encode_writes - a batch reads back as written, in order, texts and all", () => {
  const writes = [CUBE, MARKER, { ...CUBE, id: 0 }];
  const file = encode_writes(format.OBJECT_EDITS, writes);
  assert.deepStrictEqual(file[0], format.OBJECT_EDITS);
  assert.deepStrictEqual(decode_writes(format.OBJECT_EDITS, file), writes);
  assert.deepStrictEqual(
    errno(() => decode_writes(format.OBJECT, file)),
    Errno.EBADMSG,
  );
});

test("object_backend - an edit batch is kept in its shard, and the top lists it", () => {
  const s = storage();
  const b = object_backend(s);
  assert.deepStrictEqual(
    errno(() => b.open(TOP)),
    Errno.ENOENT,
  );
  b.commit(EDITS, encode_writes(format.OBJECT_EDITS, [CUBE, MARKER]));
  const own = decode_writes(format.OBJECT, b.open(SHARD));
  assert.deepStrictEqual(own, [CUBE, MARKER]);
  const rows = decode_top(b.open(TOP));
  assert.deepStrictEqual(
    rows.map((r) => [r.shard, r.z]),
    [[SHARD, 0]],
  );
  assert.ok(rows[0]!.fold >>> 16 > 0);
});

test("object_backend - a shard emptied by its edits has no file, nor a row", () => {
  const s = storage();
  const b = object_backend(s);
  b.commit(EDITS, encode_writes(format.OBJECT_EDITS, [CUBE]));
  b.commit(EDITS, encode_writes(format.OBJECT_EDITS, [{ ...CUBE, id: 0 }]));
  assert.deepStrictEqual(
    errno(() => b.open(SHARD)),
    Errno.ENOENT,
  );
  assert.deepStrictEqual(
    errno(() => b.open(TOP)),
    Errno.ENOENT,
  );
});

test("object_backend - only an edit batch is written", () => {
  const b = object_backend(storage());
  assert.deepStrictEqual(
    errno(() => b.commit(SHARD, new Uint8Array([format.OBJECT]))),
    Errno.EROFS,
  );
  assert.deepStrictEqual(
    errno(() => b.commit(EDITS, new Uint8Array([format.OBJECT]))),
    Errno.EBADMSG,
  );
});

test("object_backend - a refused shard is set aside, its rows of the top with it", () => {
  const s = storage();
  const b = object_backend(s);
  b.commit(EDITS, encode_writes(format.OBJECT_EDITS, [CUBE]));
  const text = s.map.get(object_key(SHARD))!;
  assert.ok(b.set_aside(SHARD, 7));
  assert.deepStrictEqual(s.map.get(`${object_key(SHARD)}:aside:7`), text);
  assert.deepStrictEqual(
    errno(() => b.open(SHARD)),
    Errno.ENOENT,
  );
  assert.deepStrictEqual(
    errno(() => b.open(TOP)),
    Errno.ENOENT,
  );
});

test("object_backend - text that keeps no bytes is no file of bytes", () => {
  const s = storage();
  s.map.set(object_key(SHARD), "{not base64}");
  assert.deepStrictEqual(
    errno(() => object_backend(s).open(SHARD)),
    Errno.EBADMSG,
  );
});
