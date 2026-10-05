// pxl-spec/space/sparse_tree/mem.rs
//! Sparse 2^d-ary tree over POD arenas. `heap` holds tagged child pointers
//! (MSB set: base of another child block) and `data` holds leaf values, both
//! indexed by slot. The structural unit is a child block of `1 << dim` slots;
//! the root is the block at `root`. `dim` is 2 or 3 and `max` is coarser than `min`.

use xpute_core::wire::xtp::spec::{ALIGN, GET_WORD, NONE, RESERVED, SET_WORD, WORD_SZ};

use crate::space::hierarchy::{child_of, slot_of};
use crate::space::pvf3::PVecTraits;
use crate::space::zoom::Zoom;

const WORD_REG: [u32; 2] = [NONE, NONE];

/// `heap` and `data` share one slot index; slots `0..1 << dim` are the root block.
pub struct SparseTreeMemory<T: Copy + Default + PartialEq> {
    pub heap: Vec<u32>,
    pub data: Vec<T>,
    pub head: u32,
    pub cap: u32,
}

/// `min` is the leaf's zoom and `max` the root block's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SparseTreeLayout {
    pub dim: u32,
    pub min: Zoom,
    pub max: Zoom,
}

impl SparseTreeLayout {
    /// Its levels from the leaf to the root block.
    pub const fn depth(&self) -> i32 {
        self.min as i32 - self.max as i32
    }

    /// Levels from the leaf up to `target`.
    pub const fn steps_to(&self, target: Zoom) -> i32 {
        self.min as i32 - target as i32
    }
}

/// Heap entry: MSB set is a branch whose low bits are a child block base; clear is a leaf in `data[slot]`.
const NODE_MSB: u32 = 0x80000000;
const BLOCK_MASK: u32 = 0x7fffffff;

fn block_width(dim: u32) -> u32 {
    1 << dim
}

fn validate_layout(layout: &SparseTreeLayout) {
    let dep = layout.depth();
    if dep <= 0 {
        xpute_core::bug!(EBADMSG);
    }
}

pub fn alloc<T: Copy + Default + PartialEq>(dim: u32, cap: u32) -> SparseTreeMemory<T> {
    let root_width = block_width(dim);
    let cap = if cap >= root_width { cap } else { root_width };

    SparseTreeMemory {
        heap: vec![0u32; cap as usize],
        data: vec![T::default(); cap as usize],
        head: root_width,
        cap,
    }
}

/// The same tree with its data lane widened to another type.
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

fn alloc_block<T: Copy + Default + PartialEq>(mem: &mut SparseTreeMemory<T>, dim: u32) -> u32 {
    alloc_slots(mem, block_width(dim))
}

pub fn reset<T: Copy + Default + PartialEq>(mem: &mut SparseTreeMemory<T>, layout: &SparseTreeLayout) {
    validate_layout(layout);

    let root_width = block_width(layout.dim);
    mem.head = root_width;
    mem.heap.as_mut_slice()[..root_width as usize].fill(0);
    mem.data.as_mut_slice()[..root_width as usize].fill(T::default());
}

pub fn fill<T: Copy + Default + PartialEq>(mem: &mut SparseTreeMemory<T>, layout: &SparseTreeLayout, val: T) {
    reset(mem, layout);
    mem.data.as_mut_slice()[..mem.head as usize].fill(val);
}

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

/// The slot `key` reaches at `target`, or the uniform leaf above it where the descent stops early.
pub fn resolve<K: PVecTraits, T: Copy + Default + PartialEq>(mem: &SparseTreeMemory<T>, layout: &SparseTreeLayout, key: K::V, target: Zoom, root: u32) -> u32
where
    K::V: Copy,
{
    validate_layout(layout);

    let dep = layout.depth();
    let target_u = layout.steps_to(target);

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

/// Write one value, splitting only where needed. Reaching `target` claims
/// that slot as one leaf; any child block under it is orphaned until `compact`.
pub fn set<K: PVecTraits, T: Copy + Default + PartialEq>(mem: &mut SparseTreeMemory<T>, layout: &SparseTreeLayout, key: K::V, val: T, target: Zoom, root: u32)
where
    K::V: Copy,
{
    write::<K, T>(mem, layout, key, val, target, root, &mut [(0, 0); MAX_DEP]);
}

// A branch slot's `data` is free for the caller to hold what its child block
// folds to; `set_folded`, `compact` and `refold` keep it.
const MAX_DEP: usize = 32;

/// `set`, then refold every branch the write passed or made, deepest first.
pub fn set_folded<K: PVecTraits, T: Copy + Default + PartialEq>(mem: &mut SparseTreeMemory<T>, layout: &SparseTreeLayout, key: K::V, val: T, target: Zoom, root: u32, fold: &dyn Fn(&[T], Zoom) -> T)
where
    K::V: Copy,
{
    let mut path = [(0, 0); MAX_DEP];
    let n = write::<K, T>(mem, layout, key, val, target, root, &mut path);
    let width = block_width(layout.dim) as usize;
    for &(slot, u) in path[..n].iter().rev() {
        let child = (mem.heap[slot as usize] & BLOCK_MASK) as usize;
        let folded = fold(&mem.data[child..child + width], layout.min - u as Zoom);
        mem.data[slot as usize] = folded;
    }
}

/// Refold every branch slot's `data` from its child block, deepest first.
pub fn refold<T: Copy + Default + PartialEq>(mem: &mut SparseTreeMemory<T>, layout: &SparseTreeLayout, root: u32, fold: &dyn Fn(&[T], Zoom) -> T) {
    fn go<T: Copy + Default + PartialEq>(mem: &mut SparseTreeMemory<T>, width: u32, min: Zoom, base: u32, u: i32, fold: &dyn Fn(&[T], Zoom) -> T) {
        for c in 0..width {
            let slot = (base + c) as usize;
            let node = mem.heap[slot];
            if node & NODE_MSB != 0 {
                let child = node & BLOCK_MASK;
                go(mem, width, min, child, u - 1, fold);
                let folded = fold(&mem.data[child as usize..(child + width) as usize], min - u as Zoom);
                mem.data[slot] = folded;
            }
        }
    }
    validate_layout(layout);
    let dep = layout.depth();
    go(mem, block_width(layout.dim), layout.min, root, dep - 1, fold);
}

/// `set`'s write; records in `path` the branches it passed or made, shallowest first, and returns how many.
fn write<K: PVecTraits, T: Copy + Default + PartialEq>(mem: &mut SparseTreeMemory<T>, layout: &SparseTreeLayout, key: K::V, val: T, target: Zoom, root: u32, path: &mut [(u32, i32); MAX_DEP]) -> usize
where
    K::V: Copy,
{
    validate_layout(layout);

    let mut base = root;
    let width = block_width(layout.dim);

    let dep = layout.depth();
    if dep <= 0 {
        return 0;
    }
    xpute_core::ensure!(dep as usize <= MAX_DEP, ENOTRECOVERABLE, dep);

    let target_u = layout.steps_to(target);

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

/// Bottom-up build from dense leaves; returns the root block's base.
pub fn build<T: Copy + Default + PartialEq>(mem: &mut SparseTreeMemory<T>, layout: &SparseTreeLayout, leaves: &[T]) -> u32 {
    validate_layout(layout);

    let width = block_width(layout.dim);
    let dep = layout.depth();

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

/// Dense leaf values: x fastest, then y, then z.
pub fn dense<K: PVecTraits, T: Copy + Default + PartialEq>(mem: &SparseTreeMemory<T>, layout: &SparseTreeLayout, root: u32) -> Box<[T]>
where
    K::V: Copy,
{
    validate_layout(layout);

    let dep = layout.depth();

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

/// Visit every maximal uniform leaf block once, with its corner in steps of
/// `layout.min` and its scale. Branches are never visited.
pub fn walk<T: Copy + Default + PartialEq>(mem: &SparseTreeMemory<T>, layout: &SparseTreeLayout, visit: &mut dyn FnMut(i32, i32, i32, Zoom, T), root: u32) {
    validate_layout(layout);
    let dep = layout.depth();
    let width = block_width(layout.dim);

    // The root is always a block, so a uniform tree is `width` equal leaves;
    // report it as one block at `max`.
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
fn walk_of<T: Copy + Default + PartialEq>(mem: &SparseTreeMemory<T>, dim: u32, min: Zoom, base: u32, u: i32, x: i32, y: i32, z: i32, visit: &mut dyn FnMut(i32, i32, i32, Zoom, T)) {
    let width = block_width(dim);

    for c in 0..width {
        let slot = (base + c) as usize;
        let node = mem.heap[slot];

        let [bit_x, bit_y, bit_z] = child_of([0; 3], c as usize).map(|b| b as i32);
        let cx = x | (bit_x << u);
        let cy = y | (bit_y << u);
        let cz = z | (bit_z << u);

        if node & NODE_MSB != 0 {
            walk_of(mem, dim, min, node & BLOCK_MASK, u - 1, cx, cy, cz, visit);
        } else {
            visit(cx, cy, cz, min - u as Zoom, mem.data[slot]);
        }
    }
}

/// `walk` stopped at `target`: slots at `target` are reported with their
/// `data` even when they are branches, and coarser leaves at their own scale.
pub fn walk_to<T: Copy + Default + PartialEq>(mem: &SparseTreeMemory<T>, layout: &SparseTreeLayout, target: Zoom, visit: &mut dyn FnMut(i32, i32, i32, Zoom, T), root: u32) {
    #[allow(clippy::too_many_arguments)]
    fn go<T: Copy + Default + PartialEq>(mem: &SparseTreeMemory<T>, dim: u32, min: Zoom, base: u32, u: i32, target_u: i32, [x, y, z]: [i32; 3], visit: &mut dyn FnMut(i32, i32, i32, Zoom, T)) {
        for c in 0..block_width(dim) {
            let slot = (base + c) as usize;
            let node = mem.heap[slot];
            let [bx, by, bz] = child_of([0; 3], c as usize).map(|b| b as i32);
            let at = [x | (bx << u), y | (by << u), z | (bz << u)];
            if node & NODE_MSB != 0 && u > target_u {
                go(mem, dim, min, node & BLOCK_MASK, u - 1, target_u, at, visit);
            } else {
                visit(at[0], at[1], at[2], min - u as Zoom, mem.data[slot]);
            }
        }
    }
    validate_layout(layout);
    let dep = layout.depth();
    go(mem, layout.dim, layout.min, root, dep - 1, layout.steps_to(target).max(0), [0; 3], visit);
}

pub struct Compacted<T: Copy + Default + PartialEq> {
    pub mem: SparseTreeMemory<T>,
    pub root: u32,
}

/// Prune redundant branches and relocate live blocks so the root base is 0.
pub fn compact<T: Copy + Default + PartialEq>(mem: &mut SparseTreeMemory<T>, layout: &SparseTreeLayout, root: u32) -> Compacted<T> {
    validate_layout(layout);

    let width = block_width(layout.dim);

    prune(mem, root, layout.depth(), width);

    let mut new_mem = alloc::<T>(layout.dim, mem.head);

    fn relocate<T: Copy + Default + PartialEq>(mem: &SparseTreeMemory<T>, new_mem: &mut SparseTreeMemory<T>, dim: u32, width: u32, old_base: u32, new_base: u32) {
        for i in 0..width {
            let old_slot = (old_base + i) as usize;
            let new_slot = (new_base + i) as usize;
            let node = mem.heap[old_slot];

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

/// The value a block collapses to when uniform.
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

fn total_nbyte<T: Copy + Default + PartialEq>(mem: &SparseTreeMemory<T>, data_bpe: u32) -> u32 {
    let hdr_nbyte = WORD_SZ;

    let used = mem.head;
    let heap_nbyte = ALIGN(used << 2, WORD_SZ).unwrap();
    let data_nbyte = ALIGN(used * data_bpe, WORD_SZ).unwrap();

    hdr_nbyte + heap_nbyte + data_nbyte
}

pub fn encode<T: Copy + Default + PartialEq>(mem: &SparseTreeMemory<T>) -> Vec<u64> {
    let data_bpe = core::mem::size_of::<T>() as u32;
    let total = total_nbyte(mem, data_bpe);

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

fn bytes_of_mut<T: Copy>(a: &mut [T]) -> &mut [u8] {
    // SAFETY: as `bytes_of`.
    unsafe { core::slice::from_raw_parts_mut(a.as_mut_ptr() as *mut u8, core::mem::size_of_val(a)) }
}

fn bytes_of<T: Copy>(a: &[T]) -> &[u8] {
    // SAFETY: the elements read back as bytes, which is what the blob holds.
    unsafe { core::slice::from_raw_parts(a.as_ptr() as *const u8, core::mem::size_of_val(a)) }
}

fn read_mem<T: Copy + Default + PartialEq>(buf: &[u8]) -> (SparseTreeMemory<T>, u32) {
    // Sizes are summed in u64 so a hostile count cannot wrap under the buffer's length.
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

    // Copied, not viewed: a later grow reallocates.
    let mut heap = vec![0u32; used as usize];
    let mut data = vec![T::default(); used as usize];
    bytes_of_mut(&mut heap).copy_from_slice(&buf[heap_off as usize..heap_off as usize + used as usize * 4]);
    bytes_of_mut(&mut data).copy_from_slice(&buf[data_off as usize..data_off as usize + used as usize * data_bpe as usize]);

    let mem = SparseTreeMemory { heap, data, head: used, cap: used };

    (mem, off)
}
