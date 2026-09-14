// pxl_shared/spec/space/sparse_tree/mem.rs
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
//! 10. encode()/decode() use U64Array as the sparse-tree wire/storage blob surface.
//!
//! The leaf lane's constructor is the element type `T` here, and a key's
//! traits the `PVecTraits` implementor `K`.

use xpute_core::ds::typed_alloc::{TypedAlloc, HEAP_ALLOC};
use xpute_core::io::marshal::tree::spec::{ALIGN, GET_WORD, NONE, RESERVED, SET_WORD, WORD_SZ};
use xpute_core::kit::array::{copy_ab_view, AnyElement, ArrayBuffer, NumberElement, TypedArray, U32Array, U64Array, U8Array};

use crate::spec::space::pvf3::PVecTraits;

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
pub struct SparseTreeMemory<T: NumberElement> {
    pub heap: U32Array,      // tagged child pointers (branch => MSB set, leaf => 0)
    pub data: TypedArray<T>, // leaf payload values
    pub head: u32,           // allocator cursor (next free slot)
    pub cap: u32,            // current capacity in slots
    /// Where heap and data come from, and go back to (typed_alloc.rs). A
    /// decoded tree's lanes are views over its blob and come from nowhere.
    pub alloc: Box<dyn TypedAlloc>,
}

/// Sparse tree layout.
///
/// Level model:
/// - min = leaf level
/// - max = root level
/// - dep = max - min
///
/// Structural interpretation:
/// - one descent step consumes one bit per active axis
/// - therefore one internal block always has (1 << dim) children
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SparseTreeLayout {
    pub dim: u32, // dimension (2 or 3)

    // Signed log2 scale used by the layout contract.
    // - min can be negative (e.g. -2 => 1m if 0 means "1 lattice unit")
    // - core never shifts by signed values; it normalizes to u = level - min (u>=0).
    pub min: i32, // leaf level (signed)
    pub max: i32, // root level (signed). dep = max - min
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
    let dep = layout.max - layout.min;
    if dep <= 0 {
        panic!("sparse_tree: bad layout: expected max > min, got min={}, max={}", layout.min, layout.max);
    }
}

fn take<E: AnyElement>(alloc: &mut dyn TypedAlloc, n: u32) -> TypedArray<E> {
    TypedArray::of(&alloc.take_view(E::KIND, n)).expect("take_view gave a view of another element")
}

/// Allocate sparse tree memory and reserve the root child block at base 0.
pub fn alloc<T: NumberElement>(dim: u32, cap: u32, typed: Option<Box<dyn TypedAlloc>>) -> SparseTreeMemory<T> {
    let mut typed = typed.unwrap_or_else(|| Box::new(HEAP_ALLOC));
    let root_width = block_width(dim);
    let cap = if cap >= root_width { cap } else { root_width };

    let heap = take::<u32>(typed.as_mut(), cap);
    let data = take::<T>(typed.as_mut(), cap);
    SparseTreeMemory {
        heap,
        data,
        head: root_width, // reserve base 0..root_width-1 for root block
        cap,
        alloc: typed,
    }
}

/// Hands a memory's lanes back to where they came from.
pub fn free<T: NumberElement>(mem: &mut SparseTreeMemory<T>) {
    mem.alloc.give(&mem.heap.view());
    mem.alloc.give(&mem.data.view());
}

/// The same tree with its data lane in another type — the heap is shared,
/// every value is carried across. For widening a leaf lane when its values
/// outgrow it.
pub fn retype<T: NumberElement, U: NumberElement>(mut mem: SparseTreeMemory<T>) -> SparseTreeMemory<U> {
    let data = take::<U>(mem.alloc.as_mut(), mem.cap);
    for i in 0..mem.head as usize {
        data.put(i, U::from_f64(mem.data[i].to_f64()));
    }
    mem.alloc.give(&mem.data.view());
    SparseTreeMemory {
        heap: mem.heap,
        data,
        head: mem.head,
        cap: mem.cap,
        alloc: mem.alloc,
    }
}

/// A copy of `mem` whose lanes come from `typed` — for a decoded tree,
/// whose lanes are views over the blob it was read from.
pub fn adopt<T: NumberElement>(mem: &SparseTreeMemory<T>, mut typed: Box<dyn TypedAlloc>) -> SparseTreeMemory<T> {
    let heap = take::<u32>(typed.as_mut(), mem.head);
    let data = take::<T>(typed.as_mut(), mem.head);
    heap.set(&mem.heap.subarray(0, mem.head as usize));
    data.set(&mem.data.subarray(0, mem.head as usize));
    SparseTreeMemory {
        heap,
        data,
        head: mem.head,
        cap: mem.head,
        alloc: typed,
    }
}

/// Grow backing buffers so that slot range [0, end) fits.
fn grow<T: NumberElement>(mem: &mut SparseTreeMemory<T>, end: u32) {
    let mut cap = mem.cap;
    while cap < end {
        cap <<= 1;
    }

    let heap = take::<u32>(mem.alloc.as_mut(), cap);
    let data = take::<T>(mem.alloc.as_mut(), cap);

    heap.set(&mem.heap);
    data.set(&mem.data);

    free(mem);
    mem.heap = heap;
    mem.data = data;
    mem.cap = cap;
}

/// Allocate an arbitrary contiguous slot span and return its first slot.
fn alloc_slots<T: NumberElement>(mem: &mut SparseTreeMemory<T>, cnt: u32) -> u32 {
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
fn alloc_block<T: NumberElement>(mem: &mut SparseTreeMemory<T>, dim: u32) -> u32 {
    alloc_slots(mem, block_width(dim))
}

/// Reset to the root-only state.
pub fn reset<T: NumberElement>(mem: &mut SparseTreeMemory<T>, layout: &SparseTreeLayout) {
    validate_layout(layout);

    let root_width = block_width(layout.dim);
    mem.head = root_width;
    mem.heap.as_mut_slice()[..root_width as usize].fill(0);
    mem.data.as_mut_slice()[..root_width as usize].fill(T::default());
}

/// Reset the tree and fill the root block with one uniform value.
pub fn fill<T: NumberElement>(mem: &mut SparseTreeMemory<T>, layout: &SparseTreeLayout, val: T) {
    reset(mem, layout);
    mem.data.as_mut_slice()[..mem.head as usize].fill(val);
}

// ============ Traversal ============

/// The child slot `key` selects at level `u`: one bit per active axis.
fn child_of<K: PVecTraits>(layout: &SparseTreeLayout, key: K::V, u: i32) -> u32
where
    K::V: Copy,
{
    let shifted = K::sh(key, -u);
    let mut child = (K::X(shifted) & 1) as u32;
    if layout.dim >= 2 {
        child |= ((K::Y(shifted) & 1) as u32) << 1;
    }
    if layout.dim >= 3 {
        child |= ((K::Z(shifted) & 1) as u32) << 2;
    }
    child
}

/// Resolve the leaf slot corresponding to `key` at `target_shift`.
///
/// Semantics:
/// - traversal state carries a current block base
/// - each step selects one child slot inside that block
/// - if traversal hits a uniform leaf before target depth, the current slot is returned
///
/// Return value:
/// - always one concrete slot, never a block base for the next level unless target landed there
pub fn resolve<K: PVecTraits, T: NumberElement>(mem: &SparseTreeMemory<T>, layout: &SparseTreeLayout, key: K::V, target_shift: i32, root: u32) -> u32
where
    K::V: Copy,
{
    validate_layout(layout);

    let dep = layout.max - layout.min;
    let target_u = target_shift - layout.min;

    if target_u < 0 {
        return root;
    }
    if target_u >= dep {
        return root;
    }

    let mut base = root;

    let mut u = dep - 1;
    while u >= target_u {
        let child = child_of::<K>(layout, key, u);

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
pub fn get<K: PVecTraits, T: NumberElement>(mem: &SparseTreeMemory<T>, layout: &SparseTreeLayout, key: K::V, root: u32) -> T
where
    K::V: Copy,
{
    validate_layout(layout);

    let slot = resolve::<K, T>(mem, layout, key, layout.min, root);
    let node = mem.heap[slot as usize];

    if node & NODE_MSB != 0 {
        panic!("sparse_tree: expected leaf slot, got branch");
    }
    mem.data[slot as usize]
}

/// Write one value, splitting only when needed.
///
/// `target_shift` (the finest, `layout.min`, for a leaf — the same
/// parameter shape as `resolve()`) lets a write claim a whole coarser
/// block in one call instead of always bottoming out at the finest leaf:
/// reaching `target_u` unconditionally claims that slot as one uniform
/// leaf at `val`, discarding whatever finer structure lived beneath it (its
/// child block, if any, is simply orphaned — never visited again once
/// `heap[slot]` no longer points at it, and reclaimed the next time the
/// caller compacts).
///
/// Traversal invariant:
/// - `base` is always the current child-block base
/// - `slot` is the selected child entry inside that block
pub fn set<K: PVecTraits, T: NumberElement>(mem: &mut SparseTreeMemory<T>, layout: &SparseTreeLayout, key: K::V, val: T, target_shift: i32, root: u32)
where
    K::V: Copy,
{
    validate_layout(layout);

    let mut base = root;
    let width = block_width(layout.dim);

    let dep = layout.max - layout.min;
    if dep <= 0 {
        return;
    }

    let target_u = target_shift - layout.min;

    let mut u = dep - 1;
    while u >= target_u {
        let child = child_of::<K>(layout, key, u);

        let slot = (base + child) as usize;

        if u == target_u {
            if mem.heap[slot] & NODE_MSB == 0 && mem.data[slot] == val {
                return;
            }
            mem.heap.put(slot, 0);
            mem.data.put(slot, val);
            return;
        }

        let node = mem.heap[slot];

        if node & NODE_MSB != 0 {
            base = node & BLOCK_MASK;
            u -= 1;
            continue;
        }

        if mem.data[slot] == val {
            return;
        }

        let next_base = alloc_block(mem, layout.dim);
        let was = mem.data[slot];
        mem.data.as_mut_slice()[next_base as usize..(next_base + width) as usize].fill(was);
        mem.heap.put(slot, next_base | NODE_MSB);
        base = next_base;
        u -= 1;
    }
}

// ============ Build ============

/// Bottom-up build from dense leaves.
///
/// Root invariant:
/// - the sparse-tree root is one top-level child block, not one scalar node
/// - therefore the final returned root must be a block base whose block width is (1 << dim)
pub fn build<T: NumberElement>(mem: &mut SparseTreeMemory<T>, layout: &SparseTreeLayout, leaves: &[T]) -> u32 {
    validate_layout(layout);

    let width = block_width(layout.dim);
    let dep = layout.max - layout.min;

    if leaves.is_empty() {
        panic!("sparse_tree: empty leaves");
    }

    let mut expect: u32 = 1;
    for _ in 0..dep {
        expect *= width;
    }
    if leaves.len() != expect as usize {
        panic!("sparse_tree: bad leaf count: expected {expect}, got {}", leaves.len());
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
                mem.data.put(parent_slot, first);
                mem.heap.put(parent_slot, 0);
            } else {
                mem.heap.put(parent_slot, child_base as u32 | NODE_MSB);
            }
        }

        cur_base = parent_base;
        cur_cnt = parent_cnt;
    }

    cur_base
}

/// Append one raw child block.
pub fn append<T: NumberElement>(mem: &mut SparseTreeMemory<T>, layout: &SparseTreeLayout, vals: &[T]) -> u32 {
    validate_layout(layout);

    let width = block_width(layout.dim);
    if vals.len() != width as usize {
        panic!("sparse_tree: block size mismatch: expected {width}, got {}", vals.len());
    }

    let base = alloc_block(mem, layout.dim);
    mem.data.as_mut_slice()[base as usize..base as usize + vals.len()].copy_from_slice(vals);
    base
}

// ============ Dense ============

/// Reconstruct dense leaf values from a sparse subtree.
/// dim=2 is row-major with x contiguous.
/// dim=3 is slab-major [z][y][x] with x contiguous inside each row.
pub fn dense<K: PVecTraits, T: NumberElement>(mem: &SparseTreeMemory<T>, layout: &SparseTreeLayout, root: u32) -> TypedArray<T>
where
    K::V: Copy,
{
    validate_layout(layout);

    let dep = layout.max - layout.min;

    let side: u32 = 1 << dep;
    let plane = side * side;
    let total = if layout.dim == 2 { plane } else { plane * side };

    let out = TypedArray::<T>::new(total as usize);

    if layout.dim == 2 {
        let mut i = 0;
        for y in 0..side {
            for x in 0..side {
                let key = K::gather(x as i64, y as i64, 0);
                out.put(i, get::<K, T>(mem, layout, key, root));
                i += 1;
            }
        }
        return out;
    }

    let mut i = 0;
    for z in 0..side {
        for y in 0..side {
            for x in 0..side {
                let key = K::gather(x as i64, y as i64, z as i64);
                out.put(i, get::<K, T>(mem, layout, key, root));
                i += 1;
            }
        }
    }
    out
}

// ============ Traversal (Enumerate) ============

/// Visit every maximal uniform leaf block in the tree — the inverse of
/// `dense()`: instead of expanding to one value per finest-level cell, this
/// calls `visit` once per *block*, however coarse, with that block's own
/// corner (in min-level lattice units — multiply by `2**layout.min` for a
/// real-unit offset) and shift level (`2**level` is that block's real-unit
/// size). Skips branch slots entirely (never visits an internal node, only
/// leaves), so a caller building a sparse "list of occupied regions" view
/// (e.g. one render/placement record per uniform block) never has to
/// re-detect uniformity itself the way `dense()`-then-rescan would.
pub fn walk<T: NumberElement>(mem: &SparseTreeMemory<T>, layout: &SparseTreeLayout, visit: &mut dyn FnMut(i32, i32, i32, i32, T), root: u32) {
    validate_layout(layout);
    let dep = layout.max - layout.min;
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

    _walk(mem, layout.dim, layout.min, root, dep - 1, 0, 0, 0, visit);
}

#[allow(clippy::too_many_arguments)]
fn _walk<T: NumberElement>(mem: &SparseTreeMemory<T>, dim: u32, min: i32, base: u32, u: i32, x: i32, y: i32, z: i32, visit: &mut dyn FnMut(i32, i32, i32, i32, T)) {
    let width = block_width(dim);

    for c in 0..width {
        let slot = (base + c) as usize;
        let node = mem.heap[slot];

        let bit_x = (c & 1) as i32;
        let bit_y = if dim >= 2 { ((c >> 1) & 1) as i32 } else { 0 };
        let bit_z = if dim >= 3 { ((c >> 2) & 1) as i32 } else { 0 };
        let cx = x | (bit_x << u);
        let cy = y | (bit_y << u);
        let cz = z | (bit_z << u);

        if node & NODE_MSB != 0 {
            _walk(mem, dim, min, node & BLOCK_MASK, u - 1, cx, cy, cz, visit);
        } else {
            visit(cx, cy, cz, min + u, mem.data[slot]);
        }
    }
}

// ============ Maintenance (Pack) ============

/// What `compact` hands back: the memory, and its root base (0).
pub struct Compacted<T: NumberElement> {
    pub mem: SparseTreeMemory<T>,
    pub root: u32,
}

/// Compact the tree by pruning redundant nodes and relocating live blocks.
pub fn compact<T: NumberElement>(mem: &mut SparseTreeMemory<T>, layout: &SparseTreeLayout, root: u32) -> Compacted<T> {
    validate_layout(layout);

    let width = block_width(layout.dim);

    _prune(mem, root, layout.max - layout.min, width);

    // The TypeScript hands the one allocator to the new memories; here the
    // allocator is boxed, so the new memories share it through a forwarder.
    let shared = std::rc::Rc::new(core::cell::RefCell::new(core::mem::replace(&mut mem.alloc, Box::new(HEAP_ALLOC))));
    let mut new_mem = alloc::<T>(layout.dim, mem.head, Some(Box::new(Shared(shared.clone()))));

    fn relocate<T: NumberElement>(mem: &SparseTreeMemory<T>, new_mem: &mut SparseTreeMemory<T>, dim: u32, width: u32, old_base: u32, new_base: u32) {
        for i in 0..width {
            let old_slot = (old_base + i) as usize;
            let new_slot = (new_base + i) as usize;
            let node = mem.heap[old_slot];

            if node & NODE_MSB != 0 {
                let child_base = node & BLOCK_MASK;
                let next_base = alloc_block(new_mem, dim);

                new_mem.heap.put(new_slot, next_base | NODE_MSB);
                relocate(mem, new_mem, dim, width, child_base, next_base);
            } else {
                new_mem.data.put(new_slot, mem.data[old_slot]);
            }
        }
    }

    relocate(mem, &mut new_mem, layout.dim, width, root, 0);

    let mut compacted = alloc::<T>(layout.dim, new_mem.head, Some(Box::new(Shared(shared.clone()))));

    compacted.heap.set(&new_mem.heap.subarray(0, new_mem.head as usize));
    compacted.data.set(&new_mem.data.subarray(0, new_mem.head as usize));
    compacted.head = new_mem.head;
    free(&mut new_mem);

    mem.alloc = Box::new(Shared(shared));
    Compacted { mem: compacted, root: 0 }
}

/// One allocator held by several memories.
struct Shared(std::rc::Rc<core::cell::RefCell<Box<dyn TypedAlloc>>>);

impl TypedAlloc for Shared {
    fn take_view(&mut self, kind: xpute_core::kit::array::Kind, length: u32) -> xpute_core::kit::array::ArrayBufferView {
        self.0.borrow_mut().take_view(kind, length)
    }
    fn give(&mut self, view: &xpute_core::kit::array::ArrayBufferView) {
        self.0.borrow_mut().give(view)
    }
}

/// Internal recursive prune.
///
/// Return:
/// - uniform leaf value if the whole block can collapse into one value
/// - null otherwise
fn _prune<T: NumberElement>(mem: &mut SparseTreeMemory<T>, base: u32, dep: i32, width: u32) -> Option<T> {
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
            let child_val = _prune(mem, child_base, dep - 1, width);

            if let Some(child_val) = child_val {
                mem.heap.put(slot, 0);
                mem.data.put(slot, child_val);
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

fn total_nbyte<T: NumberElement>(mem: &SparseTreeMemory<T>, data_bpe: u32) -> u32 {
    let hdr_nbyte = WORD_SZ; // [head(u32) | reserved(u32)]

    let used = mem.head;
    let heap_nbyte = ALIGN(used << 2, WORD_SZ).unwrap();
    let data_nbyte = ALIGN(used * data_bpe, WORD_SZ).unwrap();

    hdr_nbyte + heap_nbyte + data_nbyte
}

pub fn encode<T: NumberElement>(mem: &SparseTreeMemory<T>) -> U64Array {
    let data_bpe = T::BYTES_PER_ELEMENT as u32;
    let total = total_nbyte(mem, data_bpe);

    let buf: U8Array = TypedArray::new(total as usize);
    write_mem(mem, &buf);

    TypedArray::over(buf.buffer().clone(), buf.byte_offset(), (total >> 3) as usize)
}

fn write_mem<T: NumberElement>(mem: &SparseTreeMemory<T>, buf: &U8Array) -> u32 {
    let used = mem.head;

    let mut off: u32 = 0;

    SET_WORD(buf, off, used, RESERVED);
    off += WORD_SZ;

    let heap_nbyte = used << 2;
    let heap_buf: U8Array = TypedArray::over(mem.heap.buffer().clone(), mem.heap.byte_offset(), heap_nbyte as usize);
    buf.set_at(&heap_buf, off as usize);
    off += ALIGN(heap_nbyte, WORD_SZ).unwrap();

    let data_bpe = T::BYTES_PER_ELEMENT as u32;
    let data_nbyte = used * data_bpe;
    let data_buf: U8Array = TypedArray::over(mem.data.buffer().clone(), mem.data.byte_offset(), data_nbyte as usize);
    buf.set_at(&data_buf, off as usize);
    off += ALIGN(data_nbyte, WORD_SZ).unwrap();

    off
}

pub fn decode<T: NumberElement>(buf64: &U64Array) -> SparseTreeMemory<T> {
    let buf: U8Array = TypedArray::over(buf64.buffer().clone(), buf64.byte_offset(), buf64.byte_length());
    read_mem::<T>(&buf).0
}

fn read_mem<T: NumberElement>(buf: &U8Array) -> (SparseTreeMemory<T>, u32) {
    let mut off: u32 = 0;

    let mut reg = WORD_REG;
    let [used, _] = GET_WORD(buf, off, &mut reg);
    off += WORD_SZ;

    let heap_off = off;
    let heap_nbyte = used << 2;
    off += ALIGN(heap_nbyte, WORD_SZ).unwrap();

    let data_off = off;
    let data_bpe = T::BYTES_PER_ELEMENT as u32;
    let data_nbyte = used * data_bpe;
    off += ALIGN(data_nbyte, WORD_SZ).unwrap();

    if off as usize > buf.byte_length() {
        panic!("sparse_tree: truncated buffer");
    }

    let buffer: ArrayBuffer = buf.buffer().clone();
    let heap: U32Array = TypedArray::over(buffer.clone(), buf.byte_offset() + heap_off as usize, used as usize);

    let data: TypedArray<T> = if (buf.byte_offset() + data_off as usize).is_multiple_of(data_bpe as usize) {
        TypedArray::over(buffer.clone(), buf.byte_offset() + data_off as usize, used as usize)
    } else {
        copy_ab_view::<T>(&buffer, (buf.byte_offset() + data_off as usize) as u32, used)
    };

    let mem = SparseTreeMemory {
        heap,
        data,
        head: used,
        cap: used,
        alloc: Box::new(HEAP_ALLOC),
    };

    (mem, off)
}
