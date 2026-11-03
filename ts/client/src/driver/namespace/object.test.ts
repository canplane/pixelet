// @pxl/client/driver/namespace/object.test.ts

import { test } from "vitest";
import assert from "node:assert/strict";
import * as Errno from "@xpute/kit/status/errno.spec.ts";
import { FileError } from "@xpute/host/ipc/sys.ts";

import type { U8Array } from "@xpute/kit/abi/array.ts";

import { format, object } from "@/abi/host.spec.ts";
import type { Tx } from "@/bracket.tsx";
import {
  decode_top,
  decode_writes,
  encode_writes,
  object_backend,
  object_key,
  TOP_KEY,
  type Db,
} from "./object.ts";
import type { Leaf } from "./object_tree.ts";

/** A database over a Map: a transaction's puts land when it resolves, one at
 * a time in the order asked. `refuses`: every transaction rejects. */
function database(refuses = false): Db & { map: Map<string, U8Array> } {
  const map = new Map<string, U8Array>();
  let last: Promise<unknown> = Promise.resolve();
  return {
    map,
    transaction<T>(fn: (tx: Tx) => Promise<T>): Promise<T> {
      const run = last.then(async () => {
        if (refuses) throw new Error("refused");
        const puts = new Map<string, U8Array | null>();
        const out = await fn({
          get: async (k) => (puts.has(k) ? puts.get(k)! : (map.get(k) ?? null)),
          put: async (k, v) => void puts.set(k, v),
        });
        for (const [k, v] of puts) {
          if (v === null) map.delete(k);
          else map.set(k, v);
        }
        return out;
      });
      last = run.catch(() => {});
      return run;
    },
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

async function errno(f: () => unknown): Promise<number | undefined> {
  try {
    await f();
  } catch (err) {
    return err instanceof FileError ? err.errnum : undefined;
  }
  return undefined;
}

test("object_key - a shard's file under its tile's name, the top under its own", async () => {
  assert.deepStrictEqual(object_key(TILE), "01");
  assert.deepStrictEqual(object_key(TOP), TOP_KEY);
  assert.deepStrictEqual(await errno(() => object_key(EDITS)), Errno.ENOENT);
});

test("encode_writes - a batch reads back as written, in order, texts and all", async () => {
  const writes = [CUBE, MARKER, { ...CUBE, id: 0 }];
  const file = encode_writes(format.OBJECT_EDITS, writes);
  assert.deepStrictEqual(file[0], format.OBJECT_EDITS);
  assert.deepStrictEqual(decode_writes(format.OBJECT_EDITS, file), writes);
  assert.deepStrictEqual(
    await errno(() => decode_writes(format.OBJECT, file)),
    Errno.EBADMSG,
  );
});

test("object_backend - an edit batch is kept in its shard, and the top lists it", async () => {
  const b = object_backend(database());
  assert.deepStrictEqual(await errno(() => b.open(TOP)), Errno.ENOENT);
  b.commit(EDITS, encode_writes(format.OBJECT_EDITS, [CUBE, MARKER]));
  const own = decode_writes(format.OBJECT, await b.open(SHARD));
  assert.deepStrictEqual(own, [CUBE, MARKER]);
  const rows = decode_top(await b.open(TOP));
  assert.deepStrictEqual(
    rows.map((r) => [r.shard, r.z]),
    [[SHARD, 0]],
  );
  assert.ok(rows[0]!.fold >>> 16 > 0);
});

test("object_backend - batches apply in the order written", async () => {
  const b = object_backend(database());
  b.commit(EDITS, encode_writes(format.OBJECT_EDITS, [CUBE]));
  b.commit(EDITS, encode_writes(format.OBJECT_EDITS, [MARKER]));
  const own = decode_writes(format.OBJECT, await b.open(SHARD));
  assert.deepStrictEqual(own, [CUBE, MARKER]);
});

test("object_backend - a shard emptied by its edits has no file, nor a row", async () => {
  const db = database();
  const b = object_backend(db);
  b.commit(EDITS, encode_writes(format.OBJECT_EDITS, [CUBE]));
  b.commit(EDITS, encode_writes(format.OBJECT_EDITS, [{ ...CUBE, id: 0 }]));
  assert.deepStrictEqual(await errno(() => b.open(SHARD)), Errno.ENOENT);
  assert.deepStrictEqual(await errno(() => b.open(TOP)), Errno.ENOENT);
  assert.deepStrictEqual(db.map.size, 0);
});

test("object_backend - only an edit batch is written", async () => {
  const b = object_backend(database());
  assert.deepStrictEqual(
    await errno(() => b.commit(SHARD, new Uint8Array([format.OBJECT]))),
    Errno.EROFS,
  );
  assert.deepStrictEqual(
    await errno(() => b.commit(EDITS, new Uint8Array([format.OBJECT]))),
    Errno.EBADMSG,
  );
});

test("object_backend - a refused shard is set aside, its rows of the top with it", async () => {
  const db = database();
  const b = object_backend(db);
  b.commit(EDITS, encode_writes(format.OBJECT_EDITS, [CUBE]));
  const file = await b.open(SHARD);
  assert.ok(await b.set_aside(SHARD, 7));
  assert.deepStrictEqual(db.map.get(`${object_key(SHARD)}:aside:7`), file);
  assert.deepStrictEqual(await errno(() => b.open(SHARD)), Errno.ENOENT);
  assert.deepStrictEqual(await errno(() => b.open(TOP)), Errno.ENOENT);
});

test("object_backend - a database that refuses sets nothing aside, and throws at no commit", async () => {
  const error = console.error;
  console.error = () => {};
  try {
    const b = object_backend(database(true));
    b.commit(EDITS, encode_writes(format.OBJECT_EDITS, [CUBE]));
    assert.deepStrictEqual(await b.set_aside(SHARD, 7), false);
  } finally {
    console.error = error;
  }
});
