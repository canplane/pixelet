// @pxl/client/service/sys/fs/object.ts

/**
 * The backend of the core's OBJECT files, which hold the objects placed in the
 * world. It keeps them in the platform's database (service/platform/db.ts),
 * each under its key: a shard under its tile's quadkey, and the top of the
 * tree under `TOP_KEY`. The core reads those two kinds of file, (OBJECT,
 * quadkey) and (OBJECT, TOP), which binding.ts maps here, and sends batches of
 * edits through pxl:core object `apply` (`serve_objects`). In one
 * transaction, each batch is laid over the shards its writes fall in, and the
 * top's rows for those shards are rebuilt (object_tree.ts). A shard with no
 * leaves has no file, and neither has a top with no rows. A server can answer
 * the same `open` and `apply` instead, and the core cannot tell the two
 * apart. A batch the database fails to keep answers `io`, and the core sends
 * it again; a bug passes on as itself.
 */

import type { U32Array, U8Array } from "@xpute/kit/abi/array.ts";
import { Bug } from "@xpute/kit/status/bug.ts";
import { ERRNO_EBADMSG, ERRNO_EINVAL } from "@xpute/kit/status/errno.spec.ts";
import { XputeError } from "@xpute/kit/status/error.ts";
import { ComponentError, type Link } from "@xpute/link";
import {
  ERROR_CODE_BAD_MESSAGE,
  ERROR_CODE_IO,
  ERROR_CODE_NO_ENTRY,
} from "@xpute/link/abi/fs/types.enum.ts";
import { TreeEncoder, TreeReader, TreeView } from "@xpute/kit/wire/xtp/mod.ts";

import { format } from "@/abi/file.spec.ts";
import * as object from "@/abi/object.ts";
import {
  lay,
  shard_of,
  top_of,
  type Leaf,
  type TopRow,
} from "@/service/sys/fs/object_tree.ts";
import { type Tx } from "@/service/platform/types.ts";
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

function as_file(fmt: number, pkt: U8Array): U8Array {
  const out = new Uint8Array(1 + pkt.byteLength);
  out[0] = fmt;
  out.set(pkt, 1);
  return out;
}

/** Returns the writes in a file of `fmt` in the order they were written. The
 * layout is file.json's `write`. */
export function decode_writes(fmt: number, bytes: U8Array): Leaf[] {
  const [keys, asset_ids, rows, texts] = new TreeReader(pkt(fmt, bytes))
    .read_branch()
    .get<[string[], U32Array, U32Array, string[]]>();
  if (keys.length !== asset_ids.length) throw new XputeError(ERRNO_EINVAL);
  const out: Leaf[] = keys.map((key, r) => ({
    key,
    asset_id: asset_ids[r]!,
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
      .u32_array(Uint32Array.from(leaves, (l) => l.asset_id))
      .u32_array(Uint32Array.from(rows))
      .strs(texts),
  );
  return as_file(fmt, new TreeEncoder().encode(tree, { init_cap: 1 << 12 }));
}

/** The layout is file.json's `top`. */
export function decode_top(bytes: U8Array): TopRow[] {
  const [shard, fold] = new TreeReader(pkt(format.OBJECT_TOP, bytes))
    .read_branch()
    .get<[string[], U32Array]>();
  if (shard.length !== fold.length) throw new XputeError(ERRNO_EINVAL);
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

/** A test passes its own in place of the platform's database. */
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

/** A top that does not decode was written by another build, so it is set aside
 * and read as empty. */
async function rows_of(tx: Tx): Promise<TopRow[]> {
  const bytes = await tx.get(TOP_KEY);
  if (bytes === null) return [];
  try {
    return decode_top(bytes);
  } catch {
    await move_aside(tx, TOP_KEY, Date.now());
    return [];
  }
}

/** A shard that does not decode was written by another build, so it is set
 * aside and read as empty. */
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
  await tx.put(TOP_KEY, rows.length === 0 ? null : encode_top(rows));
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

/** A read sees every batch committed before it, because the database runs its
 * transactions one at a time, in the order they were begun. */
export function object_backend(db: Db = DB): {
  open(key: string): Promise<U8Array>;
  commit(bytes: U8Array): Promise<void>;
  set_aside(key: string, now_ms: number): Promise<void>;
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
      const writes = decode_writes(format.OBJECT_EDIT, bytes);
      return db.transaction((tx) => apply_edits(tx, writes));
    },
    /** Sets aside a file the core refused, so that its key reads as empty.
     * A shard's rows leave the top with it. Rejects, with nothing changed,
     * when the database fails. */
    set_aside(key, now_ms) {
      return db.transaction(async (tx) => {
        await move_aside(tx, key, now_ms);
        if (key !== TOP_KEY) {
          const left = (await rows_of(tx)).filter((r) => r.shard !== key);
          await put_top(tx, left);
        }
      });
    },
  };
}

export const OBJECTS = object_backend();

/** Serves the core's objects (pxl:core object): each batch it lends is
 * applied. One that does not decode answers `bad-message`, and one the
 * database fails to keep `io`. */
export function serve_objects(link: Link): void {
  object.mount(link, {
    async apply(batch) {
      try {
        await OBJECTS.commit(link.bytes(batch.at, batch.len).slice());
      } catch (e) {
        if (e instanceof Bug) throw e;
        // Only the decode throws an XputeError; the database throws its own.
        throw new ComponentError(
          e instanceof XputeError ? ERROR_CODE_BAD_MESSAGE : ERROR_CODE_IO,
        );
      }
    },
  });
}
