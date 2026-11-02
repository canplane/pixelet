// @pxl/client/driver/namespace/object_tree.ts

/**
 * The placed objects' tree as their source keeps it beside the kernel: a
 * leaf's shard and its digits past the shard, a write laid over a shard's
 * leaves, and what a node at `SHARD_ZOOM` folds to, the top of the tree.
 * The kernel's store owns the tree (pxl-kernel
 * domain/layer/object/store.rs); what is here is its rules held a second
 * time, ported, and held to the store by one record both read
 * (rust/spec/golden/object/top.tsv): the keys' layout (pxl-spec
 * space/hierarchy/morton.rs `key_of`), a write's reach (pxl-spec
 * space/hierarchy/sparse_tree.rs `set`), and the fold (pxl-engine
 * overwrite.rs `fold`, `share`, `voxel`; answer.rs `FULL`). The values a
 * fold counts as filling are generated (abi/voxel.spec.ts).
 */

import { SHARD_ZOOM } from "@/abi/host.spec.ts";
import { AIR, grounds, objects } from "@/abi/voxel.spec.ts";

/** A write `<key, id, ext>` (host.json `write`), and a leaf as the write that
 * made it; a marker's text its one record. */
export interface Leaf {
  key: bigint;
  id: number;
  text: string | null;
}

/** A row of the top (host.json `top`): a node at `SHARD_ZOOM` holding
 * objects, by its shard's subject and its Z, and what it folds to. */
export interface TopRow {
  shard: bigint;
  z: number;
  fold: number;
}

/** The value an erase writes, and the empty word (pxl-engine
 * `cube::sentinel::void`). */
export const VOID = 0;

/** The digits a shard's key has: two a zoom, X's and Y's, to `SHARD_ZOOM`. */
const SHARD_DIGITS = 2 * SHARD_ZOOM;
/** Past a shard a zoom adds a digit on each of three axes. */
const AXES = 3;

/** A key's digits, before the 1 that ends them. */
export function digits(key: bigint): number {
  let low = 0;
  while (((key >> BigInt(low)) & 1n) === 0n) low++;
  return 126 - low;
}

/** `key`'s first `n` digits, as a number. */
function prefix(key: bigint, n: number): bigint {
  return key >> BigInt(127 - n);
}

/** The key whose digits are the `n` of `p`. */
function key_of(p: bigint, n: number): bigint {
  return (p << BigInt(127 - n)) | (1n << BigInt(126 - n));
}

/** The shard a leaf lies in, by subject: its key's digits to the shard and
 * the 1 after them, its high half (pxl-spec space/layer/tile.rs `subject`). */
export function shard_of(key: bigint): bigint {
  return key_of(prefix(key, SHARD_DIGITS), SHARD_DIGITS) >> 64n;
}

/** How many zooms past the shard a leaf's node is. */
function depth(key: bigint): number {
  return (digits(key) - SHARD_DIGITS) / AXES;
}

/** The child a leaf lies under at the `j`th zoom past the shard, from 1: its
 * three digits there, Z's highest, the order of a node's children (pxl-spec
 * space/hierarchy/tree.rs `slot_of`). At the first, Z's digit is its sign:
 * below sea level or above. */
function slot(key: bigint, j: number): number {
  return Number(prefix(key, SHARD_DIGITS + AXES * j) & 7n);
}

/** `w` laid over `leaves`, a shard's by key, none over another: its node made
 * one leaf of its id, none for VOID, every leaf at it or under it gone, and
 * a leaf over it split along the path to it, each child off the path its
 * value, as the store's tree takes a write. */
export function lay(leaves: Map<bigint, Leaf>, w: Leaf): void {
  const n = digits(w.key);
  for (const [k, l] of leaves) {
    const m = digits(k);
    if (m >= n && prefix(k, n) === prefix(w.key, n)) {
      leaves.delete(k);
    } else if (m < n && prefix(w.key, m) === prefix(k, m)) {
      leaves.delete(k);
      for (let d = m + AXES; d <= n; d += AXES) {
        const own = prefix(w.key, d);
        for (let s = 0n; s < 8n; s++) {
          const child = ((own >> 3n) << 3n) | s;
          if (child === own) continue;
          const key = key_of(child, d);
          leaves.set(key, { key, id: l.id, text: l.text });
        }
      }
    }
  }
  if (w.id !== VOID) leaves.set(w.key, w);
}

/** A cube filled whole, in a share's units (pxl-engine answer.rs `FULL`). */
const FULL = 1 << 15;

/** A fold's word: its kind low, its share high (overwrite.rs `fold_word`). */
function fold_word(kind: number, share: number): number {
  return ((kind & 0xffff) | (share << 16)) >>> 0;
}

/** Whether a value is drawn as a voxel (overwrite.rs `voxel`). */
function voxel(val: number): boolean {
  return val === AIR || grounds(val) || objects(val);
}

/** A leaf's word in its parent's fold (overwrite.rs `share`): a voxel fills
 * it whole, anything else none. */
function share(id: number): number {
  return voxel(id) ? fold_word(id, FULL) : 0;
}

/** A node's eight children's words folded, the node at `zoom`
 * (overwrite.rs `fold`): the mean of their shares, and the kind most of
 * that fill is, a tie to the child met first, corners taken in an order
 * that alternates with the zoom. */
export function fold(children: readonly number[], zoom: number): number {
  const kind = (w: number): number => w & 0xffff;
  const fill = (w: number): number => w >>> 16;
  let sum = 0;
  for (const c of children) sum += fill(c);
  let best = 0;
  let most = 0;
  for (let j = 0; j < 8; j++) {
    const c = children[(zoom & 1) === 0 ? 7 - j : j]!;
    let w = 0;
    for (const o of children) if (kind(o) === kind(c)) w += fill(o);
    if (fill(c) > 0 && w > most) {
      best = kind(c);
      most = w;
    }
  }
  return fold_word(best, Math.floor(sum / 8));
}

/** The word the node `j` zooms past the shard holding `under` has in the
 * tree: a leaf's share where one is the node, else its children folded. */
function word(under: readonly Leaf[], j: number): number {
  if (under.length === 1 && depth(under[0]!.key) === j) {
    return share(under[0]!.id);
  }
  const children: Leaf[][] = Array.from({ length: 8 }, () => []);
  for (const l of under) children[slot(l.key, j + 1)]!.push(l);
  return fold(
    children.map((c) => (c.length > 0 ? word(c, j + 1) : 0)),
    SHARD_ZOOM + j,
  );
}

/** The top's rows of the shard `shard` holding `leaves`: each of its two
 * nodes at `SHARD_ZOOM` holding any, below sea level (Z -1) then above (Z
 * 0), and what it folds to. */
export function top_of(shard: bigint, leaves: Iterable<Leaf>): TopRow[] {
  const below: Leaf[] = [];
  const above: Leaf[] = [];
  for (const l of leaves) ((slot(l.key, 1) & 4) !== 0 ? below : above).push(l);
  const rows: TopRow[] = [];
  if (below.length > 0) rows.push({ shard, z: -1, fold: word(below, 0) });
  if (above.length > 0) rows.push({ shard, z: 0, fold: word(above, 0) });
  return rows;
}
