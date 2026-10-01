// pxl-spec/space/sparse_tree/mem.rs
// sparse tree memory primitives

//! sparse tree memory primitives (stateless / data-oriented)
//! unified sparse 2^d-ary tree algorithms (top-down + bottom-up)
//!
//! architecture
//! 1) split storage: heap(u32 ptrs) + data(T) are separated but index-aligned
//! 2) pointer tagging: MSB marks branch pointer (non-recursive traversal)
//! 3) algorithms over POD arenas (STL-style)
//! 4) subtree slicing: resolve() returns a leaf slot for O(1) subtree-local access
//!
//! naming
//! - base  : first slot of one child block
//! - slot  : one concrete entry inside a block
//! - head  : allocator cursor (next free slot)
//! - block : contiguous child group of size (1 << dim)
//!
//! The structure does NOT model a node as one scalar handle.
//! The structural unit is a child block:
//! - dim=2 => one block = 4 child slots
//! - dim=3 => one block = 8 child slots
//!
//! Therefore:
//! - traversal state usually carries a block base
//! - reads/writes usually end at one leaf slot
//! - tagged heap entries point to another child block base
//!
//! sparse_tree public contract
//! 1. layout.dim is restricted to 2 | 3 only.
//! 2. layout.max must be strictly greater than layout.min.
//! 3. the structural unit is a child block, not a scalar node.
//! 4. root is the base slot of the top-level child block.
//! 5. read/write operate on leaf slots derived from PVF3 -> IVec key lowering.
//! 6. heap slot: MSB=1 means branch pointer to another child-block base.
//! 7. heap slot: MSB=0 means leaf; payload lives in data[slot].
//! 8. fill() sets the root block to one uniform leaf value.
//! 9. compact() preserves semantics and returns a tree whose root base is 0.
//! 10. encode()/decode() carry the tree as words: `[used | 0][heap][data]`,
//!     each part padded to the word.
//!
//! The leaf lane's element type is `T`, and a key's operations are the
//! `PVecTraits` implementor `K`.

use xpute_core::wire::xtp::spec::{ALIGN, GET_WORD, NONE, RESERVED, SET_WORD, WORD_SZ};

use crate::space::hierarchy::{child_of, slot_of, Scale};
use crate::space::pvf3::PVecTraits;

/// Scratchpad for 64-bit word operations to avoid allocation.
/// [lo32, hi32]
const WORD_REG: [u32; 2] = [NONE, NONE];

// ============ Types ============

/// Sparse tree backing memory.
///
/// Invariants:
/// - heap and data are slot-aligned arenas sharing the same logical index space
/// - slot 0..(1<<dim)-1 is reserved for the root child block
/// - head is the next free slot after the last allocated block
pub struct SparseTreeMemory<T: Copy + Default + PartialEq> {
    pub heap: Vec<u32>, // tagged child pointers (branch => MSB set, leaf => 0)
    pub data: Vec<T>,   // leaf payload values
    pub head: u32,      // allocator cursor (next free slot)
    pub cap: u32,       // current capacity in slots
}

/// Sparse tree layout.
///
/// Level model:
/// - min = the leaf's scale
/// - max = the root block's scale
/// - dep = how many levels max is above min
///
/// Structural interpretation:
/// - one descent step consumes one bit per active axis
/// - therefore one internal block always has (1 << dim) children
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SparseTreeLayout {
    pub dim: u32, // dimension (2 or 3)

    // The core never shifts by a scale: it counts u, the levels above min.
    pub min: Scale,
    pub max: Scale,
}

// ============ Allocator ============

/// Tagged heap entry layout:
/// - MSB = 1 => branch: remaining bits store child block base
/// - MSB = 0 => leaf  : payload is stored directly in data[slot]
const NODE_MSB: u32 = 0x80000000;
const BLOCK_MASK: u32 = 0x7fffffff;

/// Number of child slots in one block for the current dimension.
fn block_width(dim: u32) -> u32 {
    1 << dim
}

/// Validate structural layout.
///
/// Invariants:
/// - only quadtree(dim=2) and octree(dim=3) are supported
/// - dep = max - min must be >= 1
/// - therefore the root is always one top-level child block, never a scalar leaf
fn validate_layout(layout: &SparseTreeLayout) {
    let dep = layout.max.above(layout.min);
    if dep <= 0 {
        xpute_core::bug!(EBADMSG);
    }
}

/// Allocate sparse tree memory and reserve the root child block at base 0.
pub fn alloc<T: Copy + Default + PartialEq>(dim: u32, cap: u32) -> SparseTreeMemory<T> {
    let root_width = block_width(dim);
    let cap = if cap >= root_width { cap } else { root_width };

    SparseTreeMemory {
        heap: vec![0u32; cap as usize],
        data: vec![T::default(); cap as usize],
        head: root_width, // reserve base 0..root_width-1 for root block
        cap,
    }
}

/// The same tree with its data lane in another type — the heap is shared,
/// every value is carried across. For widening a leaf lane when its values
/// outgrow it.
pub fn retype<T: Copy + Default + PartialEq, U: Copy + Default + PartialEq + From<T>>(mem: SparseTreeMemory<T>) -> SparseTreeMemory<U> {
    let mut data = vec![U::default(); mem.cap as usize];
    for (to, from) in data.iter_mut().zip(mem.data.iter().take(mem.head as usize)) {
        *to = U::from(*from);
    }
    SparseTreeMemory {
        heap: mem.heap,
        data,
        head: mem.head,
        cap: mem.cap,
    }
}

/// Grow backing buffers so that slot range [0, end) fits.
fn grow<T: Copy + Default + PartialEq>(mem: &mut SparseTreeMemory<T>, end: u32) {
    let mut cap = mem.cap;
    while cap < end {
        cap <<= 1;
    }

    let mut heap = vec![0u32; cap as usize];
    let mut data = vec![T::default(); cap as usize];

    heap[..mem.heap.len()].copy_from_slice(&mem.heap);
    data[..mem.data.len()].copy_from_slice(&mem.data);

    mem.heap = heap;
    mem.data = data;
    mem.cap = cap;
}

/// Allocate an arbitrary contiguous slot span and return its first slot.
fn alloc_slots<T: Copy + Default + PartialEq>(mem: &mut SparseTreeMemory<T>, cnt: u32) -> u32 {
    let base = mem.head;
    let end = base + cnt;

    if end > mem.cap {
        grow(mem, end);
    }

    mem.head = end;
    mem.heap.as_mut_slice()[base as usize..end as usize].fill(0);
    mem.data.as_mut_slice()[base as usize..end as usize].fill(T::default());
    base
}

/// Allocate exactly one child block and return its base slot.
///
/// Postconditions:
/// - returned range [base, base+block_width(dim)) is reserved
/// - heap/data in that block are zero-initialized
fn alloc_block<T: Copy + Default + PartialEq>(mem: &mut SparseTreeMemory<T>, dim: u32) -> u32 {
    alloc_slots(mem, block_width(dim))
}

/// Reset to the root-only state.
pub fn reset<T: Copy + Default + PartialEq>(mem: &mut SparseTreeMemory<T>, layout: &SparseTreeLayout) {
    validate_layout(layout);

    let root_width = block_width(layout.dim);
    mem.head = root_width;
    mem.heap.as_mut_slice()[..root_width as usize].fill(0);
    mem.data.as_mut_slice()[..root_width as usize].fill(T::default());
}

/// Reset the tree and fill the root block with one uniform value.
pub fn fill<T: Copy + Default + PartialEq>(mem: &mut SparseTreeMemory<T>, layout: &SparseTreeLayout, val: T) {
    reset(mem, layout);
    mem.data.as_mut_slice()[..mem.head as usize].fill(val);
}

// ============ Traversal ============

/// The child slot `key` selects at level `u`: its lanes `u` levels above the
/// leaf, in the hierarchy's slot order over the active axes.
fn slot_at<K: PVecTraits>(layout: &SparseTreeLayout, key: K::V, u: i32) -> u32
where
    K::V: Copy,
{
    let shifted = K::sh(key, -u);
    let (x, y, z) = (K::X(shifted), K::Y(shifted), K::Z(shifted));
    (if layout.dim == 3 { slot_of([x, y, z]) } else { slot_of([x, y]) }) as u32
}

/// Resolve the leaf slot corresponding to `key` at `target`.
///
/// Semantics:
/// - traversal state carries a current block base
/// - each step selects one child slot inside that block
/// - if traversal hits a uniform leaf before target depth, the current slot is returned
///
/// Return value:
/// - always one concrete slot, never a block base for the next level unless target landed there
pub fn resolve<K: PVecTraits, T: Copy + Default + PartialEq>(mem: &SparseTreeMemory<T>, layout: &SparseTreeLayout, key: K::V, target: Scale, root: u32) -> u32
where
    K::V: Copy,
{
    validate_layout(layout);

    let dep = layout.max.above(layout.min);
    let target_u = target.above(layout.min);

    if target_u < 0 {
        return root;
    }
    if target_u >= dep {
        return root;
    }

    let mut base = root;

    let mut u = dep - 1;
    while u >= target_u {
        let child = slot_at::<K>(layout, key, u);

        let slot = base + child;
        let node = mem.heap[slot as usize];

        if u == target_u {
            return slot;
        }
        if node & NODE_MSB == 0 {
            return slot;
        }

        base = node & BLOCK_MASK;
        u -= 1;
    }

    base
}

/// Read one leaf value from the tree.
pub fn get<K: PVecTraits, T: Copy + Default + PartialEq>(mem: &SparseTreeMemory<T>, layout: &SparseTreeLayout, key: K::V, root: u32) -> T
where
    K::V: Copy,
{
    validate_layout(layout);

    let slot = resolve::<K, T>(mem, layout, key, layout.min, root);
    let node = mem.heap[slot as usize];

    if node & NODE_MSB != 0 {
        xpute_core::bug!(EBADMSG);
    }
    mem.data[slot as usize]
}

/// Write one value, splitting only when needed.
///
/// `target` (the finest, `layout.min`, for a leaf — the same
/// parameter shape as `resolve()`) lets a write claim a whole coarser
/// block in one call instead of always bottoming out at the finest leaf:
/// reaching `target_u` unconditionally claims that slot as one uniform
/// leaf at `val`, discarding whatever finer structure lived beneath it (its
/// child block, if any, is simply orphaned — never visited again once
/// `heap[slot]` stops pointing at it, and reclaimed the next time the
/// caller compacts).
///
/// Traversal invariant:
/// - `base` is always the current child-block base
/// - `slot` is the selected child entry inside that block
pub fn set<K: PVecTraits, T: Copy + Default + PartialEq>(mem: &mut SparseTreeMemory<T>, layout: &SparseTreeLayout, key: K::V, val: T, target: Scale, root: u32)
where
    K::V: Copy,
{
    write::<K, T>(mem, layout, key, val, target, root, &mut [(0, 0); MAX_DEP]);
}

// A branch slot's `data` is the tree's to leave alone, and a caller may keep
// in it what the branch's child block folds to — a majority, a maximum, a
// count — so a node of any depth answers for all under it in one read. What
// folds is the caller's; the tree keeps it current through `set_folded`,
// carries it through `compact`, and makes it again whole through `refold`.

/// The most levels a tree descends: what a write's path is held in.
const MAX_DEP: usize = 32;

/// `set`, keeping every branch slot's `data` what `fold` makes of its child
/// block: the branches the write went through or made are folded again from
/// the deepest up, `fold(children, scale)` with `scale` the branch's own.
/// The branches it did not pass under are untouched, and none it passed is
/// seen between the write and its fold.
pub fn set_folded<K: PVecTraits, T: Copy + Default + PartialEq>(mem: &mut SparseTreeMemory<T>, layout: &SparseTreeLayout, key: K::V, val: T, target: Scale, root: u32, fold: &dyn Fn(&[T], Scale) -> T)
where
    K::V: Copy,
{
    let mut path = [(0, 0); MAX_DEP];
    let n = write::<K, T>(mem, layout, key, val, target, root, &mut path);
    let width = block_width(layout.dim) as usize;
    for &(slot, u) in path[..n].iter().rev() {
        let child = (mem.heap[slot as usize] & BLOCK_MASK) as usize;
        let folded = fold(&mem.data[child..child + width], layout.min.up(u));
        mem.data[slot as usize] = folded;
    }
}

/// Every branch slot's `data` made again from its child block, the deepest
/// first, by `fold` as `set_folded` keeps it: for a tree whose branches do
/// not hold it yet.
pub fn refold<T: Copy + Default + PartialEq>(mem: &mut SparseTreeMemory<T>, layout: &SparseTreeLayout, root: u32, fold: &dyn Fn(&[T], Scale) -> T) {
    fn go<T: Copy + Default + PartialEq>(mem: &mut SparseTreeMemory<T>, width: u32, min: Scale, base: u32, u: i32, fold: &dyn Fn(&[T], Scale) -> T) {
        for c in 0..width {
            let slot = (base + c) as usize;
            let node = mem.heap[slot];
            if node & NODE_MSB != 0 {
                let child = node & BLOCK_MASK;
                go(mem, width, min, child, u - 1, fold);
                let folded = fold(&mem.data[child as usize..(child + width) as usize], min.up(u));
                mem.data[slot] = folded;
            }
        }
    }
    validate_layout(layout);
    let dep = layout.max.above(layout.min);
    go(mem, block_width(layout.dim), layout.min, root, dep - 1, fold);
}

/// `set`'s write, recording in `path` every branch slot it went through or
/// made, with the level it is at, shallowest first; how many, none where
/// the write changed nothing.
fn write<K: PVecTraits, T: Copy + Default + PartialEq>(mem: &mut SparseTreeMemory<T>, layout: &SparseTreeLayout, key: K::V, val: T, target: Scale, root: u32, path: &mut [(u32, i32); MAX_DEP]) -> usize
where
    K::V: Copy,
{
    validate_layout(layout);

    let mut base = root;
    let width = block_width(layout.dim);

    let dep = layout.max.above(layout.min);
    if dep <= 0 {
        return 0;
    }
    xpute_core::ensure!(dep as usize <= MAX_DEP, ENOTRECOVERABLE, dep);

    let target_u = target.above(layout.min);

    let mut n = 0;
    let mut u = dep - 1;
    while u >= target_u {
        let child = slot_at::<K>(layout, key, u);

        let slot = (base + child) as usize;

        if u == target_u {
            if mem.heap[slot] & NODE_MSB == 0 && mem.data[slot] == val {
                return 0;
            }
            mem.heap[slot] = 0;
            mem.data[slot] = val;
            return n;
        }

        let node = mem.heap[slot];

        if node & NODE_MSB != 0 {
            path[n] = (slot as u32, u);
            n += 1;
            base = node & BLOCK_MASK;
            u -= 1;
            continue;
        }

        if mem.data[slot] == val {
            return 0;
        }

        let next_base = alloc_block(mem, layout.dim);
        let was = mem.data[slot];
        mem.data.as_mut_slice()[next_base as usize..(next_base + width) as usize].fill(was);
        mem.heap[slot] = next_base | NODE_MSB;
        path[n] = (slot as u32, u);
        n += 1;
        base = next_base;
        u -= 1;
    }
    n
}

// ============ Build ============

/// Bottom-up build from dense leaves.
///
/// Root invariant:
/// - the sparse-tree root is one top-level child block, not one scalar node
/// - therefore the final returned root must be a block base whose block width is (1 << dim)
pub fn build<T: Copy + Default + PartialEq>(mem: &mut SparseTreeMemory<T>, layout: &SparseTreeLayout, leaves: &[T]) -> u32 {
    validate_layout(layout);

    let width = block_width(layout.dim);
    let dep = layout.max.above(layout.min);

    if leaves.is_empty() {
        xpute_core::bug!(EBADMSG);
    }

    let mut expect: u32 = 1;
    for _ in 0..dep {
        expect *= width;
    }
    if leaves.len() != expect as usize {
        xpute_core::bug!(EBADMSG, expect, leaves.len());
    }

    let mut cur_base = alloc_slots(mem, leaves.len() as u32);
    mem.data.as_mut_slice()[cur_base as usize..cur_base as usize + leaves.len()].copy_from_slice(leaves);
    let mut cur_cnt = leaves.len() as u32;

    // Stop when one top-level child block remains.
    while cur_cnt > width {
        let parent_cnt = cur_cnt / width;
        let parent_base = alloc_slots(mem, parent_cnt);

        for i in 0..parent_cnt {
            let parent_slot = (parent_base + i) as usize;
            let child_base = (cur_base + i * width) as usize;

            let mut uniform = true;
            let first = mem.data[child_base];

            if mem.heap[child_base] & NODE_MSB != 0 {
                uniform = false;
            } else {
                for k in 1..width as usize {
                    if (mem.heap[child_base + k] & NODE_MSB) != 0 || mem.data[child_base + k] != first {
                        uniform = false;
                        break;
                    }
                }
            }

            if uniform {
                mem.data[parent_slot] = first;
                mem.heap[parent_slot] = 0;
            } else {
                mem.heap[parent_slot] = child_base as u32 | NODE_MSB;
            }
        }

        cur_base = parent_base;
        cur_cnt = parent_cnt;
    }

    cur_base
}

/// Append one raw child block.
pub fn append<T: Copy + Default + PartialEq>(mem: &mut SparseTreeMemory<T>, layout: &SparseTreeLayout, vals: &[T]) -> u32 {
    validate_layout(layout);

    let width = block_width(layout.dim);
    if vals.len() != width as usize {
        xpute_core::bug!(EBADMSG, width, vals.len());
    }

    let base = alloc_block(mem, layout.dim);
    mem.data.as_mut_slice()[base as usize..base as usize + vals.len()].copy_from_slice(vals);
    base
}

// ============ Dense ============

/// Reconstruct dense leaf values from a sparse subtree.
/// dim=2 is row-major with x contiguous.
/// dim=3 is slab-major [z][y][x] with x contiguous inside each row.
pub fn dense<K: PVecTraits, T: Copy + Default + PartialEq>(mem: &SparseTreeMemory<T>, layout: &SparseTreeLayout, root: u32) -> Box<[T]>
where
    K::V: Copy,
{
    validate_layout(layout);

    let dep = layout.max.above(layout.min);

    let side: u32 = 1 << dep;
    let plane = side * side;
    let total = if layout.dim == 2 { plane } else { plane * side };

    let mut out = vec![T::default(); total as usize];

    if layout.dim == 2 {
        let mut i = 0;
        for y in 0..side {
            for x in 0..side {
                let key = K::gather(x as i64, y as i64, 0);
                out[i] = get::<K, T>(mem, layout, key, root);
                i += 1;
            }
        }
        return out.into_boxed_slice();
    }

    let mut i = 0;
    for z in 0..side {
        for y in 0..side {
            for x in 0..side {
                let key = K::gather(x as i64, y as i64, z as i64);
                out[i] = get::<K, T>(mem, layout, key, root);
                i += 1;
            }
        }
    }
    out.into_boxed_slice()
}

// ============ Traversal (Enumerate) ============

/// Visit every maximal uniform leaf block in the tree — the inverse of
/// `dense()`: instead of expanding to one value per finest-level cell, this
/// calls `visit` once per *block*, however coarse, with that block's own
/// corner (in steps of `layout.min` — multiply by `layout.min.side()` for a
/// real-unit offset) and scale (its `side()` is the block's real-unit
/// size). Skips branch slots entirely (never visits an internal node, only
/// leaves), so a caller building a sparse "list of occupied regions" view
/// (e.g. one render/placement record per uniform block) never has to
/// re-detect uniformity itself the way `dense()`-then-rescan would.
pub fn walk<T: Copy + Default + PartialEq>(mem: &SparseTreeMemory<T>, layout: &SparseTreeLayout, visit: &mut dyn FnMut(i32, i32, i32, Scale, T), root: u32) {
    validate_layout(layout);
    let dep = layout.max.above(layout.min);
    let width = block_width(layout.dim);

    // The root itself can never be a scalar leaf (invariant #4 — it's always
    // pre-allocated as one block of `width` children), so a tree made
    // uniform via fill()/reset() stores that as `width` separate identical
    // leaf slots, not one leaf. Check for that case explicitly: it's still
    // the single most "maximal uniform block" this tree can ever report
    // (bigger than any of its own children), just one compact() itself never
    // rewrites (compact()'s own contract only ever preserves the root-is-a-
    // block invariant, see its doc comment) — walk() is read-only, so it can
    // report the virtual result without needing to touch tree storage.
    let mut uniform = true;
    let first = mem.data[root as usize];
    for c in 0..width as usize {
        if !uniform {
            break;
        }
        if (mem.heap[root as usize + c] & NODE_MSB) != 0 || mem.data[root as usize + c] != first {
            uniform = false;
        }
    }
    if uniform {
        visit(0, 0, 0, layout.max, first);
        return;
    }

    walk_of(mem, layout.dim, layout.min, root, dep - 1, 0, 0, 0, visit);
}

#[allow(clippy::too_many_arguments)]
fn walk_of<T: Copy + Default + PartialEq>(mem: &SparseTreeMemory<T>, dim: u32, min: Scale, base: u32, u: i32, x: i32, y: i32, z: i32, visit: &mut dyn FnMut(i32, i32, i32, Scale, T)) {
    let width = block_width(dim);

    for c in 0..width {
        let slot = (base + c) as usize;
        let node = mem.heap[slot];

        // The child's own bit on each axis: its corner a level down from the
        // block's, which at the finest scale is that bit shifted by u.
        let [bit_x, bit_y, bit_z] = child_of([0; 3], c as usize).map(|b| b as i32);
        let cx = x | (bit_x << u);
        let cy = y | (bit_y << u);
        let cz = z | (bit_z << u);

        if node & NODE_MSB != 0 {
            walk_of(mem, dim, min, node & BLOCK_MASK, u - 1, cx, cy, cz, visit);
        } else {
            visit(cx, cy, cz, min.up(u), mem.data[slot]);
        }
    }
}

/// `walk` stopped at `target`: every slot at `target`'s depth — a branch
/// there reported with its `data`, what its child block folds to where the
/// caller keeps that (`set_folded`) — and every leaf coarser than it, at its
/// own scale, with its corner in steps of `layout.min`.
pub fn walk_to<T: Copy + Default + PartialEq>(mem: &SparseTreeMemory<T>, layout: &SparseTreeLayout, target: Scale, visit: &mut dyn FnMut(i32, i32, i32, Scale, T), root: u32) {
    #[allow(clippy::too_many_arguments)]
    fn go<T: Copy + Default + PartialEq>(mem: &SparseTreeMemory<T>, dim: u32, min: Scale, base: u32, u: i32, target_u: i32, [x, y, z]: [i32; 3], visit: &mut dyn FnMut(i32, i32, i32, Scale, T)) {
        for c in 0..block_width(dim) {
            let slot = (base + c) as usize;
            let node = mem.heap[slot];
            let [bx, by, bz] = child_of([0; 3], c as usize).map(|b| b as i32);
            let at = [x | (bx << u), y | (by << u), z | (bz << u)];
            if node & NODE_MSB != 0 && u > target_u {
                go(mem, dim, min, node & BLOCK_MASK, u - 1, target_u, at, visit);
            } else {
                visit(at[0], at[1], at[2], min.up(u), mem.data[slot]);
            }
        }
    }
    validate_layout(layout);
    let dep = layout.max.above(layout.min);
    go(mem, layout.dim, layout.min, root, dep - 1, target.above(layout.min).max(0), [0; 3], visit);
}

// ============ Maintenance (Pack) ============

/// What `compact` hands back: the memory, and its root base (0).
pub struct Compacted<T: Copy + Default + PartialEq> {
    pub mem: SparseTreeMemory<T>,
    pub root: u32,
}

/// Compact the tree by pruning redundant nodes and relocating live blocks.
pub fn compact<T: Copy + Default + PartialEq>(mem: &mut SparseTreeMemory<T>, layout: &SparseTreeLayout, root: u32) -> Compacted<T> {
    validate_layout(layout);

    let width = block_width(layout.dim);

    prune(mem, root, layout.max.above(layout.min), width);

    let mut new_mem = alloc::<T>(layout.dim, mem.head);

    fn relocate<T: Copy + Default + PartialEq>(mem: &SparseTreeMemory<T>, new_mem: &mut SparseTreeMemory<T>, dim: u32, width: u32, old_base: u32, new_base: u32) {
        for i in 0..width {
            let old_slot = (old_base + i) as usize;
            let new_slot = (new_base + i) as usize;
            let node = mem.heap[old_slot];

            // A branch's `data` goes with it too: what it folds to, where
            // its caller keeps that (`set_folded`).
            new_mem.data[new_slot] = mem.data[old_slot];
            if node & NODE_MSB != 0 {
                let child_base = node & BLOCK_MASK;
                let next_base = alloc_block(new_mem, dim);

                new_mem.heap[new_slot] = next_base | NODE_MSB;
                relocate(mem, new_mem, dim, width, child_base, next_base);
            }
        }
    }

    relocate(mem, &mut new_mem, layout.dim, width, root, 0);

    let mut compacted = alloc::<T>(layout.dim, new_mem.head);

    let used = new_mem.head as usize;
    compacted.heap[..used].copy_from_slice(&new_mem.heap[..used]);
    compacted.data[..used].copy_from_slice(&new_mem.data[..used]);
    compacted.head = new_mem.head;

    Compacted { mem: compacted, root: 0 }
}

/// Internal recursive prune.
///
/// Return:
/// - uniform leaf value if the whole block can collapse into one value
/// - `None` otherwise
fn prune<T: Copy + Default + PartialEq>(mem: &mut SparseTreeMemory<T>, base: u32, dep: i32, width: u32) -> Option<T> {
    if dep == 0 {
        return None;
    }

    let mut uniform = true;
    let mut first: Option<T> = None;

    for i in 0..width {
        let slot = (base + i) as usize;
        let node = mem.heap[slot];
        let mut val: Option<T> = if node & NODE_MSB != 0 { None } else { Some(mem.data[slot]) };

        if node & NODE_MSB != 0 {
            let child_base = node & BLOCK_MASK;
            let child_val = prune(mem, child_base, dep - 1, width);

            if let Some(child_val) = child_val {
                mem.heap[slot] = 0;
                mem.data[slot] = child_val;
                val = Some(child_val);
            } else {
                uniform = false;
            }
        }

        if first.is_none() {
            first = val;
        } else if val.is_none() || first != val {
            uniform = false;
        }
    }

    if uniform {
        first
    } else {
        None
    }
}

// ============ Memory I/O ============

fn total_nbyte<T: Copy + Default + PartialEq>(mem: &SparseTreeMemory<T>, data_bpe: u32) -> u32 {
    let hdr_nbyte = WORD_SZ; // [head(u32) | reserved(u32)]

    let used = mem.head;
    let heap_nbyte = ALIGN(used << 2, WORD_SZ).unwrap();
    let data_nbyte = ALIGN(used * data_bpe, WORD_SZ).unwrap();

    hdr_nbyte + heap_nbyte + data_nbyte
}

pub fn encode<T: Copy + Default + PartialEq>(mem: &SparseTreeMemory<T>) -> Vec<u64> {
    let data_bpe = core::mem::size_of::<T>() as u32;
    let total = total_nbyte(mem, data_bpe);

    // The blob is words on the wire, so it is allocated as words and written
    // as the bytes they are.
    let mut words = vec![0u64; (total >> 3) as usize];
    // SAFETY: the words' own bytes, which `total` is the length of.
    let buf = unsafe { core::slice::from_raw_parts_mut(words.as_mut_ptr() as *mut u8, total as usize) };
    write_mem(mem, buf);
    words
}

fn write_mem<T: Copy + Default + PartialEq>(mem: &SparseTreeMemory<T>, buf: &mut [u8]) -> u32 {
    let used = mem.head;

    let mut off: u32 = 0;

    SET_WORD(buf, off, used, RESERVED);
    off += WORD_SZ;

    let heap_nbyte = used << 2;
    buf[off as usize..(off + heap_nbyte) as usize].copy_from_slice(bytes_of(&mem.heap[..used as usize]));
    off += ALIGN(heap_nbyte, WORD_SZ).unwrap();

    let data_bpe = core::mem::size_of::<T>() as u32;
    let data_nbyte = used * data_bpe;
    buf[off as usize..(off + data_nbyte) as usize].copy_from_slice(bytes_of(&mem.data[..used as usize]));
    off += ALIGN(data_nbyte, WORD_SZ).unwrap();

    off
}

/// `ALIGN(nbyte, WORD_SZ)` for a size that may not fit u32, refusing as
/// ALIGN does.
fn aligned_word(nbyte: u64) -> u64 {
    match u32::try_from(nbyte) {
        Ok(n) => ALIGN(n, WORD_SZ).unwrap_or_else(|_| xpute_core::bug!(EOVERFLOW, n)) as u64,
        Err(_) => xpute_core::bug!(EOVERFLOW, nbyte),
    }
}

pub fn decode<T: Copy + Default + PartialEq>(buf64: &[u64]) -> SparseTreeMemory<T> {
    // SAFETY: the blob's own bytes, which are what it was written as.
    let buf = unsafe { core::slice::from_raw_parts(buf64.as_ptr() as *const u8, buf64.len() * 8) };
    read_mem::<T>(buf).0
}

/// A lane's elements as the bytes they lie in.
fn bytes_of_mut<T: Copy>(a: &mut [T]) -> &mut [u8] {
    // SAFETY: as `bytes_of`.
    unsafe { core::slice::from_raw_parts_mut(a.as_mut_ptr() as *mut u8, core::mem::size_of_val(a)) }
}

/// A lane's elements as the bytes they lie in.
fn bytes_of<T: Copy>(a: &[T]) -> &[u8] {
    // SAFETY: the elements read back as bytes, which is what the blob holds.
    unsafe { core::slice::from_raw_parts(a.as_ptr() as *const u8, core::mem::size_of_val(a)) }
}

fn read_mem<T: Copy + Default + PartialEq>(buf: &[u8]) -> (SparseTreeMemory<T>, u32) {
    // The sizes are the blob's word times an element, and the blob may be
    // anything: they are summed in u64 so a large count cannot wrap back
    // under the buffer's length.
    let mut reg = WORD_REG;
    let [used, _] = GET_WORD(buf, 0, &mut reg);
    let mut off = WORD_SZ as u64;

    let heap_off = off as u32;
    off += aligned_word(used as u64 * 4);

    let data_off = off;
    let data_bpe = core::mem::size_of::<T>() as u32;
    off += aligned_word(used as u64 * data_bpe as u64);

    if off > buf.len() as u64 {
        xpute_core::bug!(EBADMSG);
    }
    let (data_off, off) = (data_off as u32, off as u32);

    // Copied rather than read where they lie: the memory grows, and a grow
    // reallocates what a view would have pointed into.
    let mut heap = vec![0u32; used as usize];
    let mut data = vec![T::default(); used as usize];
    bytes_of_mut(&mut heap).copy_from_slice(&buf[heap_off as usize..heap_off as usize + used as usize * 4]);
    bytes_of_mut(&mut data).copy_from_slice(&buf[data_off as usize..data_off as usize + used as usize * data_bpe as usize]);

    let mem = SparseTreeMemory { heap, data, head: used, cap: used };

    (mem, off)
}
