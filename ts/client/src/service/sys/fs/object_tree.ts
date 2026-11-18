// @pxl/client/service/sys/fs/object_tree.ts

/**
 * The rules of the object tree that the OBJECT backend (object.ts)
 * needs: which shard a leaf lies in, how a write is laid over a shard's
 * leaves, and what a shard folds to in the top of the tree. A leaf is named
 * by its octkey, and the first `SHARD_ZOOM` digits of an octkey are the
 * quadkey of its shard.
 *
 * The core's store owns these rules (pxl-core domain/layer/object/store.rs).
 * This file is a port of them, held equal to the store by a record that the
 * tests of both read (spec/golden/object/top.tsv). The rules ported are
 * an octkey's digits (pxl-spec space/hierarchy/morton.rs `name`), the nodes a
 * write reaches (pxl-core domain/layer/object/sparse_tree.rs `set`), and the fold
 * (pxl-engine object.rs `fold`, `share` and `voxel`, and pxl-spec share.rs `FULL`).
 * The values a fold counts as filling are the cube's (abi/cube.spec.ts).
 *
 * The backend stands in for a server. Its rules are a copy, never a second
 * owner: they change only to follow the store.
 */

import { SHARD_ZOOM } from "@/abi/file.spec.ts";
import {
  ground,
  object,
  SENTINEL_AIR,
  SENTINEL_VOID,
} from "@/abi/cube.spec.ts";

/** A write, laid out as file.json5's `write`. A leaf is kept as the write that
 * made it. `text` is a marker's words, the one record a leaf can carry. */
export interface Leaf {
  key: string;
  assetId: number;
  text: string | null;
}

/** A row of the top, laid out as file.json5's `top`. Only a shard that holds
 * objects has a row, which gives the shard's quadkey and what it folds to. */
export interface TopRow {
  shard: string;
  fold: number;
}

export function shardOf(key: string): string {
  return key.slice(0, SHARD_ZOOM);
}

/** How many zooms past the shard a leaf's node is. */
function depth(key: string): number {
  return key.length - SHARD_ZOOM;
}

/** Which of its node's eight children a leaf lies under, `j` zooms past the
 * shard, counting from 1. The tree numbers a node's children `x + 2y + 4z`
 * (pxl-spec space/hierarchy/tree.rs `child_idx`), and the octkey's digit at
 * that zoom gives the number. At the first zoom past the shard, the digit's
 * Z bit is the sign of Z, while the tree counts Z up from the world's floor
 * (pxl-engine object.rs `path`) and puts the half below sea level first. So
 * that bit is flipped there. */
function slot(key: string, j: number): number {
  return Number(key[SHARD_ZOOM + j - 1]) ^ (j === 1 ? 4 : 0);
}

/** Lays the write `w` over `leaves`, the leaves of `w`'s shard by key, as the
 * store's tree takes a write. The write replaces everything at or under its
 * node. A leaf above its node is split, so that the rest of that leaf keeps
 * its value. A write of `SENTINEL_VOID` leaves no leaf. */
export function lay(leaves: Map<string, Leaf>, w: Leaf): void {
  for (const [k, l] of leaves) {
    if (k.startsWith(w.key)) {
      leaves.delete(k);
    } else if (w.key.startsWith(k)) {
      leaves.delete(k);
      for (let d = k.length + 1; d <= w.key.length; d++) {
        const parent = w.key.slice(0, d - 1);
        for (let s = 0; s < 8; s++) {
          const key = `${parent}${s}`;
          if (key !== w.key.slice(0, d)) {
            leaves.set(key, { key, assetId: l.assetId, text: l.text });
          }
        }
      }
    }
  }
  if (w.assetId !== SENTINEL_VOID) leaves.set(w.key, w);
}

/** A share is how much of a cube is filled, and this is a whole cube
 * (pxl-spec share.rs `FULL`). */
const FULL = 1 << 15;

/** A fold's word holds its kind in the low 16 bits and its share in the high
 * 16 (pxl-engine object.rs `fold_word`). */
function foldWord(kind: number, share: number): number {
  return ((kind & 0xffff) | (share << 16)) >>> 0;
}

/** The ground's and the object numbers, the cube catalog's rows. */
const CUBES: ReadonlySet<number> = new Set(
  [...ground, ...object].map((c) => c.number),
);

/** Whether a value is drawn as a voxel (pxl-engine object.rs `voxel`). */
function voxel(value: number): boolean {
  return value === SENTINEL_AIR || CUBES.has(value);
}

/** The word a leaf of `id` gives its parent's fold (pxl-engine object.rs
 * `share`). A voxel fills its cube whole, and any other value fills none of
 * it. */
function share(id: number): number {
  return voxel(id) ? foldWord(id, FULL) : 0;
}

/** Folds the words of a node's eight children into the word of the node, which
 * is at `zoom` (pxl-engine object.rs `fold`). The node's share is the mean of
 * the children's shares. Its kind is the one that makes up most of that fill.
 * A tie goes to the child met first, and the order the children are met in
 * reverses at every other zoom. */
export function fold(children: readonly number[], zoom: number): number {
  const kind = (w: number): number => w & 0xffff;
  const fill = (w: number): number => w >>> 16;
  let sum = 0;
  for (const c of children) sum += fill(c);
  let best: number = SENTINEL_VOID;
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
  return foldWord(best, Math.floor(sum / 8));
}

/** The word in the tree of the node that lies `j` zooms past the shard and
 * holds the leaves `under`. */
function word(under: readonly Leaf[], j: number): number {
  if (under.length === 1 && depth(under[0]!.key) === j) {
    return share(under[0]!.assetId);
  }
  const children: Leaf[][] = Array.from({ length: 8 }, () => []);
  for (const l of under) children[slot(l.key, j + 1)]!.push(l);
  return fold(
    children.map((c) => (c.length > 0 ? word(c, j + 1) : 0)),
    SHARD_ZOOM + j,
  );
}

export function topOf(shard: string, leaves: Iterable<Leaf>): TopRow[] {
  const under = [...leaves];
  return under.length > 0 ? [{ shard, fold: word(under, 0) }] : [];
}
