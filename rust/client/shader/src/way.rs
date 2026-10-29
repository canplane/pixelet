// pxl-shader/way.rs

//! Each handed brick's ways: their spans off its lines (`way_cs`), and
//! their tree in the ways' pool (pxl-spec-core `way::tree`), the old one
//! given back (`way_free_cs`), the new one built (`way_tree_cs`) and the
//! families the build gave back pushed (`way_freed_cs`).

use pxl_spec_core::at;
use pxl_spec_core::fill::{self, put};
use pxl_spec_core::overwrite;
use pxl_spec_core::way::tree::{self, Pool, FREED_AT, RECORD_WORDS};
use pxl_spec_core::window;
use spirv_std::arch::IndexUnchecked;
use spirv_std::glam::UVec3;
use spirv_std::memory::{Scope, Semantics};
use spirv_std::spirv;

use crate::add;
use crate::world::brick_at;

/// The ways of the squares `fresh` names, one a `z`, a thread a column
/// (`x`, cells row by row): each column's spans and those beside it off the
/// square's lines (pxl-spec-core `ways::around`) read along z as runs
/// (`ways::runs`), into the square's scratch from its column's first
/// (`window::FIRSTS_AT`). Four dispatches: `pass.x` 3 first, each column
/// of the square and its ring its spans once into the scratch
/// (`fill::spans`), which the rest read; then 0 to 2: each column
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
    #[spirv(storage_buffer, descriptor_set = 0, binding = 0)] fresh: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] ground: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] scratch: &mut [u32],
) {
    if pass.x == 3 {
        fill::spans(ground, fresh, scratch, id.z, id.x);
        return;
    }
    if pass.x != 1 {
        fill::ways(ground, fresh, scratch, id.z, pass.x == 2, id.x);
        return;
    }
    let t = lane.x;
    let own = fill::stretch_runs(scratch, id.z, t);
    fill::scanned(scratch, id.z, t, scan(sums, t, own) - own);
}

/// The lanes of a workgroup a brick: `way_tree_cs`'s, which a level's
/// scan ranks the flagged across, and `way_freed_cs`'s.
const LANES: u32 = 64;

/// The workgroup's words, a lane's own: a scan's sums, a level's numbers.
type Shared = [u32; LANES as usize];

/// Thread `t`'s inclusive sum of the workgroup's `own`s, a step a doubling
/// (Hillis and Steele 1986; GPU Gems 3, ch. 39): each sum adds the one `d`
/// before it, both read before any is written. Each index is masked to the
/// array's length and read unchecked (spirv-std `IndexUnchecked`), so no
/// bounds check branches on the lane between the barriers, which WebGPU
/// takes only in uniform control flow; and two loads lie between them, a
/// lane's own, so they are no one load every lane shares
/// (pxl-shader-build's `workgroupUniformLoad`). `way_tree_cs`'s scan.
fn scan(sums: &mut Shared, t: u32, own: u32) -> u32 {
    let at = |v: u32| (v & (LANES - 1)) as usize;
    // SAFETY: every index is masked under the array's length.
    unsafe { *sums.index_unchecked_mut(at(t)) = own };
    let mut d = 1;
    while d < LANES {
        spirv_std::arch::workgroup_memory_barrier_with_group_sync();
        // SAFETY: masked, as above.
        let (before, sum) = unsafe {
            (
                *sums.index_unchecked(at(t.wrapping_sub(d))),
                *sums.index_unchecked(at(t)),
            )
        };
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
fn load_shared(w: &Shared, i: u32) -> u32 {
    spirv_std::arch::workgroup_memory_barrier_with_group_sync();
    // SAFETY: masked under the array's length.
    let v = unsafe { *w.index_unchecked((i & (LANES - 1)) as usize) };
    spirv_std::arch::workgroup_memory_barrier_with_group_sync();
    v
}

/// Word `i` of the workgroup's `w`, as the lane that wrote it reads it.
fn lane_word(w: &Shared, i: u32) -> u32 {
    // SAFETY: masked under the array's length.
    unsafe { *w.index_unchecked((i & (LANES - 1)) as usize) }
}

/// Word `i` of the workgroup's `w` set to `v`, by lane 0 alone.
fn set_shared(w: &mut Shared, t: u32, i: u32, v: u32) {
    if t == 0 {
        // SAFETY: masked under the array's length.
        unsafe { *w.index_unchecked_mut((i & (LANES - 1)) as usize) = v };
    }
}

/// Every lane's writes to storage seen by every lane of the workgroup
/// after it, WGSL's `storageBarrier`: a level's slots, written by the lanes
/// of the one before, read by others.
fn settle() {
    spirv_std::arch::control_barrier::<
        { spirv_std::memory::Scope::Workgroup as u32 },
        { spirv_std::memory::Scope::Workgroup as u32 },
        {
            spirv_std::memory::Semantics::UNIFORM_MEMORY.bits()
                | spirv_std::memory::Semantics::ACQUIRE_RELEASE.bits()
        },
    >();
}

/// The ways' pool's free stack, a word a family of the pool's, the two
/// grown together (pxl-render gpu/ray/ground.rs `GroundBuffers::grow`),
/// which starts as WebGPU makes a buffer, zeroed: word `TAKEN` how many
/// families are taken, the slots after it their ids, then the free families'
/// ids, family 0 never among them, a brick's root word 0 naming no tree as
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

/// `n` families off the stack together for a brick that has taken `taken`:
/// where their ids begin in it, and false where fewer are free or the
/// brick would take more than its most (`way::tree::fits`), none taken.
/// One exit, for the reason `add` gives.
fn pop(stack: &mut [u32], taken: u32, n: u32) -> (u32, bool) {
    let len = stack.len() as u32;
    // SAFETY: word `TAKEN`, the stack's first; the buffer is never empty
    // (pxl-render gpu/ray/ground.rs makes it a word a family of the pool's).
    let cnt = unsafe { stack.get_unchecked_mut(TAKEN as usize) };
    let mut gone = unsafe {
        spirv_std::arch::atomic_load::<u32, { Scope::QueueFamily as u32 }, { Semantics::NONE.bits() }>(
            cnt,
        )
    };
    let (mut done, mut ok) = (false, false);
    while !done {
        if !tree::fits(free(len, gone), taken, n) {
            done = true;
        } else {
            // SAFETY: as above.
            let was = unsafe {
                spirv_std::arch::atomic_compare_exchange::<
                    u32,
                    { Scope::QueueFamily as u32 },
                    { Semantics::NONE.bits() },
                    { Semantics::NONE.bits() },
                >(cnt, gone + n, gone)
            };
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
fn give_back(scratch: &mut [u32], at: u32, family: u32) {
    let r = add(scratch, at + FREED_AT, 1);
    put(scratch, at + FREED_AT + 1 + r, family);
}

/// Each re-filled brick's old ways' tree given back to the pool, a
/// workgroup a brick, ahead of its build (`way_tree_cs`): walked from its
/// root word, depth first, every family its own and its branches' pushed.
#[spirv(compute(threads(64, 1, 1), entry_point_name = "way_free_cs"))]
pub fn way_free_cs(
    #[spirv(workgroup_id)] group: UVec3,
    #[spirv(local_invocation_id)] lane: UVec3,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 0)] fresh: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] ground: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] pool: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 3)] stack: &mut [u32],
) {
    let root = at(
        ground,
        brick_at(at(fresh, window::handed_at(group.z) + window::FRESH_BRICK)) + window::ROOT_AT,
    );
    if lane.x != 0 || root == 0 {
        return;
    }
    let heap = |slot: u32| at(pool, slot);
    // Each level's family and the next of its eight to look under, from the
    // brick's own nodes down to its cells.
    let mut path = [[0u32; 2]; window::BRICK_SHIFT as usize + 1];
    let (mut d, mut first) = (0usize, tree::root_slot(root).0);
    path[0] = [first, 0];
    push(stack, first / 8);
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

/// Each handed brick's ways as a tree of the placed tree's shape
/// (pxl-spec-core `way::tree`), a workgroup a brick, `group.z` its place
/// in `fresh`: from the brick's own nodes along z its ways' bound meets
/// (`tree::span`) down a level at a time, each
/// candidate flagged where the brick's levels say a way may lie under it
/// and the flagged given their children's families off the pool's stack
/// together, in the order the workgroup's scan ranks them; at the brick's
/// cells each a leaf off the spans kept (`way::lined`); then up, each
/// branch folded, one of eight equal leaves written as that leaf and its
/// family given back (`way_freed_cs` pushes them), every family it holds
/// kept as a record for the read-back. Its root word last; none where it
/// has no ways, nothing taken nor kept, or where the pool or its scratch ran
/// out, the families it took given back, and its records' count saying
/// which (`tree::STARVED`, `tree::CRAMPED`). A level's numbers are what
/// every lane reads alike (`load_shared`), the loops' bounds.
#[spirv(compute(threads(64, 1, 1), entry_point_name = "way_tree_cs"))]
#[allow(clippy::too_many_arguments)]
pub fn way_tree_cs(
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
    let (k, t, lanes) = (group.z, lane.x, LANES);
    let base = brick_at(at(fresh, window::handed_at(k) + window::FRESH_BRICK));
    let mine = fill::scratch_at(scratch, k);
    // The families a brick's scratch holds, every brick's alike, and where
    // its records begin (`tree::scratch_words`).
    let room = tree::scratch_families(scratch.len() as u32 / window::HANDED);
    let records = mine + tree::records_at(room);
    let mut pool = tree::Lane(pool);
    // A level's candidate `c`: slot `c % 8` of the family whose id is the
    // level's `c / 8`-th off the stack, from where its ids begin.
    let slot = |stack: &[u32], from: u32, c: u32| 8 * id(stack, from + c / 8) + c % 8;
    // levels[2j], levels[2j + 1]: level j's first id on the stack and its
    // candidates, level 0 the brick's own nodes along z; the last four
    // words, the first one's Z, the families the brick has taken, its
    // cells' zoom and whether the pool held every level so far (`HELD`), or
    // the brick has no ways (`BARE`), or the level the brick took past its
    // own most (`PAST`), or past what its scratch holds (`SHORT`); 0 where
    // the pool ran out.
    let (z0, taken, cells, held) = (lanes - 4, lanes - 3, lanes - 2, lanes - 1);
    const HELD: u32 = 1;
    const PAST: u32 = 2;
    const SHORT: u32 = 3;
    const BARE: u32 = 4;
    if t == 0 {
        put(scratch, mine + FREED_AT, 0);
        put(scratch, records, 0);
        let cell = fill::cell_of(fresh, k);
        let (z, nodes) = fill::root_span(ground, base, cell);
        let (from, state) = if nodes == 0 {
            (0, BARE)
        } else if nodes > tree::ROOT_NODES {
            (0, PAST)
        } else {
            let (from, ok) = pop(stack, 0, 1);
            if ok {
                tree::root(&mut pool, z, 8 * id(stack, from));
            }
            (from, ok as u32)
        };
        let ok = state == HELD;
        set_shared(levels, t, taken, ok as u32);
        set_shared(levels, t, 0, from);
        set_shared(levels, t, 1, if ok { tree::ROOT_NODES } else { 0 });
        set_shared(levels, t, z0, z as u32);
        set_shared(levels, t, cells, cell);
        set_shared(levels, t, held, state);
    }
    let (cell, first) = (load_shared(levels, cells), load_shared(levels, 0));
    let (mut j, mut zoom) = (0, cell - window::BRICK_SHIFT);
    let mut ok = load_shared(levels, held) == HELD;
    while ok && zoom < cell {
        settle();
        let (from, n) = (load_shared(levels, 2 * j), load_shared(levels, 2 * j + 1));
        let brick = fill::Fresh { ground, base };
        // The flagged, counted, then given their families together.
        let (mut flagged, mut c) = (0, 0);
        while c < n {
            // Asked of every lane, its own past the level or not, then
            // chosen: a short circuit would put the scan after it in a
            // branch of the lane's own, where no barrier may be.
            let i = c + t;
            let flag = tree::flag(&pool, &brick, 0, cell, zoom, slot(stack, from, i)) & (i < n);
            scan(sums, t, flag as u32);
            flagged += load_shared(sums, lanes - 1);
            c += lanes;
        }
        if t == 0 {
            let had = lane_word(levels, taken);
            // Past the brick's most, or past its scratch, none is taken:
            // the pool is asked only of what the scratch holds.
            let short = if tree::past(had, flagged) {
                PAST
            } else if tree::is_cramped(room, had, flagged) {
                SHORT
            } else {
                0
            };
            let (next, fits) = if short == 0 {
                pop(stack, had, flagged)
            } else {
                (0, false)
            };
            set_shared(levels, t, taken, had + flagged);
            set_shared(levels, t, 2 * j + 2, next);
            set_shared(levels, t, 2 * j + 3, 8 * flagged);
            set_shared(levels, t, held, if fits { HELD } else { short });
        }
        ok = load_shared(levels, held) == HELD;
        let next = load_shared(levels, 2 * j + 2);
        let (mut before, mut c) = (0, 0);
        while ok && c < n {
            let i = c + t;
            let s = slot(stack, from, i);
            let flag = tree::flag(&pool, &brick, 0, cell, zoom, s) & (i < n);
            let rank = before + scan(sums, t, flag as u32) - flag as u32;
            if flag {
                tree::branch(&mut pool, s, 8 * id(stack, next + rank));
            } else if i < n {
                tree::empty(&mut pool, s);
            }
            before += load_shared(sums, lanes - 1);
            c += lanes;
        }
        (j, zoom) = (j + 1, zoom + 1);
    }
    // The pool or the scratch ran out: the families each level before took,
    // given back.
    if !ok && t == 0 {
        let mut level = 0;
        while level < j {
            let (from, n) = (
                lane_word(levels, 2 * level),
                lane_word(levels, 2 * level + 1),
            );
            let mut b = 0;
            while b < n / 8 {
                give_back(scratch, mine, id(stack, from + b));
                b += 1;
            }
            level += 1;
        }
    }
    settle();
    let (from, n) = (load_shared(levels, 2 * j), load_shared(levels, 2 * j + 1));
    // Every loop a lane's share steps the same chunks, its bound what every
    // lane reads alike, so the barriers after it are in uniform control
    // flow, which WebGPU takes only there.
    let mut c = 0;
    while ok && c < n {
        let kept = fill::Scratch {
            words: scratch,
            at: mine,
        };
        if c + t < n {
            tree::leaf(&mut pool, &kept, cell, slot(stack, from, c + t));
        }
        c += lanes;
    }
    // Up: each level's branches folded over the level under them.
    while ok && j > 0 {
        settle();
        (j, zoom) = (j - 1, zoom - 1);
        let (from, n) = (load_shared(levels, 2 * j), load_shared(levels, 2 * j + 1));
        let mut c = 0;
        while c < n {
            let s = slot(stack, from, c + t);
            let under = tree::under(&pool, s);
            let first = if c + t < n { under } else { tree::NONE };
            if first != tree::NONE {
                let gave = tree::fold(&mut pool, zoom, s);
                if gave != tree::NONE {
                    give_back(scratch, mine, gave / 8);
                } else {
                    record(scratch, records, &pool, first);
                }
            }
            c += lanes;
        }
    }
    settle();
    if t == 0 {
        let family = id(stack, first);
        let short = lane_word(levels, held);
        if ok {
            record(scratch, records, &pool, 8 * family);
        } else if short == 0 {
            // Past what the pool holds: its count `STARVED`, the pool's to
            // grow (pxl-render `Short`).
            put(scratch, records, tree::STARVED);
        } else if short == PAST {
            // Past the brick's own most, which no pool's growth lets it
            // build: its count `NONE`, refused at once (pxl-render `Landing`).
            put(scratch, records, tree::NONE);
        } else if short == SHORT {
            // Past what its scratch holds: its count `CRAMPED`, the
            // scratch's to grow, as none is the pool's (pxl-render `Short`).
            put(scratch, records, tree::CRAMPED);
        }
        let root = if ok {
            tree::root_word(family, lane_word(levels, z0) as i32)
        } else {
            0
        };
        put(ground, base + window::ROOT_AT, root);
    }
}

/// The families each handed brick's build gave back (`way_tree_cs`), on the
/// stack again: after the build, so no push lands under another brick's
/// ids while it reads them.
#[spirv(compute(threads(64, 1, 1), entry_point_name = "way_freed_cs"))]
pub fn way_freed_cs(
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
        r += LANES;
    }
}
