// pxl-shader/observed.rs

//! The observed pyramid (pxl-spec-core `observed`): what the cast's walks
//! kept of the world's query, read and written by them, and forgotten where
//! what the query reads changed (`forget_cs`).

use pxl_spec_core::answer::In;
use pxl_spec_core::cast::Query;
use pxl_spec_core::observed;
use pxl_spec_core::world::At;
use spirv_std::glam::UVec3;
use spirv_std::memory::{Scope, Semantics};
use spirv_std::spirv;

use crate::world::{Looks, World};

/// The observed pyramid's buffer (pxl-spec-core `observed`), bound as a
/// runtime array, whose pointer a walk holds whole: a slice's would be a
/// pointer and a length, which naga traces no atomic through.
pub(crate) type Seen = spirv_std::RuntimeArray<u32>;

/// Word `i` of the observed pyramid's buffer.
pub(crate) fn seen_at(w: &Seen, i: u32) -> u32 {
    // SAFETY: an entry's or the head's word, within the buffer.
    unsafe { *w.index(i as usize) }
}

pub(crate) fn seen_put(w: &mut Seen, i: u32, v: u32) {
    // SAFETY: as `seen_at`.
    unsafe { *w.index_mut(i as usize) = v }
}

/// Word `i` read and written whole, where a walk may race another on it.
fn seen_load(w: &mut Seen, i: u32) -> u32 {
    // SAFETY: as `seen_at`.
    unsafe {
        spirv_std::arch::atomic_load::<u32, { Scope::QueueFamily as u32 }, { Semantics::NONE.bits() }>(
            w.index_mut(i as usize),
        )
    }
}

fn seen_store(w: &mut Seen, i: u32, v: u32) {
    // SAFETY: as `seen_at`.
    unsafe {
        spirv_std::arch::atomic_store::<
            u32,
            { Scope::QueueFamily as u32 },
            { Semantics::NONE.bits() },
        >(w.index_mut(i as usize), v)
    }
}

/// `visits`, the nodes a walk of `observer`'s asked, added to its lane
/// `lane` of the head where the walks count (pxl-spec-core
/// `observed::TALLY_ON`, `tally`).
pub(crate) fn tallied(w: &mut Seen, observer: u32, lane: u32, visits: u32) {
    if seen_load(w, observed::TALLY_ON) != 0 {
        // SAFETY: as `seen_at`.
        unsafe {
            spirv_std::arch::atomic_i_add::<
                u32,
                { Scope::QueueFamily as u32 },
                { Semantics::NONE.bits() },
            >(
                w.index_mut(observed::tally_at(observer, lane) as usize),
                visits,
            );
        }
    }
}

/// Word `i` set to `v` where it holds `was`; what it held.
fn seen_swap(w: &mut Seen, i: u32, was: u32, v: u32) -> u32 {
    // SAFETY: as `seen_at`.
    unsafe {
        spirv_std::arch::atomic_compare_exchange::<
            u32,
            { Scope::QueueFamily as u32 },
            { Semantics::NONE.bits() },
            { Semantics::NONE.bits() },
        >(w.index_mut(i as usize), v, was)
    }
}

/// The first slot of the bucket that may keep node `c` of `zoom`,
/// `observed::NONE` where none does (`observed::keeps`).
fn bucket_of(zoom: u32, c: [i32; 3]) -> u32 {
    if observed::keeps(zoom, c) {
        observed::entry_of(zoom, c)
    } else {
        observed::NONE
    }
}

/// Whether entry word `w` is node `c` of `zoom`, as its node words say.
fn is_node(seen: &Seen, w: u32, zoom: u32, c: [i32; 3]) -> bool {
    let n = w + observed::W_NODE;
    seen_at(seen, n) == zoom
        && seen_at(seen, n + 1) == c[0] as u32
        && seen_at(seen, n + 2) == c[1] as u32
        && seen_at(seen, n + 3) == c[2] as u32
}

/// Where a writing walk is: the node's slot, `observed::NONE` where none
/// keeps it, and what was handed it.
#[derive(Clone, Copy)]
pub(crate) struct Writ {
    pub(crate) entry: u32,
    pub(crate) at: At,
}

/// The observed pyramid as a beam ray's walk, a sample's, or a pixel's that
/// missed, reads it: a node its slot keeps at the zooms asked, unless this
/// pass is still writing it (`tag`), taken where the key still stood once
/// it was read; any other asked of the world's one query there and then,
/// from what was handed it, and written where this walk claims the slot.
pub(crate) struct Writing<'a, L> {
    pub(crate) world: World<'a, L>,
    pub(crate) seen: &'a mut Seen,
    pub(crate) tag: u32,
    /// Whether it writes a block a ray sees: a walk whose world reads no
    /// looks (the sun's, `Unlooked`) answers such a block without them, and
    /// another walk may read what it writes; splits, what is transparent and
    /// lacks carry none.
    pub(crate) blocks: bool,
}

impl<L: Looks> Query for Writing<'_, L> {
    type At = Writ;

    fn root(&mut self, node: [i32; 3]) -> Writ {
        Writ {
            entry: observed::NONE,
            at: Query::root(&mut self.world, node),
        }
    }

    fn child(&mut self, parent: &Writ, zoom: u32, node: [i32; 3]) -> Writ {
        Writ {
            entry: observed::NONE,
            at: Query::child(&mut self.world, &parent.at, zoom, node),
        }
    }

    fn node(
        &mut self,
        n: &mut Writ,
        zoom: u32,
        c: [i32; 3],
        asked: [u32; 2],
        t0: f32,
    ) -> In<[u32; 2]> {
        let first = bucket_of(zoom, c);
        // The slot this walk would write, and the key it read there: the
        // node's own, else a free one, else the one it takes over; none where
        // a walk of this pass keeps it.
        let (mut to, mut key) = (observed::NONE, observed::CLAIMED);
        if first != observed::NONE {
            let mut free = observed::NONE;
            let mut k = 0;
            while k < observed::BUCKET {
                let w = observed::entry_at(first + k);
                let held = seen_load(self.seen, w);
                if held != observed::CLAIMED && held != 0 && is_node(self.seen, w, zoom, c) {
                    if observed::holds(held, asked) {
                        if observed::tag_of(held) == self.tag {
                            // Kept by a walk of this pass: asked of the
                            // world, as its words may be half written, and
                            // left to that walk.
                            to = observed::NONE;
                            free = observed::NONE;
                            break;
                        }
                        let a = w + observed::W_AT;
                        let at = At::of_words([
                            seen_at(self.seen, a),
                            seen_at(self.seen, a + 1),
                            seen_at(self.seen, a + 2),
                            seen_at(self.seen, a + 3),
                            seen_at(self.seen, a + 4),
                        ]);
                        let kind = observed::kind_of(seen_at(self.seen, w + observed::W_KIND));
                        let block = [
                            seen_at(self.seen, w + observed::W_BLOCK),
                            seen_at(self.seen, w + observed::W_BLOCK + 1),
                        ];
                        if seen_load(self.seen, w) == held {
                            n.entry = first + k;
                            n.at = at;
                            return In { kind, block };
                        }
                        to = observed::NONE;
                        free = observed::NONE;
                        break;
                    }
                    // The node, asked at other zooms by an earlier frame:
                    // written over. One this frame asked so (the eye's and
                    // the sun's ask the nodes near the eye apart) stands
                    // beside it.
                    if observed::tag_of(held) / observed::PASSES < self.tag / observed::PASSES {
                        (to, key) = (first + k, held);
                    }
                } else if held == 0 && free == observed::NONE {
                    free = first + k;
                }
                k += 1;
            }
            if to == observed::NONE && free != observed::NONE {
                (to, key) = (free, 0);
            } else if to == observed::NONE && k == observed::BUCKET {
                let v = first + observed::victim(zoom, c);
                (to, key) = (v, seen_load(self.seen, observed::entry_at(v)));
            }
        }
        n.entry = to;
        let answer = Query::node(&mut self.world, &mut n.at, zoom, c, asked, t0);
        let w = observed::entry_at(to);
        if to != observed::NONE
            && (self.blocks
                || answer.kind != pxl_spec_core::answer::Kind::Block
                || pxl_spec_core::answer::transparent(answer.block[0]))
            && key != observed::CLAIMED
            && seen_swap(self.seen, w, key, observed::CLAIMED) == key
        {
            seen_put(
                self.seen,
                w + observed::W_KIND,
                observed::kind_word(answer.kind),
            );
            seen_put(self.seen, w + observed::W_BLOCK, answer.block[0]);
            seen_put(self.seen, w + observed::W_BLOCK + 1, answer.block[1]);
            let words = n.at.words();
            let a = w + observed::W_AT;
            seen_put(self.seen, a, words[0]);
            seen_put(self.seen, a + 1, words[1]);
            seen_put(self.seen, a + 2, words[2]);
            seen_put(self.seen, a + 3, words[3]);
            seen_put(self.seen, a + 4, words[4]);
            let k = w + observed::W_NODE;
            seen_put(self.seen, k, zoom);
            seen_put(self.seen, k + 1, c[0] as u32);
            seen_put(self.seen, k + 2, c[1] as u32);
            seen_put(self.seen, k + 3, c[2] as u32);
            seen_store(self.seen, w, observed::key(self.tag, asked));
        }
        answer
    }
}

/// What the observed pyramid kept forgotten, a thread a slot, where what
/// the world's query reads changed (pxl-spec-core `observed::FORGETS`):
/// every slot's key, or the key of each slot over a square that moved
/// (`observed::forgets`), over a box of what is placed or cut that changed
/// (`observed::over_box`), or handed a slot of a family of what is placed
/// that was written (`observed::reads_written`).
#[spirv(compute(threads(64, 1, 1), entry_point_name = "forget_cs"))]
pub fn forget_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 9)] seen: &mut Seen,
) {
    if id.x >= observed::ENTRIES {
        return;
    }
    let w = observed::entry_at(id.x);
    if seen_at(seen, observed::FORGETS) == observed::FORGETS_ALL {
        seen_put(seen, w + observed::W_KEY, 0);
        return;
    }
    if seen_at(seen, w + observed::W_KEY) == 0 {
        return;
    }
    let (zoom, x, y) = (
        seen_at(seen, w + observed::W_NODE),
        seen_at(seen, w + observed::W_NODE + 1) as i32,
        seen_at(seen, w + observed::W_NODE + 2) as i32,
    );
    let mut moved = [0; observed::MOVED_WORDS as usize];
    let mut k = 0;
    while k < observed::MOVED_WORDS {
        moved[k as usize] = seen_at(seen, observed::MOVED_AT + k);
        k += 1;
    }
    if observed::forgets(&moved, zoom, [x, y]) {
        seen_put(seen, w + observed::W_KEY, 0);
        return;
    }
    // Or handed a slot of a family of what is placed that was written.
    let placed = seen_at(seen, w + observed::W_PLACED);
    let mut f = 0;
    while f < seen_at(seen, observed::WRITTEN_AT) {
        if observed::reads_written(placed, seen_at(seen, observed::WRITTEN_AT + 1 + f)) {
            seen_put(seen, w + observed::W_KEY, 0);
            return;
        }
        f += 1;
    }
    // Or over a box of what is placed or cut that changed.
    let mut b = 0;
    while b < seen_at(seen, observed::BOXES_AT) {
        let at = observed::BOXES_AT + 1 + 4 * b;
        let changed = [
            seen_at(seen, at) as i32,
            seen_at(seen, at + 1) as i32,
            seen_at(seen, at + 2) as i32,
            seen_at(seen, at + 3) as i32,
        ];
        if observed::over_box(zoom, [x, y], changed) {
            seen_put(seen, w + observed::W_KEY, 0);
            return;
        }
        b += 1;
    }
}
