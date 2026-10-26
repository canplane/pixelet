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

fn seen_add(w: &mut Seen, i: u32, v: u32) -> u32 {
    // SAFETY: as `seen_at`.
    unsafe {
        spirv_std::arch::atomic_i_add::<
            u32,
            { Scope::QueueFamily as u32 },
            { Semantics::NONE.bits() },
        >(w.index_mut(i as usize), v)
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

/// The observed pyramid as a pixel's walk reads it, writing nothing
/// (pxl-spec-core `observed`): a node it keeps at the zooms asked is its
/// entry's answer; any other is a miss, which ends the walk as a lack would,
/// its block `MISSED`, which no lack's is. A miss is told by the answer and
/// not by a field: a walk's state that changes in its loop holds no pointer,
/// which naga takes no struct holding.
pub(crate) struct Reading<'a> {
    pub(crate) seen: &'a Seen,
}

/// The block a miss ends a walk on.
pub(crate) const MISSED: [u32; 2] = [observed::NONE, observed::NONE];

impl Query for Reading<'_> {
    type At = u32;

    fn root(&mut self, node: [i32; 3]) -> u32 {
        if observed::keeps(node) {
            observed::octant(node)
        } else {
            observed::NONE
        }
    }

    fn child(&mut self, parent: &u32, _: u32, node: [i32; 3]) -> u32 {
        if *parent == observed::NONE {
            return observed::NONE;
        }
        let c = seen_at(
            self.seen,
            observed::entry_at(*parent) + observed::W_CHILDREN,
        );
        if c == 0 || c == observed::CLAIMED {
            observed::NONE
        } else {
            observed::child(c - 1, observed::octant(node))
        }
    }

    fn node(&mut self, e: &mut u32, _: u32, _: [i32; 3], asked: [u32; 2], _: f32) -> In<[u32; 2]> {
        if *e != observed::NONE {
            let w = observed::entry_at(*e);
            if observed::holds(seen_at(self.seen, w), asked) {
                let kind = observed::kind_of(seen_at(self.seen, w + observed::W_KIND));
                return In {
                    kind,
                    block: [
                        seen_at(self.seen, w + observed::W_BLOCK),
                        seen_at(self.seen, w + observed::W_BLOCK + 1),
                    ],
                };
            }
        }
        In {
            kind: pxl_spec_core::answer::Kind::Lacks,
            block: MISSED,
        }
    }
}

/// Where a writing walk is: the node's entry, `observed::NONE` where none
/// keeps it, and what was handed it.
#[derive(Clone, Copy)]
pub(crate) struct Writ {
    pub(crate) entry: u32,
    pub(crate) at: At,
}

/// The observed pyramid as a beam ray's walk, or a pixel's that missed,
/// reads it: a node it keeps at the zooms asked, as its entry holds it,
/// unless this pass is still writing it (`tag`); any other asked of the
/// world's one query there and then, from what was handed it, and written.
/// A node's children are taken a tile once, by whichever walk swaps it in
/// first, off the tiles' stack.
pub(crate) struct Writing<'a, L> {
    pub(crate) world: World<'a, L>,
    pub(crate) seen: &'a mut Seen,
    pub(crate) tag: u32,
}

impl<L: Looks> Writing<'_, L> {
    /// The entry of `parent`'s child `oct`, its tile taken where it has
    /// none: claimed first by one compare-and-swap, so one walk of the many
    /// that reach a node at once takes a tile, and the rest walk on under
    /// it with none kept; none where no tile is left, one taken too many
    /// given back at once, as no tile is given back while walks take them.
    fn tile(&mut self, parent: u32, oct: u32) -> u32 {
        let w = observed::entry_at(parent) + observed::W_CHILDREN;
        let c = seen_load(self.seen, w);
        if c == observed::CLAIMED {
            return observed::NONE;
        }
        if c != 0 {
            return observed::child(c - 1, oct);
        }
        let was = seen_swap(self.seen, w, 0, observed::CLAIMED);
        if was != 0 {
            return if was == observed::CLAIMED {
                observed::NONE
            } else {
                observed::child(was - 1, oct)
            };
        }
        let k = seen_add(self.seen, observed::TAKEN, 1);
        if k >= observed::TILES {
            seen_add(self.seen, observed::TAKEN, 1u32.wrapping_neg());
            seen_store(self.seen, w, 0);
            return observed::NONE;
        }
        let t = seen_at(self.seen, observed::STACK_AT + k);
        seen_store(self.seen, w, t + 1);
        observed::child(t, oct)
    }
}

impl<L: Looks> Query for Writing<'_, L> {
    type At = Writ;

    fn root(&mut self, node: [i32; 3]) -> Writ {
        let entry = if observed::keeps(node) {
            observed::octant(node)
        } else {
            observed::NONE
        };
        Writ {
            entry,
            at: Query::root(&mut self.world, node),
        }
    }

    fn child(&mut self, parent: &Writ, zoom: u32, node: [i32; 3]) -> Writ {
        let entry = if parent.entry == observed::NONE {
            observed::NONE
        } else {
            self.tile(parent.entry, observed::octant(node))
        };
        Writ {
            entry,
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
        let w = observed::entry_at(n.entry);
        if n.entry != observed::NONE {
            let key = seen_load(self.seen, w);
            if observed::holds(key, asked) && observed::tag_of(key) != self.tag {
                let a = w + observed::W_AT;
                n.at = At::of_words([
                    seen_at(self.seen, a),
                    seen_at(self.seen, a + 1),
                    seen_at(self.seen, a + 2),
                    seen_at(self.seen, a + 3),
                    seen_at(self.seen, a + 4),
                ]);
                let kind = observed::kind_of(seen_at(self.seen, w + observed::W_KIND));
                return In {
                    kind,
                    block: [
                        seen_at(self.seen, w + observed::W_BLOCK),
                        seen_at(self.seen, w + observed::W_BLOCK + 1),
                    ],
                };
            }
        }
        let answer = Query::node(&mut self.world, &mut n.at, zoom, c, asked, t0);
        if n.entry != observed::NONE {
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
            seen_put(self.seen, w + observed::W_NODE, zoom);
            seen_put(self.seen, w + observed::W_NODE + 1, c[0] as u32);
            seen_put(self.seen, w + observed::W_NODE + 2, c[1] as u32);
            seen_store(self.seen, w, observed::key(self.tag, asked));
        }
        answer
    }
}

/// What the observed pyramid kept forgotten, a thread an entry, where what
/// the world's query reads changed (pxl-spec-core `observed::FORGETS`): every
/// entry's key and its children's tile, every tile on the stack in turn; or
/// the key of each entry over a square that moved (`observed::forgets`), its
/// children kept for the walk that asks it again, and its children's tile
/// given back once that keeps nothing. This dispatch only clears words, so
/// a child read before its own thread cleared it keeps its tile a
/// forgetting longer, and one read cleared is cleared.
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
        seen_put(seen, w + observed::W_CHILDREN, 0);
        if id.x < observed::TILES {
            seen_put(seen, observed::STACK_AT + id.x, id.x);
        }
        return;
    }
    if seen_load(seen, w + observed::W_KEY) != 0 {
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
            seen_store(seen, w + observed::W_KEY, 0);
        }
    }
    let c = seen_load(seen, w + observed::W_CHILDREN);
    if c == 0 {
        return;
    }
    let mut kept = false;
    let mut oct = 0;
    while oct < 8 {
        let e = observed::entry_at(observed::child(c - 1, oct));
        kept = kept
            || seen_load(seen, e + observed::W_KEY) != 0
            || seen_load(seen, e + observed::W_CHILDREN) != 0;
        oct += 1;
    }
    if !kept {
        seen_store(seen, w + observed::W_CHILDREN, 0);
        // One fewer taken: an add of -1, as the count's word is unsigned.
        let k = seen_add(seen, observed::TAKEN, 1u32.wrapping_neg());
        seen_put(seen, observed::STACK_AT + k - 1, c - 1);
    }
}
