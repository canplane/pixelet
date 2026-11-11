// @pxl/client/service/sys/fs/object.test.ts

import { test } from "vitest";
import assert from "node:assert/strict";
import * as Errno from "@xpute/kit/status/errno.spec.ts";
import { FileError } from "@xpute/runtime";

import type { U8Array } from "@xpute/kit/abi/array.ts";

import { format } from "@/abi/file.spec.ts";
import type { Tx } from "@/service/platform/types.ts";
import {
  decode_top,
  decode_writes,
  encode_writes,
  object_backend,
  TOP_KEY as TOP,
  type Db,
} from "./object.ts";
import type { Leaf } from "./object_tree.ts";

/** A transaction's puts land when it resolves, and transactions run one at a
 * time in the order they are asked for, as the platform's do. */
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

/** The quadkey of shard 0 at `SHARD_ZOOM`. */
const SHARD = "00000000";

/** A cube of the palette's third entry at the first zoom past shard 0, and a
 * marker beside it at a finer zoom. */
const CUBE: Leaf = {
  key: "000000000",
  asset_id: 3,
  text: null,
};
const MARKER: Leaf = {
  key: "0000000010",
  asset_id: 2048,
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

test("encode_writes - a batch reads back as written, in order, texts and all", async () => {
  const writes = [CUBE, MARKER, { ...CUBE, asset_id: 0 }];
  const file = encode_writes(format.OBJECT_EDIT, writes);
  assert.deepStrictEqual(file[0], format.OBJECT_EDIT);
  assert.deepStrictEqual(decode_writes(format.OBJECT_EDIT, file), writes);
  assert.deepStrictEqual(
    await errno(() => decode_writes(format.OBJECT, file)),
    Errno.EBADMSG,
  );
});

test("object_backend - an edit batch is kept in its shard, and the top lists it", async () => {
  const b = object_backend(database());
  assert.deepStrictEqual(await errno(() => b.open(TOP)), Errno.ENOENT);
  b.commit(encode_writes(format.OBJECT_EDIT, [CUBE, MARKER]));
  const own = decode_writes(format.OBJECT, await b.open(SHARD));
  assert.deepStrictEqual(own, [CUBE, MARKER]);
  const rows = decode_top(await b.open(TOP));
  assert.deepStrictEqual(
    rows.map((r) => r.shard),
    [SHARD],
  );
  assert.ok(rows[0]!.fold >>> 16 > 0);
});

test("object_backend - batches apply in the order written", async () => {
  const b = object_backend(database());
  b.commit(encode_writes(format.OBJECT_EDIT, [CUBE]));
  b.commit(encode_writes(format.OBJECT_EDIT, [MARKER]));
  const own = decode_writes(format.OBJECT, await b.open(SHARD));
  assert.deepStrictEqual(own, [CUBE, MARKER]);
});

test("object_backend - a shard emptied by its edits has no file, nor a row", async () => {
  const db = database();
  const b = object_backend(db);
  b.commit(encode_writes(format.OBJECT_EDIT, [CUBE]));
  b.commit(encode_writes(format.OBJECT_EDIT, [{ ...CUBE, asset_id: 0 }]));
  assert.deepStrictEqual(await errno(() => b.open(SHARD)), Errno.ENOENT);
  assert.deepStrictEqual(await errno(() => b.open(TOP)), Errno.ENOENT);
  assert.deepStrictEqual(db.map.size, 0);
});

test("object_backend - a file not an edit batch is refused", async () => {
  const b = object_backend(database());
  assert.deepStrictEqual(
    await errno(() => b.commit(new Uint8Array([format.OBJECT]))),
    Errno.EBADMSG,
  );
});

test("object_backend - an edit over a shard of another build sets it aside and is kept", async () => {
  const db = database();
  const b = object_backend(db);
  const old = new Uint8Array([format.OBJECT - 1, 0, 0, 0]);
  db.map.set(SHARD, old);
  b.commit(encode_writes(format.OBJECT_EDIT, [CUBE]));
  const own = decode_writes(format.OBJECT, await b.open(SHARD));
  assert.deepStrictEqual(own, [CUBE]);
  assert.ok(
    [...db.map].some(([k, v]) => k.startsWith(`${SHARD}:aside:`) && v === old),
  );
});

test("object_backend - a refused shard is set aside, its rows of the top with it", async () => {
  const db = database();
  const b = object_backend(db);
  b.commit(encode_writes(format.OBJECT_EDIT, [CUBE]));
  const file = await b.open(SHARD);
  assert.ok(await b.set_aside(SHARD, 7));
  assert.deepStrictEqual(db.map.get(`${SHARD}:aside:7`), file);
  assert.deepStrictEqual(await errno(() => b.open(SHARD)), Errno.ENOENT);
  assert.deepStrictEqual(await errno(() => b.open(TOP)), Errno.ENOENT);
});

test("object_backend - a database that refuses sets nothing aside, and throws at no commit", async () => {
  const err = console.error;
  console.error = () => {};
  try {
    const b = object_backend(database(true));
    b.commit(encode_writes(format.OBJECT_EDIT, [CUBE]));
    assert.deepStrictEqual(await b.set_aside(SHARD, 7), false);
  } finally {
    console.error = err;
  }
});
