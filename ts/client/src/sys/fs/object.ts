// @pxl/client/sys/fs/object.ts

/**
 * The backend of the core's OBJECT files, which hold the objects placed in the
 * world. It keeps them in the platform's database (service/db.ts). The core
 * reads two kinds of file: a shard, named by its tile's quadkey, and the top
 * of the tree (`object.TOP` in file.json). The core writes one: a batch of
 * edits to `object.EDITS`. In one transaction, each batch is laid over the
 * shards its writes fall in, and the top's rows for those shards are rebuilt
 * (object_tree.ts). A shard with no leaves has no file, and neither has a top
 * with no rows. A server can answer the same `open` and `commit` instead
 * (namespace.ts), and the core cannot tell the two apart. A commit sends no
 * answer back to the core, so a batch that cannot be kept is only logged.
 */

import type { U32Array, U8Array } from "@xpute/kit/abi/array.ts";
import * as Errno from "@xpute/kit/status/errno.spec.ts";
import { FileError } from "@xpute/runtime";
import { TreeEncoder, TreeReader, TreeView } from "@xpute/kit/wire/xtp/mod.ts";

import { format, object } from "@/abi/file.spec.ts";
import {
  lay,
  shard_of,
  top_of,
  type Leaf,
  type TopRow,
} from "@/sys/fs/object_tree.ts";
import { type Tx } from "@/service/types.ts";
import * as db from "@/service/db.ts";

/** A file is one format byte followed by a tree packet. A file whose format
 * byte is not `fmt` is refused with EBADMSG. */
function packet(fmt: number, bytes: U8Array): U8Array {
  if (bytes[0] !== fmt) throw new FileError(Errno.EBADMSG);
  // The tree reader needs the packet to start on a word boundary. A view at
  // offset 1 would not, so the packet is copied.
  return bytes.slice(1);
}

function as_file(fmt: number, pkt: U8Array): U8Array {
  const out = new Uint8Array(1 + pkt.byteLength);
  out[0] = fmt;
  out.set(pkt, 1);
  return out;
}

/** Returns the writes in a file of `fmt` in the order they were written. The
 * layout is file.json's `write`. */
export function decode_writes(fmt: number, bytes: U8Array): Leaf[] {
  const [keys, id, rows, texts] = new TreeReader(packet(fmt, bytes))
    .read_branch()
    .get<[string[], U32Array, U32Array, string[]]>();
  if (keys.length !== id.length) throw new FileError(Errno.EINVAL);
  const out: Leaf[] = keys.map((key, r) => ({ key, id: id[r]!, text: null }));
  rows.forEach((r, k) => {
    if (out[r] === undefined || texts[k] === undefined) {
      throw new FileError(Errno.EINVAL);
    }
    out[r]!.text = texts[k]!;
  });
  return out;
}

export function encode_writes(fmt: number, leaves: readonly Leaf[]): U8Array {
  const rows: number[] = [];
  const texts: string[] = [];
  leaves.forEach((l, r) => {
    if (l.text === null) return;
    rows.push(r);
    texts.push(l.text);
  });
  const tree = new TreeView();
  // The fields go in the order file.json's `write` lists them.
  tree.branch((b) =>
    b
      .strs(leaves.map((l) => l.key))
      .u32_array(Uint32Array.from(leaves, (l) => l.id))
      .u32_array(Uint32Array.from(rows))
      .strs(texts),
  );
  return as_file(fmt, new TreeEncoder().encode(tree, { init_cap: 1 << 12 }));
}

/** Returns the top's rows. The layout is file.json's `top`. */
export function decode_top(bytes: U8Array): TopRow[] {
  const [shard, fold] = new TreeReader(packet(format.OBJECT_TOP, bytes))
    .read_branch()
    .get<[string[], U32Array]>();
  if (shard.length !== fold.length) throw new FileError(Errno.EINVAL);
  return shard.map((s, r) => ({ shard: s, fold: fold[r]! }));
}

export function encode_top(rows: readonly TopRow[]): U8Array {
  const tree = new TreeView();
  // The fields go in the order file.json's `top` lists them.
  tree.branch((b) =>
    b
      .strs(rows.map((r) => r.shard))
      .u32_array(Uint32Array.from(rows, (r) => r.fold)),
  );
  return as_file(
    format.OBJECT_TOP,
    new TreeEncoder().encode(tree, { init_cap: 1 << 12 }),
  );
}

function by_shard(a: TopRow, b: TopRow): number {
  return a.shard < b.shard ? -1 : a.shard > b.shard ? 1 : 0;
}

/** What the backend needs of the platform's database (service/db.ts). A test
 * passes its own. */
export type Db = { transaction<T>(fn: (tx: Tx) => Promise<T>): Promise<T> };

const DB: Db = db;

/** Moves the value under `key` to `<key>:aside:<now_ms>`. A value set aside
 * earlier is never overwritten: a clash takes a numbered suffix. */
async function move_aside(tx: Tx, key: string, now_ms: number): Promise<void> {
  const bytes = await tx.get(key);
  if (bytes === null) return;
  let aside = `${key}:aside:${now_ms}`;
  for (let n = 1; (await tx.get(aside)) !== null; n++) {
    aside = `${key}:aside:${now_ms}-${n}`;
  }
  await tx.put(aside, bytes);
  await tx.put(key, null);
}

/** Reads the kept top's rows. A top that does not decode was written by
 * another build: it is set aside and read as empty. */
async function rows_of(tx: Tx): Promise<TopRow[]> {
  const bytes = await tx.get(object.TOP);
  if (bytes === null) return [];
  try {
    return decode_top(bytes);
  } catch {
    await move_aside(tx, object.TOP, Date.now());
    return [];
  }
}

/** Reads a kept shard's leaves. A shard that does not decode was written by
 * another build: it is set aside and read as empty. */
async function leaves_of(tx: Tx, shard: string): Promise<Leaf[]> {
  const bytes = await tx.get(shard);
  if (bytes === null) return [];
  try {
    return decode_writes(format.OBJECT, bytes);
  } catch {
    await move_aside(tx, shard, Date.now());
    return [];
  }
}

async function put_top(tx: Tx, rows: readonly TopRow[]): Promise<void> {
  await tx.put(object.TOP, rows.length === 0 ? null : encode_top(rows));
}

async function apply_edits(tx: Tx, writes: readonly Leaf[]): Promise<void> {
  const shards = new Map<string, Map<string, Leaf>>();
  for (const w of writes) {
    const shard = shard_of(w.key);
    let leaves = shards.get(shard);
    if (leaves === undefined) {
      const own = await leaves_of(tx, shard);
      leaves = new Map(own.map((l) => [l.key, l]));
      shards.set(shard, leaves);
    }
    lay(leaves, w);
  }
  const others = (await rows_of(tx)).filter((r) => !shards.has(r.shard));
  for (const [shard, leaves] of shards) {
    const own = [...leaves.values()].sort((a, b) =>
      a.key < b.key ? -1 : a.key > b.key ? 1 : 0,
    );
    await tx.put(
      shard,
      own.length === 0 ? null : encode_writes(format.OBJECT, own),
    );
    others.push(...top_of(shard, own));
  }
  others.sort(by_shard);
  await put_top(tx, others);
}

/** OBJECT's backend over `db`. A read sees every batch committed before it,
 * because the database runs its transactions one at a time, in the order they
 * were begun. */
export function object_backend(db: Db = DB): {
  open(path: string): Promise<U8Array>;
  commit(path: string, bytes: U8Array): void;
  set_aside(path: string, now_ms: number): Promise<boolean>;
} {
  return {
    open(path) {
      return db.transaction(async (tx) => {
        const bytes = await tx.get(path);
        if (bytes === null) throw new FileError(Errno.ENOENT);
        return bytes;
      });
    },
    commit(path, bytes) {
      if (path !== object.EDITS) throw new FileError(Errno.EROFS);
      const writes = decode_writes(format.OBJECT_EDITS, bytes);
      db.transaction((tx) => apply_edits(tx, writes)).catch((err: unknown) => {
        console.error("[object] an edit batch was not kept:", err);
      });
    },
    /** Sets aside a file the core refused, so that its path reads as empty.
     * A shard's rows leave the top with it. Resolves false, with nothing
     * changed, when the database fails. */
    set_aside(path, now_ms) {
      return db
        .transaction(async (tx) => {
          await move_aside(tx, path, now_ms);
          if (path !== object.TOP) {
            const left = (await rows_of(tx)).filter((r) => r.shard !== path);
            await put_top(tx, left);
          }
          return true;
        })
        .catch(() => false);
    },
  };
}

/** The backend that namespace.ts binds OBJECT to, and that refusal.ts sets
 * refused files aside through. */
export const OBJECTS = object_backend();
