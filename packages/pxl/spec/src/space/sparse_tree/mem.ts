// @pxl/spec/space/sparse_tree/mem.ts
// sparse tree memory primitives

/**
 * sparse tree memory primitives (stateless / data-oriented)
 * unified sparse 2^d-ary tree algorithms (top-down + bottom-up)
 *
 * @architecture
 * 1) split storage: heap(u32 ptrs) + data(T) are separated but index-aligned
 * 2) pointer tagging: MSB marks branch pointer (non-recursive traversal)
 * 3) algorithms over POD arenas (STL-style)
 * 4) subtree slicing: resolve() returns a leaf slot for O(1) subtree-local access
 *
 * @naming
 * - base  : first slot of one child block
 * - slot  : one concrete entry inside a block
 * - head  : allocator cursor (next free slot)
 * - block : contiguous child group of size (1 << dim)
 *
 * The structure does NOT model a node as one scalar handle.
 * The structural unit is a child block:
 * - dim=2 => one block = 4 child slots
 * - dim=3 => one block = 8 child slots
 *
 * Therefore:
 * - traversal state usually carries a block base
 * - reads/writes usually end at one leaf slot
 * - tagged heap entries point to another child block base
 */

// sparse_tree public contract
// 1. layout.dim is restricted to 2 | 3 only.
// 2. layout.max must be strictly greater than layout.min.
// 3. the structural unit is a child block, not a scalar node.
// 4. root is the base slot of the top-level child block.
// 5. read/write operate on leaf slots derived from PVF3 -> IVec key lowering.
// 6. heap slot: MSB=1 means branch pointer to another child-block base.
// 7. heap slot: MSB=0 means leaf; payload lives in data[slot].
// 8. fill() sets the root block to one uniform leaf value.
// 9. compact() preserves semantics and returns a tree whose root base is 0.
// 10. encode()/decode() use U64Array as the sparse-tree wire/storage blob surface.

import type { i32, u32 } from "@xpute/spec/lib/kit/word.ts";
import type { ElementOf, TypedArray, TypedArrayCtor, U32Array, U64Array, U8Array } from "@xpute/spec/lib/kit/array.ts";
import { copy_ab_view } from "@xpute/spec/lib/kit/array.ts";

import type { PVec, PVecTraits } from "@pxl/spec/space/pvf3/mod.ts";
import { ALIGN, GET_WORD, NONE, RESERVED, SET_WORD, WORD_SZ } from "@xpute/spec/io/marshal/tree/spec.ts";

/**
 * Scratchpad for 64-bit word operations to avoid allocation.
 * [lo32, hi32]
 */
const WORD_REG: [u32, u32] = [NONE, NONE];

// ============ Types ============

/**
 * Sparse tree backing memory.
 *
 * Invariants:
 * - heap and data are slot-aligned arenas sharing the same logical index space
 * - slot 0..(1<<dim)-1 is reserved for the root child block
 * - head is the next free slot after the last allocated block
 */
export type SparseTreeMemory<T extends TypedArray> = {
  heap: U32Array; // tagged child pointers (branch => MSB set, leaf => 0)
  data: T; // leaf payload values
  head: u32; // allocator cursor (next free slot)
  cap: u32; // current capacity in slots
};

/**
 * Sparse tree layout.
 *
 * Level model:
 * - min = leaf level
 * - max = root level
 * - dep = max - min
 *
 * Structural interpretation:
 * - one descent step consumes one bit per active axis
 * - therefore one internal block always has (1 << dim) children
 */
export type SparseTreeLayout = {
  readonly dim: 2 | 3; // dimension (2 or 3)

  // Signed log2 scale used by the layout contract.
  // - min can be negative (e.g. -2 => 1m if 0 means "1 lattice unit")
  // - core never shifts by signed values; it normalizes to u = level - min (u>=0).
  readonly min: i32; // leaf level (signed)
  readonly max: i32; // root level (signed). dep = max - min
};

// ============ Allocator ============

/**
 * Tagged heap entry layout:
 * - MSB = 1 => branch: remaining bits store child block base
 * - MSB = 0 => leaf  : payload is stored directly in data[slot]
 */
const NODE_MSB: u32 = 0x80000000;
const BLOCK_MASK: u32 = 0x7fffffff;

/** Number of child slots in one block for the current dimension. */
const block_width = (dim: 2 | 3): u32 => 1 << dim;

/**
 * Validate structural layout.
 *
 * Invariants:
 * - only quadtree(dim=2) and octree(dim=3) are supported
 * - dep = max - min must be >= 1
 * - therefore the root is always one top-level child block, never a scalar leaf
 */
function validate_layout(layout: SparseTreeLayout): void {
  const dep: i32 = layout.max - layout.min;
  if (dep <= 0) {
    throw new Error(`sparse_tree: bad layout: expected max > min, got min=${layout.min}, max=${layout.max}`);
  }
}

/** Allocate sparse tree memory and reserve the root child block at base 0. */
export function alloc<T extends TypedArray>(dim: 2 | 3, cap: u32, ctor: TypedArrayCtor<T>): SparseTreeMemory<T> {
  const root_width: u32 = block_width(dim);
  cap = cap >= root_width ? cap : root_width;

  return {
    heap: new Uint32Array(cap),
    data: new ctor(cap) as T,
    head: root_width, // reserve base 0..root_width-1 for root block
    cap,
  };
}

/** Grow backing buffers so that slot range [0, end) fits. */
function grow<T extends TypedArray>(mem: SparseTreeMemory<T>, end: u32): void {
  let cap: u32 = mem.cap;
  while (cap < end) cap <<= 1;

  const heap: U32Array = new Uint32Array(cap);
  const ctor = mem.data.constructor as unknown as TypedArrayCtor<T>;
  const data: T = new ctor(cap);

  heap.set(mem.heap);
  data.set(mem.data);

  mem.heap = heap;
  mem.data = data;
  mem.cap = cap;
}

/** Allocate an arbitrary contiguous slot span and return its first slot. */
function alloc_slots<T extends TypedArray>(mem: SparseTreeMemory<T>, cnt: u32): u32 {
  const base: u32 = mem.head;
  const end: u32 = base + cnt;

  if (end > mem.cap) grow(mem, end);

  mem.head = end;
  mem.heap.fill(0, base, end);
  mem.data.fill(0 as ElementOf<T>, base, end);
  return base;
}

/**
 * Allocate exactly one child block and return its base slot.
 *
 * Postconditions:
 * - returned range [base, base+block_width(dim)) is reserved
 * - heap/data in that block are zero-initialized
 */
function alloc_block<T extends TypedArray>(mem: SparseTreeMemory<T>, dim: 2 | 3): u32 {
  return alloc_slots(mem, block_width(dim));
}

/** Reset to the root-only state. */
export function reset<T extends TypedArray>(mem: SparseTreeMemory<T>, layout: SparseTreeLayout): void {
  validate_layout(layout);

  const root_width: u32 = block_width(layout.dim);
  mem.head = root_width;
  mem.heap.fill(0, 0, root_width);
  mem.data.fill(0 as ElementOf<T>, 0, root_width);
}

/** Reset the tree and fill the root block with one uniform value. */
export function fill<T extends TypedArray>(mem: SparseTreeMemory<T>, layout: SparseTreeLayout, val: ElementOf<T>): void {
  reset(mem, layout);
  mem.data.fill(val, 0, mem.head);
}

// ============ Traversal ============

/**
 * Resolve the leaf slot corresponding to `key` at `target_shift`.
 *
 * Semantics:
 * - traversal state carries a current block base
 * - each step selects one child slot inside that block
 * - if traversal hits a uniform leaf before target depth, the current slot is returned
 *
 * Return value:
 * - always one concrete slot, never a block base for the next level unless target landed there
 */
export function resolve<K extends PVec, T extends TypedArray>(
  mem: SparseTreeMemory<T>,
  traits: PVecTraits<K>,
  layout: SparseTreeLayout,
  key: K,
  target_shift: i32 = layout.min,
  root: u32 = 0,
): u32 {
  validate_layout(layout);

  const dep: i32 = layout.max - layout.min;
  const target_u: i32 = target_shift - layout.min;

  if (target_u < 0) return root;
  if (target_u >= dep) return root;

  let base: u32 = root;

  for (let u = dep - 1; u >= target_u; u--) {
    const shifted: K = traits.sh(key, -u);

    let child: u32 = Number(traits.X(shifted) & 1n);
    if (layout.dim >= 2) child |= Number(traits.Y(shifted) & 1n) << 1;
    if (layout.dim >= 3) child |= Number(traits.Z(shifted) & 1n) << 2;

    const slot: u32 = base + child;
    const node: u32 = mem.heap[slot];

    if (u === target_u) return slot;
    if (!(node & NODE_MSB)) return slot;

    base = node & BLOCK_MASK;
  }

  return base;
}

/** Read one leaf value from the tree. */
export function get<K extends PVec, T extends TypedArray>(
  mem: SparseTreeMemory<T>,
  traits: PVecTraits<K>,
  layout: SparseTreeLayout,
  key: K,
  root: u32 = 0,
): ElementOf<T> {
  validate_layout(layout);

  const slot: u32 = resolve(mem, traits, layout, key, layout.min, root);
  const node: u32 = mem.heap[slot];

  if (node & NODE_MSB) throw new Error("sparse_tree: expected leaf slot, got branch");
  return mem.data[slot] as ElementOf<T>;
}

/**
 * Write one value, splitting only when needed.
 *
 * Traversal invariant:
 * - `base` is always the current child-block base
 * - `slot` is the selected child entry inside that block
 */
export function set<K extends PVec, T extends TypedArray>(
  mem: SparseTreeMemory<T>,
  traits: PVecTraits<K>,
  layout: SparseTreeLayout,
  key: K,
  val: ElementOf<T>,
  root: u32 = 0,
): void {
  validate_layout(layout);

  let base: u32 = root;
  const width: u32 = block_width(layout.dim);

  const dep: i32 = layout.max - layout.min;
  if (dep <= 0) return;

  for (let u = dep - 1; u >= 0; u--) {
    const shifted = traits.sh(key, -u);

    let child: u32 = Number(traits.X(shifted) & 1n);
    if (layout.dim >= 2) child |= Number(traits.Y(shifted) & 1n) << 1;
    if (layout.dim >= 3) child |= Number(traits.Z(shifted) & 1n) << 2;

    const slot: u32 = base + child;
    const node: u32 = mem.heap[slot];

    if (node & NODE_MSB) {
      base = node & BLOCK_MASK;
      continue;
    }

    if (mem.data[slot] === val) return;

    if (u === 0) {
      mem.data[slot] = val;
      return;
    }

    const next_base: u32 = alloc_block(mem, layout.dim);
    mem.data.fill(mem.data[slot], next_base, next_base + width);
    mem.heap[slot] = next_base | NODE_MSB;
    base = next_base;
  }
}

// ============ Build ============

/**
 * Bottom-up build from dense leaves.
 *
 * Root invariant:
 * - the sparse-tree root is one top-level child block, not one scalar node
 * - therefore the final returned root must be a block base whose block width is (1 << dim)
 */
export function build<T extends TypedArray>(
  mem: SparseTreeMemory<T>,
  layout: SparseTreeLayout,
  leaves: ArrayLike<ElementOf<T>>,
): u32 {
  validate_layout(layout);

  const width: u32 = block_width(layout.dim);
  const dep: i32 = layout.max - layout.min;

  if (leaves.length === 0) throw new Error("sparse_tree: empty leaves");

  let expect: u32 = 1;
  for (let d = 0; d < dep; d++) expect *= width;
  if (leaves.length !== expect) {
    throw new Error(`sparse_tree: bad leaf count: expected ${expect}, got ${leaves.length}`);
  }

  let cur_base: u32 = alloc_slots(mem, leaves.length);
  mem.data.set(leaves, cur_base);
  let cur_cnt: u32 = leaves.length;

  // Stop when one top-level child block remains.
  while (cur_cnt > width) {
    const parent_cnt: u32 = cur_cnt / width;
    const parent_base: u32 = alloc_slots(mem, parent_cnt);

    for (let i = 0; i < parent_cnt; i++) {
      const parent_slot: u32 = parent_base + i;
      const child_base: u32 = cur_base + i * width;

      let uniform = true;
      const first = mem.data[child_base];

      if (mem.heap[child_base] & NODE_MSB) {
        uniform = false;
      } else {
        for (let k = 1; k < width; k++) {
          if ((mem.heap[child_base + k] & NODE_MSB) || mem.data[child_base + k] !== first) {
            uniform = false;
            break;
          }
        }
      }

      if (uniform) {
        mem.data[parent_slot] = first;
        mem.heap[parent_slot] = 0;
      } else {
        mem.heap[parent_slot] = child_base | NODE_MSB;
      }
    }

    cur_base = parent_base;
    cur_cnt = parent_cnt;
  }

  return cur_base;
}

/** Append one raw child block. */
export function append<T extends TypedArray>(
  mem: SparseTreeMemory<T>,
  layout: SparseTreeLayout,
  vals: ArrayLike<ElementOf<T>>,
): u32 {
  validate_layout(layout);

  const width: u32 = block_width(layout.dim);
  if (vals.length !== width) {
    throw new Error(`sparse_tree: block size mismatch: expected ${width}, got ${vals.length}`);
  }

  const base: u32 = alloc_block(mem, layout.dim);
  mem.data.set(vals, base);
  return base;
}

// ============ Dense ============

/**
 * Reconstruct dense leaf values from a sparse subtree.
 * dim=2 is row-major with x contiguous.
 * dim=3 is slab-major [z][y][x] with x contiguous inside each row.
 */
export function dense<K extends PVec, T extends TypedArray>(
  mem: SparseTreeMemory<T>,
  traits: PVecTraits<K>,
  layout: SparseTreeLayout,
  root: u32 = 0,
): T {
  validate_layout(layout);

  const dep: i32 = layout.max - layout.min;

  const side: u32 = 1 << dep;
  const plane: u32 = side * side;
  const total: u32 = layout.dim === 2 ? plane : (plane * side);

  const ctor = mem.data.constructor as TypedArrayCtor<T>;
  const out = new ctor(total) as T;

  if (layout.dim === 2) {
    let i = 0;
    for (let y = 0; y < side; y++) {
      for (let x = 0; x < side; x++) {
        const key = traits.gather(BigInt(x), BigInt(y), 0n);
        out[i++] = get(mem, traits, layout, key, root);
      }
    }
    return out;
  }

  let i = 0;
  for (let z = 0; z < side; z++) {
    for (let y = 0; y < side; y++) {
      for (let x = 0; x < side; x++) {
        const key = traits.gather(BigInt(x), BigInt(y), BigInt(z));
        out[i++] = get(mem, traits, layout, key, root);
      }
    }
  }
  return out;
}

// ============ Maintenance (Pack) ============

/** Compact the tree by pruning redundant nodes and relocating live blocks. */
export function compact<T extends TypedArray>(
  mem: SparseTreeMemory<T>,
  layout: SparseTreeLayout,
  root: u32,
): { mem: SparseTreeMemory<T>; root: u32 } {
  validate_layout(layout);

  const width: u32 = block_width(layout.dim);

  _prune(mem, root, layout.max - layout.min, width);

  const new_mem = alloc(layout.dim, mem.head, mem.data.constructor as TypedArrayCtor<T>);

  const relocate = (old_base: u32, new_base: u32): void => {
    for (let i = 0; i < width; i++) {
      const old_slot: u32 = old_base + i;
      const new_slot: u32 = new_base + i;
      const node: u32 = mem.heap[old_slot];

      if (node & NODE_MSB) {
        const child_base: u32 = node & BLOCK_MASK;
        const next_base: u32 = alloc_block(new_mem, layout.dim);

        new_mem.heap[new_slot] = next_base | NODE_MSB;
        relocate(child_base, next_base);
      } else {
        new_mem.data[new_slot] = mem.data[old_slot];
      }
    }
  };

  relocate(root, 0);

  const compacted = alloc(layout.dim, new_mem.head, mem.data.constructor as TypedArrayCtor<T>);

  compacted.heap.set(new_mem.heap.subarray(0, new_mem.head));
  compacted.data.set(new_mem.data.subarray(0, new_mem.head));
  compacted.head = new_mem.head;

  return { mem: compacted, root: 0 };
}

/**
 * Internal recursive prune.
 *
 * Return:
 * - uniform leaf value if the whole block can collapse into one value
 * - null otherwise
 */
function _prune<T extends TypedArray>(
  mem: SparseTreeMemory<T>,
  base: u32,
  dep: i32,
  width: u32,
): ElementOf<T> | null {
  if (dep === 0) return null;

  let uniform = true;
  let first: ElementOf<T> | null = null;

  for (let i = 0; i < width; i++) {
    const slot: u32 = base + i;
    const node: u32 = mem.heap[slot];
    let val: ElementOf<T> | null = (node & NODE_MSB) ? null : (mem.data[slot] as ElementOf<T>);

    if (node & NODE_MSB) {
      const child_base: u32 = node & BLOCK_MASK;
      const child_val: ElementOf<T> | null = _prune(mem, child_base, dep - 1, width);

      if (child_val !== null) {
        mem.heap[slot] = 0;
        mem.data[slot] = child_val;
        val = child_val;
      } else {
        uniform = false;
      }
    }

    if (first === null) {
      first = val;
    } else if (val === null || first !== val) {
      uniform = false;
    }
  }

  return uniform && first !== null ? first : null;
}

// ============ Memory I/O ============

function total_nbyte<T extends TypedArray>(mem: SparseTreeMemory<T>, data_bpe: u32): u32 {
  const hdr_nbyte: u32 = WORD_SZ; // [head(u32) | reserved(u32)]

  const used: u32 = mem.head;
  const heap_nbyte: u32 = ALIGN(used << 2, WORD_SZ);
  const data_nbyte: u32 = ALIGN(used * data_bpe, WORD_SZ);

  return hdr_nbyte + heap_nbyte + data_nbyte;
}

export function encode<T extends TypedArray>(mem: SparseTreeMemory<T>): U64Array {
  const data_bpe: u32 = mem.data.BYTES_PER_ELEMENT;
  const total: u32 = total_nbyte(mem, data_bpe);

  const buf: U8Array = new Uint8Array(total);
  write_mem(mem, buf);

  return new BigUint64Array(buf.buffer, buf.byteOffset, total >> 3);
}

function write_mem<T extends TypedArray>(mem: SparseTreeMemory<T>, buf: U8Array): u32 {
  const buf_view = new DataView(buf.buffer, buf.byteOffset, buf.byteLength);
  const used: u32 = mem.head;

  let off: u32 = 0;

  SET_WORD(buf_view, off, used, RESERVED);
  off += WORD_SZ;

  const heap_nbyte: u32 = used << 2;
  const heap_buf: U8Array = new Uint8Array(mem.heap.buffer, mem.heap.byteOffset, heap_nbyte);
  buf.set(heap_buf, off);
  off += ALIGN(heap_nbyte, WORD_SZ);

  const data_bpe: u32 = mem.data.BYTES_PER_ELEMENT;
  const data_nbyte: u32 = used * data_bpe;
  const data_buf: U8Array = new Uint8Array(mem.data.buffer, mem.data.byteOffset, data_nbyte);
  buf.set(data_buf, off);
  off += ALIGN(data_nbyte, WORD_SZ);

  return off;
}

export function decode<T extends TypedArray>(
  buf64: U64Array,
  ctor: TypedArrayCtor<T>,
): SparseTreeMemory<T> {
  const buf: U8Array = new Uint8Array(buf64.buffer, buf64.byteOffset, buf64.byteLength);
  return read_mem(buf, ctor).mem;
}

function read_mem<T extends TypedArray>(
  buf: U8Array,
  ctor: TypedArrayCtor<T>,
): { mem: SparseTreeMemory<T>; nbyte: u32 } {
  const buf_view = new DataView(buf.buffer, buf.byteOffset, buf.byteLength);

  let off: u32 = 0;

  const [used] = GET_WORD(buf_view, off, WORD_REG);
  off += WORD_SZ;

  const heap_off: u32 = off;
  const heap_nbyte: u32 = used << 2;
  off += ALIGN(heap_nbyte, WORD_SZ);

  const data_off: u32 = off;
  const data_bpe: u32 = ctor.BYTES_PER_ELEMENT;
  const data_nbyte: u32 = used * data_bpe;
  off += ALIGN(data_nbyte, WORD_SZ);

  if (off > buf.byteLength) throw new Error("sparse_tree: truncated buffer");

  const heap: U32Array = new Uint32Array(buf.buffer, buf.byteOffset + heap_off, used);

  let data: T;
  if (((buf.byteOffset + data_off) % data_bpe) === 0) {
    data = new ctor(buf.buffer, buf.byteOffset + data_off, used);
  } else {
    data = copy_ab_view(buf.buffer, buf.byteOffset + data_off, used, ctor);
  }

  const mem: SparseTreeMemory<T> = {
    heap,
    data,
    head: used,
    cap: used,
  };

  return { mem, nbyte: off };
}
