// pxl-shader/ways.rs

//! Each handed square's ways: their runs off its lines (`ways_cs`), and
//! their tree in the ways' pool (pxl-spec-core `ways::tree`), the old one
//! given back (`ways_free_cs`), the new one built (`ways_tree_cs`) and the
//! families the build gave back pushed (`ways_freed_cs`).

use pxl_spec_core::fill::{self, put};
use pxl_spec_core::overwrite;
use pxl_spec_core::ways::tree::{self, Pool, FREED_AT, RECORD_WORDS};
use pxl_spec_core::window;
use pxl_spec_core::{at, LEAF_CELL};
use spirv_std::arch::IndexUnchecked;
use spirv_std::glam::{UVec3, UVec4};
use spirv_std::memory::{Scope, Semantics};
use spirv_std::spirv;

use crate::add;
use crate::world::square_at;

/// The ways of the squares `fresh` names, one a `z`, a thread a column
/// (`x`, cells row by row): each column's spans and those beside it off the
/// square's lines (pxl-spec-core `ways::around`) read along z as runs
/// (`ways::runs`), into the square's scratch from its column's first
/// (`window::FIRSTS_AT`). Three dispatches, `pass.x` 0 to 2: each column
/// counts its runs into its first; a workgroup a square sums them into each
/// column's first, each
/// thread a stretch (`fill::stretch_runs`), the stretches' sums scanned in
/// the workgroup's memory (Hillis and Steele 1986; GPU Gems 3, ch. 39) and
/// each stretch's firsts written off it (`fill::scanned`); each column
/// writes its runs from its first. Over `heights_cs`'s tops: a way is told
/// from their faces. Each square's entry as handed whole: where it lies
/// and its shard lay the area's edge (pxl-spec-core
/// `ways::Columns::walled`).
#[spirv(compute(threads(64, 1, 1), entry_point_name = "ways_cs"))]
pub fn ways_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(local_invocation_id)] lane: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] pass: &UVec4,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] fresh: &[u32],
    // Read and written, as `bound_cs` binds it: the two share one group.
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] ground: &mut [u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 3)] scratch: &mut [u32],
    #[spirv(workgroup)] sums: &mut [u32; fill::SCAN_THREADS as usize],
) {
    if pass.x != 1 {
        fill::ways(ground, fresh, scratch, id.z, pass.x == 2, id.x);
        return;
    }
    let t = lane.x;
    let own = fill::stretch_runs(scratch, id.z, t);
    fill::scanned(scratch, id.z, t, scan(sums, t, own) - own);
}

/// The workgroup's words, a lane's own: a scan's sums, a level's numbers.
type Shared = [u32; fill::SCAN_THREADS as usize];

/// Thread `t`'s inclusive sum of the workgroup's `own`s, a step a doubling
/// (Hillis and Steele 1986; GPU Gems 3, ch. 39): each sum adds the one `d`
/// before it, both read before any is written. Each index is masked to the
/// array's length and read unchecked (spirv-std `IndexUnchecked`), so no
/// bounds check branches on the lane between the barriers, which WebGPU
/// takes only in uniform control flow; and two loads lie between them, a
/// lane's own, so they are no one load every lane shares
/// (pxl-shader-build's `workgroupUniformLoad`). `ways_cs`'s and
/// `ways_tree_cs`'s one scan.
fn scan(sums: &mut Shared, t: u32, own: u32) -> u32 {
    let at = |v: u32| (v & (fill::SCAN_THREADS - 1)) as usize;
    // SAFETY: every index is masked under the array's length.
    unsafe { *sums.index_unchecked_mut(at(t)) = own };
    let mut d = 1;
    while d < fill::SCAN_THREADS {
        spirv_std::arch::workgroup_memory_barrier_with_group_sync();
        // SAFETY: masked, as above.
        let (before, sum) = unsafe { (*sums.index_unchecked(at(t.wrapping_sub(d))), *sums.index_unchecked(at(t))) };
        spirv_std::arch::workgroup_memory_barrier_with_group_sync();
        // SAFETY: masked, as above.
        unsafe { *sums.index_unchecked_mut(at(t)) = sum + if t >= d { before } else { 0 } };
        d *= 2;
    }
    // SAFETY: masked, as above.
    unsafe { *sums.index_unchecked(at(t)) }
}

/// Word `i` of the workgroup's `w` as every lane reads it alike: the
/// barriers about the one load, which pxl-shader-build reads back as
/// WGSL's `workgroupUniformLoad`, so a loop bound on it is uniform.
fn shared(w: &Shared, i: u32) -> u32 {
    spirv_std::arch::workgroup_memory_barrier_with_group_sync();
    // SAFETY: masked under the array's length.
    let v = unsafe { *w.index_unchecked((i & (fill::SCAN_THREADS - 1)) as usize) };
    spirv_std::arch::workgroup_memory_barrier_with_group_sync();
    v
}

/// Word `i` of the workgroup's `w`, as the lane that wrote it reads it.
fn lane_word(w: &Shared, i: u32) -> u32 {
    // SAFETY: masked under the array's length.
    unsafe { *w.index_unchecked((i & (fill::SCAN_THREADS - 1)) as usize) }
}

/// Word `i` of the workgroup's `w` set to `v`, by lane 0 alone.
fn set_shared(w: &mut Shared, t: u32, i: u32, v: u32) {
    if t == 0 {
        // SAFETY: masked under the array's length.
        unsafe { *w.index_unchecked_mut((i & (fill::SCAN_THREADS - 1)) as usize) = v };
    }
}

/// Every lane's writes to storage seen by every lane of the workgroup
/// after it, WGSL's `storageBarrier`: a level's slots, written by the lanes
/// of the one before, read by others.
fn settle() {
    spirv_std::arch::control_barrier::<
        { spirv_std::memory::Scope::Workgroup as u32 },
        { spirv_std::memory::Scope::Workgroup as u32 },
        { spirv_std::memory::Semantics::UNIFORM_MEMORY.bits() | spirv_std::memory::Semantics::ACQUIRE_RELEASE.bits() },
    >();
}

/// The ways' pool's free stack, a word a family of the pool's, the two
/// grown together (pxl-render gpu/ray/ground.rs `GroundBuffers::grow`),
/// which starts as WebGPU makes a buffer, zeroed: word `TAKEN` how many
/// families are taken, the slots after it their ids, then the free families'
/// ids, family 0 never among them, a square's root word 0 naming no tree as
/// a heap word 0 names no children. A slot's id is read and written by
/// `id` and `put_id` alone, a slot's word its id XOR its index, so a slot
/// never written holds its own index: every family free, with no start to
/// write, the ids handed from the lowest up, and a stack grown holds the
/// families past the old free, so every id ever handed lies under the most
/// ever taken, which the pool's and its mirror's growth keep to
/// (pxl-kernel domain/layer/terrain/mirror.rs).
const TAKEN: u32 = 0;

/// The families free when `taken` are of a stack of `len` slots, family 0
/// never one.
fn free(len: u32, taken: u32) -> u32 {
    len - 1 - taken
}

/// The id stack slot `i` holds.
fn id(stack: &[u32], i: u32) -> u32 {
    at(stack, i) ^ i
}

/// Family `family` into stack slot `i`.
fn put_id(stack: &mut [u32], i: u32, family: u32) {
    put(stack, i, family ^ i);
}

/// Family `family` back on the stack, into the slot of the last taken.
fn push(stack: &mut [u32], family: u32) {
    // One fewer taken: an add of -1, as the count's word is unsigned.
    let was = add(stack, TAKEN, 1u32.wrapping_neg());
    put_id(stack, was, family);
}

/// `n` families off the stack together for a square that has taken `taken`:
/// where their ids begin in it, and false where fewer are free or the
/// square would take more than its most (`ways::tree::fits`), none taken.
/// One exit, for the reason `add` gives.
fn pop(stack: &mut [u32], taken: u32, n: u32) -> (u32, bool) {
    let len = stack.len() as u32;
    // SAFETY: word `TAKEN`, the stack's first; the buffer is never empty
    // (pxl-render gpu/ray/ground.rs makes it a word a family of the pool's).
    let count = unsafe { stack.get_unchecked_mut(TAKEN as usize) };
    let mut gone = unsafe { spirv_std::arch::atomic_load::<u32, { Scope::QueueFamily as u32 }, { Semantics::NONE.bits() }>(count) };
    let (mut done, mut ok) = (false, false);
    while !done {
        if !tree::fits(free(len, gone), taken, n) {
            done = true;
        } else {
            // SAFETY: as above.
            let was = unsafe { spirv_std::arch::atomic_compare_exchange::<u32, { Scope::QueueFamily as u32 }, { Semantics::NONE.bits() }, { Semantics::NONE.bits() }>(count, gone + n, gone) };
            (done, ok) = (was == gone, was == gone);
            if !done {
                gone = was;
            }
        }
    }
    (if ok { 1 + gone } else { 0 }, ok)
}

/// Family `first / 8`'s record, its id and its sixteen words, the next of
/// the records at `records` in the scratch (`tree::records_at`).
fn record(scratch: &mut [u32], records: u32, pool: &impl Pool, first: u32) {
    let r = add(scratch, records, 1);
    let w = records + 1 + r * RECORD_WORDS;
    put(scratch, w, first / 8);
    // Off the pool word by word, as `tree::lay` reads them, no array
    // between: a runtime index into one is a bounds check, whose panic's
    // path no loop about it may carry (`add`).
    let mut k = 0;
    while k < 8 {
        put(scratch, w + 1 + k, pool.heap(first + k));
        put(scratch, w + 9 + k, pool.data(first + k));
        k += 1;
    }
}

/// Family `family` the build gave back, into the scratch at `at`.
fn freed(scratch: &mut [u32], at: u32, family: u32) {
    let r = add(scratch, at + FREED_AT, 1);
    put(scratch, at + FREED_AT + 1 + r, family);
}

/// Each re-filled square's old ways' tree given back to the pool, a
/// workgroup a square, ahead of its build (`ways_tree_cs`): walked from its
/// root word, depth first, every family its own and its branches' pushed.
#[spirv(compute(threads(64, 1, 1), entry_point_name = "ways_free_cs"))]
pub fn ways_free_cs(
    #[spirv(workgroup_id)] group: UVec3,
    #[spirv(local_invocation_id)] lane: UVec3,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 0)] fresh: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] ground: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] pool: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 3)] stack: &mut [u32],
) {
    let root = at(ground, square_at(at(fresh, window::handed_at(group.z) + window::FRESH_SQUARE)) + window::ROOT_AT);
    if lane.x != 0 || root == 0 {
        return;
    }
    let heap = |slot: u32| at(pool, slot);
    // Each level's family and the next of its eight to look under.
    let mut path = [[0u32; 2]; LEAF_CELL as usize + 1];
    let (mut d, mut first) = (0usize, root);
    path[0] = [root, 0];
    push(stack, root / 8);
    loop {
        let [b, k] = path[d];
        if k == 8 {
            if d == 0 {
                break;
            }
            d -= 1;
            continue;
        }
        path[d][1] = k + 1;
        let h = heap(b + k);
        if h != 0 {
            first = h & !overwrite::BRANCH;
            push(stack, first / 8);
            d += 1;
            path[d] = [first, 0];
        }
    }
    let _ = first;
}

/// Each handed square's ways as a tree of the placed tree's shape
/// (pxl-spec-core `ways::tree`), a workgroup a square, `group.z` its place
/// in `fresh`: from the world's zoom-0 block down a level at a time, each
/// candidate flagged where the square's levels say a way may lie under it
/// and the flagged given their children's families off the pool's stack
/// together, in the order the workgroup's scan ranks them; at the square's
/// cells each a leaf off its column's runs; then up, each branch folded,
/// one of eight equal leaves written as that leaf and its family given back
/// (`ways_freed_cs` pushes them), every family it holds kept as a record for
/// the read-back. Its root word last; none where the pool or its scratch
/// ran out, the families it took given back, and its records' count saying
/// which (`tree::CRAMPED`). A level's numbers are what every lane reads
/// alike (`shared`), the loops' bounds.
#[spirv(compute(threads(64, 1, 1), entry_point_name = "ways_tree_cs"))]
#[allow(clippy::too_many_arguments)]
pub fn ways_tree_cs(
    #[spirv(workgroup_id)] group: UVec3,
    #[spirv(local_invocation_id)] lane: UVec3,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 0)] fresh: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] ground: &mut [u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] pool: &mut [u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 3)] stack: &mut [u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 4)] scratch: &mut [u32],
    #[spirv(workgroup)] sums: &mut Shared,
    #[spirv(workgroup)] levels: &mut Shared,
) {
    let (k, t, lanes) = (group.z, lane.x, fill::SCAN_THREADS);
    let of = fill::of(fresh, k);
    let base = square_at(at(fresh, window::handed_at(k) + window::FRESH_SQUARE));
    let mine = fill::scratch_at(scratch, k);
    // The families a square's scratch holds, every square's alike, and where
    // its records begin (`tree::scratch_words`).
    let room = tree::scratch_families(scratch.len() as u32 / window::HANDED);
    let records = mine + tree::records_at(room);
    let mut pool = tree::Lane(pool);
    // A level's candidate `c`: slot `c % 8` of the family whose id is the
    // level's `c / 8`-th off the stack, from where its ids begin.
    let slot = |stack: &[u32], from: u32, c: u32| 8 * id(stack, from + c / 8) + c % 8;
    // levels[2j], levels[2j + 1]: level j's first id on the stack and its
    // candidates; the last three words, the families the square has taken, its
    // cells' zoom and whether the pool held every level so far (`HELD`), or
    // the level the square took past its own most (`PAST`), or past what its
    // scratch holds (`SHORT`).
    let (taken, cells, held) = (lanes - 3, lanes - 2, lanes - 1);
    const HELD: u32 = 1;
    const PAST: u32 = 2;
    const SHORT: u32 = 3;
    if t == 0 {
        put(scratch, mine + FREED_AT, 0);
        put(scratch, records, 0);
        let (from, ok) = pop(stack, 0, 1);
        set_shared(levels, t, taken, 1);
        if ok {
            tree::root(&mut pool, of, 8 * id(stack, from));
        }
        set_shared(levels, t, 0, from);
        set_shared(levels, t, 1, if ok { 8 } else { 0 });
        set_shared(levels, t, cells, of.cell);
        set_shared(levels, t, held, ok as u32);
    }
    let (cell, first) = (shared(levels, cells), shared(levels, 0));
    let (mut j, mut zoom) = (0, 0);
    let mut ok = shared(levels, held) == HELD;
    while ok && zoom < cell {
        settle();
        let (from, n) = (shared(levels, 2 * j), shared(levels, 2 * j + 1));
        let square = fill::Fresh { ground, base };
        // The flagged, counted, then given their families together.
        let (mut flagged, mut c) = (0, 0);
        while c < n {
            // Asked of every lane, its own past the level or not, then
            // chosen: a short circuit would put the scan after it in a
            // branch of the lane's own, where no barrier may be.
            let i = c + t;
            let flag = tree::flag(&pool, &square, 0, of, zoom, slot(stack, from, i)) & (i < n);
            scan(sums, t, flag as u32);
            flagged += shared(sums, lanes - 1);
            c += lanes;
        }
        if t == 0 {
            let had = lane_word(levels, taken);
            // Past the square's most, or past its scratch, none is taken:
            // the pool is asked only of what the scratch holds.
            let short = if tree::past(had, flagged) {
                PAST
            } else if tree::cramped(room, had, flagged) {
                SHORT
            } else {
                0
            };
            let (next, fits) = if short == 0 { pop(stack, had, flagged) } else { (0, false) };
            set_shared(levels, t, taken, had + flagged);
            set_shared(levels, t, 2 * j + 2, next);
            set_shared(levels, t, 2 * j + 3, 8 * flagged);
            set_shared(levels, t, held, if fits { HELD } else { short });
        }
        ok = shared(levels, held) == HELD;
        let next = shared(levels, 2 * j + 2);
        let (mut before, mut c) = (0, 0);
        while ok && c < n {
            let i = c + t;
            let s = slot(stack, from, i);
            let flag = tree::flag(&pool, &square, 0, of, zoom, s) & (i < n);
            let rank = before + scan(sums, t, flag as u32) - flag as u32;
            if flag {
                tree::branch(&mut pool, of, zoom, s, 8 * id(stack, next + rank));
            } else if i < n {
                tree::empty(&mut pool, s);
            }
            before += shared(sums, lanes - 1);
            c += lanes;
        }
        (j, zoom) = (j + 1, zoom + 1);
    }
    // The pool or the scratch ran out: the families each level before took,
    // given back.
    if !ok && t == 0 {
        let mut level = 0;
        while level < j {
            let (from, n) = (lane_word(levels, 2 * level), lane_word(levels, 2 * level + 1));
            let mut b = 0;
            while b < n / 8 {
                freed(scratch, mine, id(stack, from + b));
                b += 1;
            }
            level += 1;
        }
    }
    settle();
    let (from, n) = (shared(levels, 2 * j), shared(levels, 2 * j + 1));
    // Every loop a lane's share steps the same chunks, its bound what every
    // lane reads alike, so the barriers after it are in uniform control
    // flow, which WebGPU takes only there.
    let mut c = 0;
    while ok && c < n {
        let runs = fill::Scratch { words: scratch, at: mine };
        if c + t < n {
            tree::leaf(&mut pool, &runs, of, slot(stack, from, c + t));
        }
        c += lanes;
    }
    // Up: each level's branches folded over the level under them.
    while ok && j > 0 {
        settle();
        (j, zoom) = (j - 1, zoom - 1);
        let (from, n) = (shared(levels, 2 * j), shared(levels, 2 * j + 1));
        let mut c = 0;
        while c < n {
            let s = slot(stack, from, c + t);
            let under = tree::under(&pool, s);
            let first = if c + t < n { under } else { tree::NONE };
            if first != tree::NONE {
                let gave = tree::fold(&mut pool, zoom, s);
                if gave != tree::NONE {
                    freed(scratch, mine, gave / 8);
                } else {
                    record(scratch, records, &pool, first);
                }
            }
            c += lanes;
        }
    }
    settle();
    if t == 0 {
        let root = if ok { 8 * id(stack, first) } else { 0 };
        let short = lane_word(levels, held);
        if ok {
            record(scratch, records, &pool, root);
        } else if short == PAST {
            // Past the square's own most, which no pool's growth lets it
            // build: its count `NONE`, refused at once (pxl-render `Landing`).
            put(scratch, records, tree::NONE);
        } else if short == SHORT {
            // Past what its scratch holds: its count `CRAMPED`, the
            // scratch's to grow, as none is the pool's (pxl-render `Short`).
            put(scratch, records, tree::CRAMPED);
        }
        put(ground, base + window::ROOT_AT, root);
    }
}

/// The families each handed square's build gave back (`ways_tree_cs`), on the
/// stack again: after the build, so no push lands under another square's
/// ids while it reads them.
#[spirv(compute(threads(64, 1, 1), entry_point_name = "ways_freed_cs"))]
pub fn ways_freed_cs(
    #[spirv(workgroup_id)] group: UVec3,
    #[spirv(local_invocation_id)] lane: UVec3,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 0)] scratch: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] stack: &mut [u32],
) {
    let mine = fill::scratch_at(scratch, group.z);
    let n = at(scratch, mine + FREED_AT);
    let mut r = lane.x;
    while r < n {
        push(stack, at(scratch, mine + FREED_AT + 1 + r));
        r += fill::SCAN_THREADS;
    }
}
