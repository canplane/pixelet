// @pxl/shared/spec/space/sparse_tree/tree.ts
// sparse tree (2^d-ary sparse tree; quadtree/octree are special cases)

import type { i32, u32 } from "@xpute/core/kit/word.ts";
import type { ElementOf, TypedArray, TypedArrayCtor, U64Array } from "@xpute/core/kit/array.ts";
import type { TypedAlloc } from "@xpute/core/ds/typed_alloc.ts";

import { type IVec, ivec, type PVec, type PVecTraits, type PVF3, pvf3 } from "@pxl/shared/spec/space/pvf3/mod.ts";

import type { SparseTreeLayout, SparseTreeMemory } from "./mem.ts";
import * as st_mem from "./mem.ts";

export type { SparseTreeLayout } from "./mem.ts";

const PVF3_REG: PVF3 = { i: 0n, f: 0n };

export type SparseTree<T extends TypedArray> = {
  readonly layout: SparseTreeLayout;
  mem: SparseTreeMemory<T>;
};

// ---- Lifecycle & Codec ----

/**
 * Convert PVF3 to a sparse-tree leaf key at min resolution.
 *
 * Semantics:
 * - sparse-tree traversal consumes the integer lattice key only
 * - PVF3 fractional residue is discarded by shifting down to the leaf scale
 */
const to_key = (coord: PVF3, min: i32): IVec => pvf3.sh(PVF3_REG, coord, -min).i;

/** Allocate a sparse tree with the given layout and initial capacity, its
 * lanes from `typed` (the heap unless given). */
export function alloc<T extends TypedArray>(layout: SparseTreeLayout, cap: u32, ctor: TypedArrayCtor<T>, typed?: TypedAlloc): SparseTree<T> {
  return { layout, mem: st_mem.alloc(layout.dim, cap, ctor, typed) };
}

/** Hands the tree's lanes back. The tree must not be used afterwards. */
export function free<T extends TypedArray>(tree: SparseTree<T>): void {
  st_mem.free(tree.mem);
}

/** Moves the tree's leaf lane to another type in place, every value carried
 * across — widening a lane whose values outgrew it. */
export function retype<T extends TypedArray, U extends TypedArray>(tree: SparseTree<T>, ctor: TypedArrayCtor<U>): void {
  (tree as unknown as SparseTree<U>).mem = st_mem.retype(tree.mem, ctor);
}

/** Encode the sparse tree backing memory into the wire/storage blob format. */
export function encode<T extends TypedArray>(tree: SparseTree<T>): U64Array {
  return st_mem.encode(tree.mem);
}

/** U64Array -> SparseTree<T>. Decode a wire/storage blob into a sparse tree with the given layout. */
export function decode<T extends TypedArray>(
  buf: U64Array,
  layout: SparseTreeLayout,
  ctor: TypedArrayCtor<T>,
  typed?: TypedAlloc,
): SparseTree<T> {
  const mem = st_mem.decode(buf, ctor);
  return { layout, mem: typed ? st_mem.adopt(mem, typed) : mem };
}

/** SparseTree<T> -> T. Reconstruct the dense array from an already-decoded sparse tree. */
export function dense<K extends PVec, T extends TypedArray>(
  tree: SparseTree<T>,
  traits: PVecTraits<K>,
): T {
  return st_mem.dense(tree.mem, traits, tree.layout);
}

// ---- I/O ----

/**
 * Write one value at the given PVF3 coordinate. `level` (default: finest,
 * `tree.layout.min`) claims a whole block at that coarser shift amount in
 * one call instead of always writing a single finest-leaf cell — see
 * st_mem.set's own doc comment.
 */
export function write<T extends TypedArray>(tree: SparseTree<T>, coord: PVF3, val: ElementOf<T>, level: i32 = tree.layout.min): void {
  const key: IVec = to_key(coord, tree.layout.min);
  st_mem.set(tree.mem, ivec, tree.layout, key, val, level);
}

/** Read one value at the given PVF3 coordinate. */
export function read<T extends TypedArray>(tree: SparseTree<T>, coord: PVF3): ElementOf<T> {
  const key: IVec = to_key(coord, tree.layout.min);
  return st_mem.get(tree.mem, ivec, tree.layout, key);
}

/** Reset the tree to a uniform root value. */
export function fill<T extends TypedArray>(tree: SparseTree<T>, val: ElementOf<T>): void {
  st_mem.fill(tree.mem, tree.layout, val);
}

/** Compact the tree in-place by pruning and relocating live blocks. */
export function compact<T extends TypedArray>(tree: SparseTree<T>): SparseTree<T> {
  const old = tree.mem;
  tree.mem = st_mem.compact(old, tree.layout, 0).mem;
  st_mem.free(old);
  return tree;
}

/**
 * Visit every maximal uniform leaf block — see st_mem.walk's own doc
 * comment. `x`/`y`/`z` are in min-level lattice units (multiply by
 * `2**tree.layout.min` for a real-unit corner); `level` is that block's
 * own shift amount (`2**level` is its real-unit size).
 */
export function walk<T extends TypedArray>(
  tree: SparseTree<T>,
  visit: (x: i32, y: i32, z: i32, level: i32, val: ElementOf<T>) => void,
): void {
  st_mem.walk(tree.mem, tree.layout, visit);
}

/**
 * Replace tree contents from a dense leaf array.
 *
 * Dense layout contract:
 * - dim=2: row-major, x contiguous inside each row
 * - dim=3: slab-major [z][y][x], x contiguous inside each row
 *
 * This is the preferred path when the producer already rasterized into a dense buffer.
 */
export function build_dense<T extends TypedArray>(
  tree: SparseTree<T>,
  leaves: ArrayLike<ElementOf<T>>,
): SparseTree<T> {
  const ctor = tree.mem.data.constructor as TypedArrayCtor<T>;
  tree.mem = st_mem.alloc(tree.layout.dim, leaves.length, ctor);

  const root: u32 = st_mem.build(tree.mem, tree.layout, leaves);
  tree.mem = st_mem.compact(tree.mem, tree.layout, root).mem;
  return tree;
}
