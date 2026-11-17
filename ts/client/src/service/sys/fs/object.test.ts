// @pxl/client/service/sys/fs/object.test.ts

import { test } from "vitest";
import assert from "node:assert/strict";
import { ERRNO_EBADMSG } from "@xpute/kit/status/errno.spec.ts";
import { XputeError } from "@xpute/kit/status/error.ts";
import { ComponentError } from "@xpute/link";
import { ERROR_CODE_NO_ENTRY } from "@xpute/host/abi/fs/types.enum.ts";

import type { U8Array } from "@xpute/kit/abi/array.ts";

import { format } from "@/abi/file.spec.ts";
import type { Transaction } from "@/service/platform/types.ts";
import {
  decodeTop,
  decodeWrites,
  encodeWrites,
  objectBackend,
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
    transaction<T>(fn: (tx: Transaction) => Promise<T>): Promise<T> {
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
  assetId: 3,
  text: null,
};
const MARKER: Leaf = {
  key: "0000000010",
  assetId: 2048,
  text: "here",
};

async function errno(f: () => unknown): Promise<number | undefined> {
  try {
    await f();
  } catch (err) {
    return err instanceof XputeError ? err.errnum : undefined;
  }
  return undefined;
}

async function code(f: () => unknown): Promise<unknown> {
  try {
    await f();
  } catch (err) {
    return err instanceof ComponentError ? err.payload : undefined;
  }
  return undefined;
}

test("encodeWrites - a batch reads back as written, in order, texts and all", async () => {
  const writes = [CUBE, MARKER, { ...CUBE, assetId: 0 }];
  const file = encodeWrites(format.OBJECT_EDIT, writes);
  assert.deepStrictEqual(file[0], format.OBJECT_EDIT);
  assert.deepStrictEqual(decodeWrites(format.OBJECT_EDIT, file), writes);
  assert.deepStrictEqual(
    await errno(() => decodeWrites(format.OBJECT, file)),
    ERRNO_EBADMSG,
  );
});

test("objectBackend - an edit batch is kept in its shard, and the top lists it", async () => {
  const b = objectBackend(database());
  assert.deepStrictEqual(await code(() => b.open(TOP)), ERROR_CODE_NO_ENTRY);
  b.commit(encodeWrites(format.OBJECT_EDIT, [CUBE, MARKER]));
  const own = decodeWrites(format.OBJECT, await b.open(SHARD));
  assert.deepStrictEqual(own, [CUBE, MARKER]);
  const rows = decodeTop(await b.open(TOP));
  assert.deepStrictEqual(
    rows.map((r) => r.shard),
    [SHARD],
  );
  assert.ok(rows[0]!.fold >>> 16 > 0);
});

test("objectBackend - batches apply in the order written", async () => {
  const b = objectBackend(database());
  b.commit(encodeWrites(format.OBJECT_EDIT, [CUBE]));
  b.commit(encodeWrites(format.OBJECT_EDIT, [MARKER]));
  const own = decodeWrites(format.OBJECT, await b.open(SHARD));
  assert.deepStrictEqual(own, [CUBE, MARKER]);
});

test("objectBackend - a shard emptied by its edits has no file, nor a row", async () => {
  const db = database();
  const b = objectBackend(db);
  b.commit(encodeWrites(format.OBJECT_EDIT, [CUBE]));
  b.commit(encodeWrites(format.OBJECT_EDIT, [{ ...CUBE, assetId: 0 }]));
  assert.deepStrictEqual(await code(() => b.open(SHARD)), ERROR_CODE_NO_ENTRY);
  assert.deepStrictEqual(await code(() => b.open(TOP)), ERROR_CODE_NO_ENTRY);
  assert.deepStrictEqual(db.map.size, 0);
});

test("objectBackend - a file not an edit batch is refused", async () => {
  const b = objectBackend(database());
  assert.deepStrictEqual(
    await errno(() => b.commit(new Uint8Array([format.OBJECT]))),
    ERRNO_EBADMSG,
  );
});

test("objectBackend - an edit over a shard of another build sets it aside and is kept", async () => {
  const db = database();
  const b = objectBackend(db);
  const old = new Uint8Array([format.OBJECT - 1, 0, 0, 0]);
  db.map.set(SHARD, old);
  b.commit(encodeWrites(format.OBJECT_EDIT, [CUBE]));
  const own = decodeWrites(format.OBJECT, await b.open(SHARD));
  assert.deepStrictEqual(own, [CUBE]);
  assert.ok(
    [...db.map].some(([k, v]) => k.startsWith(`${SHARD}:aside:`) && v === old),
  );
});

test("objectBackend - a refused shard is set aside, its rows of the top with it", async () => {
  const db = database();
  const b = objectBackend(db);
  b.commit(encodeWrites(format.OBJECT_EDIT, [CUBE]));
  const file = await b.open(SHARD);
  await b.setAside(SHARD, 7);
  assert.deepStrictEqual(db.map.get(`${SHARD}:aside:7`), file);
  assert.deepStrictEqual(await code(() => b.open(SHARD)), ERROR_CODE_NO_ENTRY);
  assert.deepStrictEqual(await code(() => b.open(TOP)), ERROR_CODE_NO_ENTRY);
});

test("objectBackend - a database that refuses keeps no commit and sets nothing aside", async () => {
  const b = objectBackend(database(true));
  await assert.rejects(b.commit(encodeWrites(format.OBJECT_EDIT, [CUBE])));
  await assert.rejects(b.setAside(SHARD, 7));
});
