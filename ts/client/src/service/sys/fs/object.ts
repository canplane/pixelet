// @pxl/client/service/sys/fs/object.ts

/**
 * The backend of the core's OBJECT files, which hold the objects placed in the
 * world. It keeps them in the platform's database (service/platform/db.ts),
 * each under its key: a shard under its tile's quadkey, and the top of the
 * tree under `TOP_KEY`. The core reads those two kinds of file, (OBJECT,
 * quadkey) and (OBJECT, TOP), which binding.ts maps here, and sends batches of
 * edits through pxl:core object `apply` (`mountObjects`). In one
 * transaction, each batch is laid over the shards its writes fall in, and the
 * top's rows for those shards are rebuilt (object_tree.ts). A shard with no
 * leaves has no file, and neither has a top with no rows. A server can answer
 * the same `open` and `apply` instead, and the core cannot tell the two
 * apart. A batch the database fails to keep answers `io`, and the core sends
 * it again; a bug passes on as itself.
 */

import type { U32Array, U8Array } from "@xpute/kit/abi/array.ts";
import { XputeBug } from "@xpute/kit/status/bug.ts";
import { ERRNO_EBADMSG, ERRNO_EINVAL } from "@xpute/kit/status/errno.spec.ts";
import { XputeError } from "@xpute/kit/status/error.ts";
import { type Callee, ComponentError, type Memory } from "@xpute/link/link.ts";
import {
  ERROR_CODE_BAD_MESSAGE,
  ERROR_CODE_IO,
  ERROR_CODE_NO_ENTRY,
} from "@xpute/host/abi/fs/types.enum.ts";
import { TreeReader } from "@xpute/kit/wire/xtp/cursor.ts";
import { TreeEncoder } from "@xpute/kit/wire/xtp/encode.ts";
import { TreeView } from "@xpute/kit/wire/xtp/view.ts";

import { format } from "@/abi/file.spec.ts";
import * as object from "@/abi/object.ts";
import {
  lay,
  shardOf,
  topOf,
  type Leaf,
  type TopRow,
} from "@/service/sys/fs/object_tree.ts";
import { type Transaction } from "@/service/platform/types.ts";
import * as db from "@/service/platform/db.ts";

/** The key the top is kept under; no quadkey has a letter. */
export const TOP_KEY = "top";

/** A file is one format byte followed by a tree packet. */
function pkt(fmt: number, bytes: U8Array): U8Array {
  if (bytes[0] !== fmt) throw new XputeError(ERRNO_EBADMSG);
  // The tree reader needs the packet to start on a word boundary. A view at
  // offset 1 would not, so the packet is copied.
  return bytes.slice(1);
}

function asFile(fmt: number, pkt: U8Array): U8Array {
  const out = new Uint8Array(1 + pkt.byteLength);
  out[0] = fmt;
  out.set(pkt, 1);
  return out;
}

/** Returns the writes in a file of `fmt` in the order they were written. The
 * layout is file.json5's `write`. */
export function decodeWrites(fmt: number, bytes: U8Array): Leaf[] {
  const [keys, assetIds, rows, texts] = new TreeReader(pkt(fmt, bytes))
    .read_branch()
    .get<[string[], U32Array, U32Array, string[]]>();
  if (keys.length !== assetIds.length) throw new XputeError(ERRNO_EINVAL);
  const out: Leaf[] = keys.map((key, r) => ({
    key,
    assetId: assetIds[r]!,
    text: null,
  }));
  rows.forEach((r, k) => {
    if (out[r] === undefined || texts[k] === undefined) {
      throw new XputeError(ERRNO_EINVAL);
    }
    out[r]!.text = texts[k]!;
  });
  return out;
}

export function encodeWrites(fmt: number, leaves: readonly Leaf[]): U8Array {
  const rows: number[] = [];
  const texts: string[] = [];
  leaves.forEach((l, r) => {
    if (l.text === null) return;
    rows.push(r);
    texts.push(l.text);
  });
  const tree = new TreeView();
  // The fields go in the order file.json5's `write` lists them.
  tree.branch((b) =>
    b
      .strs(leaves.map((l) => l.key))
      .u32_array(Uint32Array.from(leaves, (l) => l.assetId))
      .u32_array(Uint32Array.from(rows))
      .strs(texts),
  );
  return asFile(fmt, new TreeEncoder().encode(tree, { init_cap: 1 << 12 }));
}

/** The layout is file.json5's `top`. */
export function decodeTop(bytes: U8Array): TopRow[] {
  const [shard, fold] = new TreeReader(pkt(format.OBJECT_TOP, bytes))
    .read_branch()
    .get<[string[], U32Array]>();
  if (shard.length !== fold.length) throw new XputeError(ERRNO_EINVAL);
  return shard.map((s, r) => ({ shard: s, fold: fold[r]! }));
}

function encodeTop(rows: readonly TopRow[]): U8Array {
  const tree = new TreeView();
  // The fields go in the order file.json5's `top` lists them.
  tree.branch((b) =>
    b
      .strs(rows.map((r) => r.shard))
      .u32_array(Uint32Array.from(rows, (r) => r.fold)),
  );
  return asFile(
    format.OBJECT_TOP,
    new TreeEncoder().encode(tree, { init_cap: 1 << 12 }),
  );
}

function byShard(a: TopRow, b: TopRow): number {
  return a.shard < b.shard ? -1 : a.shard > b.shard ? 1 : 0;
}

/** The database the files are kept in: the platform's, or a test's own. */
export type Db = {
  transaction<T>(fn: (tx: Transaction) => Promise<T>): Promise<T>;
};

const DB: Db = db;

/** Moves the value under `key` to `<key>:aside:<nowMs>`. A value set aside
 * earlier is never overwritten: a clash takes a numbered suffix. */
async function moveAside(
  tx: Transaction,
  key: string,
  nowMs: number,
): Promise<void> {
  const bytes = await tx.get(key);
  if (bytes === null) return;
  let aside = `${key}:aside:${nowMs}`;
  for (let n = 1; (await tx.get(aside)) !== null; n++) {
    aside = `${key}:aside:${nowMs}-${n}`;
  }
  await tx.put(aside, bytes);
  await tx.put(key, null);
}

/** A top that does not decode was written by another build, so it is set aside
 * and read as empty. */
async function rowsOf(tx: Transaction): Promise<TopRow[]> {
  const bytes = await tx.get(TOP_KEY);
  if (bytes === null) return [];
  try {
    return decodeTop(bytes);
  } catch {
    await moveAside(tx, TOP_KEY, Date.now());
    return [];
  }
}

/** A shard that does not decode was written by another build, so it is set
 * aside and read as empty. */
async function leavesOf(tx: Transaction, shard: string): Promise<Leaf[]> {
  const bytes = await tx.get(shard);
  if (bytes === null) return [];
  try {
    return decodeWrites(format.OBJECT, bytes);
  } catch {
    await moveAside(tx, shard, Date.now());
    return [];
  }
}

async function putTop(tx: Transaction, rows: readonly TopRow[]): Promise<void> {
  await tx.put(TOP_KEY, rows.length === 0 ? null : encodeTop(rows));
}

async function applyEdits(
  tx: Transaction,
  writes: readonly Leaf[],
): Promise<void> {
  const shards = new Map<string, Map<string, Leaf>>();
  for (const w of writes) {
    const shard = shardOf(w.key);
    let leaves = shards.get(shard);
    if (leaves === undefined) {
      const own = await leavesOf(tx, shard);
      leaves = new Map(own.map((l) => [l.key, l]));
      shards.set(shard, leaves);
    }
    lay(leaves, w);
  }
  const others = (await rowsOf(tx)).filter((r) => !shards.has(r.shard));
  for (const [shard, leaves] of shards) {
    const own = [...leaves.values()].sort((a, b) =>
      a.key < b.key ? -1 : a.key > b.key ? 1 : 0,
    );
    await tx.put(
      shard,
      own.length === 0 ? null : encodeWrites(format.OBJECT, own),
    );
    others.push(...topOf(shard, own));
  }
  others.sort(byShard);
  await putTop(tx, others);
}

/** A read sees every batch committed before it, because the database runs its
 * transactions one at a time, in the order they were begun. */
export function objectBackend(db: Db = DB): {
  open(key: string): Promise<U8Array>;
  commit(bytes: U8Array): Promise<void>;
  setAside(key: string, nowMs: number): Promise<void>;
} {
  return {
    open(key) {
      return db.transaction(async (tx) => {
        const bytes = await tx.get(key);
        if (bytes === null) throw new ComponentError(ERROR_CODE_NO_ENTRY);
        return bytes;
      });
    },
    /** Applies an edit batch. */
    commit(bytes) {
      const writes = decodeWrites(format.OBJECT_EDIT, bytes);
      return db.transaction((tx) => applyEdits(tx, writes));
    },
    /** Sets aside a file the core refused, so that its key reads as empty.
     * A shard's rows leave the top with it. Rejects, with nothing changed,
     * when the database fails. */
    setAside(key, nowMs) {
      return db.transaction(async (tx) => {
        await moveAside(tx, key, nowMs);
        if (key !== TOP_KEY) {
          const left = (await rowsOf(tx)).filter((r) => r.shard !== key);
          await putTop(tx, left);
        }
      });
    },
  };
}

export const OBJECTS = objectBackend();

/** Mounts the core's objects (pxl:core object) on `link`: each batch it
 * lends in `memory` is applied. One that does not decode answers `bad-message`,
 * and one the database fails to keep `io`. */
export function mountObjects(link: Callee, memory: Memory): void {
  object.mount(link, {
    async apply(batch) {
      try {
        await OBJECTS.commit(memory.bytes(batch.buf, batch.nbyte).slice());
      } catch (e) {
        if (e instanceof XputeBug) throw e;
        // Only the decode throws an XputeError; the database throws its own.
        throw new ComponentError(
          e instanceof XputeError ? ERROR_CODE_BAD_MESSAGE : ERROR_CODE_IO,
        );
      }
    },
  });
}
