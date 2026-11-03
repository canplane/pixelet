// @pxl/client/driver/namespace/object.ts

/**
 * OBJECT's backend: the placed objects' source, for now a loopback over the
 * place's database (bracket `db`). It serves what the kernel reads, a shard's
 * file by the shard's subject and the top of the tree (host.json
 * `object.TOP`), and takes what it writes, edit batches (`object.EDITS`): it
 * lays each batch's writes over the shards they lie in, keeps them, and
 * rebuilds the top's rows of those shards (object_tree.ts), in one
 * transaction. A shard is kept under a key a shard, named as the shard's tile
 * is, and the top under its own; a shard with none, or a top with no row, has
 * no file. A server answers the same `open` and `commit` in its place
 * (namespace.ts): the kernel cannot tell which. A batch that is not kept is
 * logged: a commit has no answer to the kernel.
 */

import type {
  I32Array,
  U32Array,
  U64Array,
  U8Array,
} from "@xpute/kit/abi/array.ts";
import * as Errno from "@xpute/kit/status/errno.spec.ts";
import { FileError } from "@xpute/host/ipc/sys.ts";
import { TreeEncoder, TreeReader, TreeView } from "@xpute/kit/wire/xtp/mod.ts";

import { format, object } from "@/abi/host.spec.ts";
import {
  lay,
  shard_of,
  top_of,
  type Leaf,
  type TopRow,
} from "@/driver/namespace/object_tree.ts";
import { bracket, type Tx } from "@/bracket.tsx";
import { tile_name } from "@/driver/namespace/tile.ts";

const TOP = BigInt(object.TOP);
const EDITS = BigInt(object.EDITS);

/** The key the top is kept under; a tile's name is digits. */
export const TOP_KEY = "top";

/** The key OBJECT's object `number` is kept under: the top's, or a shard's
 * tile's name; ENOENT for a number that is neither. */
export function object_key(number: bigint): string {
  if (number === TOP) return TOP_KEY;
  const name = number === EDITS ? null : tile_name(number);
  if (name === null) throw new FileError(Errno.ENOENT);
  return name;
}

/** The tree packet a file of `fmt` holds past its format byte; EBADMSG for
 * one of another format. */
function packet(fmt: number, bytes: U8Array): U8Array {
  if (bytes[0] !== fmt) throw new FileError(Errno.EBADMSG);
  // A packet starts on a word: its own copy does.
  return bytes.slice(1);
}

/** `pkt` after `fmt`, as a file. */
function as_file(fmt: number, pkt: U8Array): U8Array {
  const out = new Uint8Array(1 + pkt.byteLength);
  out[0] = fmt;
  out.set(pkt, 1);
  return out;
}

/** The writes a file of `fmt` holds (host.json `write`), in order. */
export function decode_writes(fmt: number, bytes: U8Array): Leaf[] {
  const [hi, lo, id, rows, texts] = new TreeReader(packet(fmt, bytes))
    .read_branch()
    .get<[U64Array, U64Array, U32Array, U32Array, string[]]>();
  if (hi.length !== lo.length || hi.length !== id.length) {
    throw new FileError(Errno.EINVAL);
  }
  const out: Leaf[] = Array.from(id, (v, r) => ({
    key: (hi[r]! << 64n) | lo[r]!,
    id: v,
    text: null,
  }));
  rows.forEach((r, k) => {
    if (out[r] === undefined || texts[k] === undefined) {
      throw new FileError(Errno.EINVAL);
    }
    out[r]!.text = texts[k]!;
  });
  return out;
}

/** `leaves` as a file of `fmt` (host.json `write`). */
export function encode_writes(fmt: number, leaves: readonly Leaf[]): U8Array {
  const rows: number[] = [];
  const texts: string[] = [];
  leaves.forEach((l, r) => {
    if (l.text === null) return;
    rows.push(r);
    texts.push(l.text);
  });
  const tree = new TreeView();
  // In host.json `write`'s order.
  tree.branch((b) =>
    b
      .u64_array(BigUint64Array.from(leaves, (l) => l.key >> 64n))
      .u64_array(
        BigUint64Array.from(leaves, (l) => l.key & 0xffff_ffff_ffff_ffffn),
      )
      .u32_array(Uint32Array.from(leaves, (l) => l.id))
      .u32_array(Uint32Array.from(rows))
      .strs(texts),
  );
  return as_file(fmt, new TreeEncoder().encode(tree, { init_cap: 1 << 12 }));
}

/** The top's rows (host.json `top`). */
export function decode_top(bytes: U8Array): TopRow[] {
  const [shard, z, fold] = new TreeReader(packet(format.OBJECT_TOP, bytes))
    .read_branch()
    .get<[U64Array, I32Array, U32Array]>();
  if (shard.length !== z.length || shard.length !== fold.length) {
    throw new FileError(Errno.EINVAL);
  }
  return Array.from(shard, (s, r) => ({ shard: s, z: z[r]!, fold: fold[r]! }));
}

/** `rows` as the top (host.json `top`). */
export function encode_top(rows: readonly TopRow[]): U8Array {
  const tree = new TreeView();
  // In host.json `top`'s order.
  tree.branch((b) =>
    b
      .u64_array(BigUint64Array.from(rows, (r) => r.shard))
      .i32_array(Int32Array.from(rows, (r) => r.z))
      .u32_array(Uint32Array.from(rows, (r) => r.fold)),
  );
  return as_file(
    format.OBJECT_TOP,
    new TreeEncoder().encode(tree, { init_cap: 1 << 12 }),
  );
}

/** By shard, then below sea level before above. */
function by_shard(a: TopRow, b: TopRow): number {
  return a.shard === b.shard ? a.z - b.z : a.shard < b.shard ? -1 : 1;
}

/** The database as the bracket gives it. */
export type Db = { transaction<T>(fn: (tx: Tx) => Promise<T>): Promise<T> };

/** The bracket's, read at each call: the bracket is set after this loads. */
const DB: Db = { transaction: (fn) => bracket.db.transaction(fn) };

/** Moves what `key` holds to `<key>:aside:<ms>`, none of an older one lost. */
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

/** The top's rows as kept; a top of another build set aside, none. */
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

async function put_top(tx: Tx, rows: readonly TopRow[]): Promise<void> {
  await tx.put(TOP_KEY, rows.length === 0 ? null : encode_top(rows));
}

/** `writes`: every shard they reach read, laid over each in order, then each
 * kept with its rows of the top; nothing is kept where a shard does not read. */
async function apply_edits(tx: Tx, writes: readonly Leaf[]): Promise<void> {
  const shards = new Map<bigint, Map<bigint, Leaf>>();
  for (const w of writes) {
    const shard = shard_of(w.key);
    let leaves = shards.get(shard);
    if (leaves === undefined) {
      const file = await tx.get(object_key(shard));
      const own = file === null ? [] : decode_writes(format.OBJECT, file);
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
      object_key(shard),
      own.length === 0 ? null : encode_writes(format.OBJECT, own),
    );
    others.push(...top_of(shard, own));
  }
  others.sort(by_shard);
  await put_top(tx, others);
}

/** OBJECT's backend over `db`: a file read whole, ENOENT where none is kept,
 * after every batch committed before it; an edit batch applied in the order
 * written; a file the kernel refused set aside. */
export function object_backend(db: Db = DB): {
  open(number: bigint): Promise<U8Array>;
  commit(number: bigint, bytes: U8Array): void;
  set_aside(number: bigint, now_ms: number): Promise<boolean>;
} {
  return {
    open(number) {
      return db.transaction(async (tx) => {
        const bytes = await tx.get(object_key(number));
        if (bytes === null) throw new FileError(Errno.ENOENT);
        return bytes;
      });
    },
    commit(number, bytes) {
      if (number !== EDITS) throw new FileError(Errno.EROFS);
      const writes = decode_writes(format.OBJECT_EDITS, bytes);
      db.transaction((tx) => apply_edits(tx, writes)).catch((err: unknown) => {
        console.error("[object] an edit batch was not kept:", err);
      });
    },
    /** A file the kernel refused set aside and emptied; a shard's rows of
     * the top go with it. False, nothing changed, where the database failed. */
    set_aside(number, now_ms) {
      const key = object_key(number);
      return db
        .transaction(async (tx) => {
          await move_aside(tx, key, now_ms);
          if (number !== TOP) {
            const left = (await rows_of(tx)).filter((r) => r.shard !== number);
            await put_top(tx, left);
          }
          return true;
        })
        .catch(() => false);
    },
  };
}

/** The one OBJECT is bound to (namespace.ts), and refusals are set aside by
 * (refusal.ts). */
export const OBJECTS = object_backend();
