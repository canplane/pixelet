// pxl-shader/observed.rs

//! The observed pyramid (pxl-spec-core `observed`): what the cast's walks
//! kept of the world's query, read and written by them, and forgotten where
//! what the query reads changed (`forget_cs`).

use pxl_spec_core::answer::In;
use pxl_spec_core::cast::Query;
use pxl_spec_core::observed;
use pxl_spec_core::world::Inheritance;
use spirv_std::glam::UVec3;
use spirv_std::memory::{Scope, Semantics};
use spirv_std::spirv;

use crate::world::{PaletteWords, World};

/// The observed pyramid's buffer (pxl-spec-core `observed`), bound as a
/// runtime array, whose pointer a walk holds whole: a slice's would be a
/// pointer and a length, which naga traces no atomic through.
pub(crate) type Pyramid = spirv_std::RuntimeArray<u32>;

/// Word `i` of the observed pyramid's buffer.
pub(crate) fn pyramid_at(w: &Pyramid, i: u32) -> u32 {
    // SAFETY: an entry's or the head's word, within the buffer.
    unsafe { *w.index(i as usize) }
}

pub(crate) fn pyramid_put(w: &mut Pyramid, i: u32, v: u32) {
    // SAFETY: as `pyramid_at`.
    unsafe { *w.index_mut(i as usize) = v }
}

/// `v` added to word `i`, whoever else adds to it: what it held.
pub(crate) fn pyramid_add(w: &mut Pyramid, i: u32, v: u32) -> u32 {
    // SAFETY: as `pyramid_at`.
    unsafe {
        spirv_std::arch::atomic_i_add::<
            u32,
            { Scope::QueueFamily as u32 },
            { Semantics::NONE.bits() },
        >(w.index_mut(i as usize), v)
    }
}

/// Word `i` raised to `v` where it is less, whoever else raises it.
pub(crate) fn pyramid_most(w: &mut Pyramid, i: u32, v: u32) {
    // SAFETY: as `pyramid_at`.
    unsafe {
        spirv_std::arch::atomic_u_max::<
            u32,
            { Scope::QueueFamily as u32 },
            { Semantics::NONE.bits() },
        >(w.index_mut(i as usize), v);
    }
}

/// Word `i` read and written whole, where a walk may race another on it.
fn pyramid_load(w: &mut Pyramid, i: u32) -> u32 {
    // SAFETY: as `pyramid_at`.
    unsafe {
        spirv_std::arch::atomic_load::<u32, { Scope::QueueFamily as u32 }, { Semantics::NONE.bits() }>(
            w.index_mut(i as usize),
        )
    }
}

fn pyramid_store(w: &mut Pyramid, i: u32, v: u32) {
    // SAFETY: as `pyramid_at`.
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
pub(crate) fn add_tally(w: &mut Pyramid, observer: u32, lane: u32, visits: u32) {
    if pyramid_load(w, observed::TALLY_ON) != 0 {
        // SAFETY: as `pyramid_at`.
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
fn pyramid_swap(w: &mut Pyramid, i: u32, was: u32, v: u32) -> u32 {
    // SAFETY: as `pyramid_at`.
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
fn is_node(seen: &Pyramid, w: u32, zoom: u32, c: [i32; 3]) -> bool {
    let n = w + observed::W_NODE;
    pyramid_at(seen, n) == zoom
        && pyramid_at(seen, n + 1) == c[0] as u32
        && pyramid_at(seen, n + 2) == c[1] as u32
        && pyramid_at(seen, n + 3) == c[2] as u32
}

/// The observed pyramid as a beam ray's walk, a sample's, or a pixel's that
/// missed, reads it: a node its slot keeps at the zooms asked, unless this
/// pass is still writing it (`tag`), taken where the key still stood once
/// it was read; any other asked of the world's one query there and then,
/// from what was handed it, and written where this walk claims the slot.
/// A walk holds of a node what was handed it alone (`world::Inheritance`), as the
/// world's one query does: a node's slot is its key's (`observed::entry_of`),
/// found again wherever the node is asked.
pub(crate) struct Writing<'a, L> {
    pub(crate) world: World<'a, L>,
    pub(crate) seen: &'a mut Pyramid,
    pub(crate) tag: u32,
    /// Whether it writes a cell a ray sees, which another walk may then
    /// read (observer.rs `Observer::assets`); splits, what is transparent
    /// and lacks are written by every walk.
    pub(crate) assets: bool,
}

impl<L: PaletteWords> Query for Writing<'_, L> {
    type At = Inheritance;

    fn root(&mut self, node: [i32; 3]) -> Inheritance {
        Query::root(&mut self.world, node)
    }

    fn child(&mut self, parent: &Inheritance, zoom: u32, node: [i32; 3]) -> Inheritance {
        Query::child(&mut self.world, parent, zoom, node)
    }

    fn node(
        &mut self,
        n: &mut Inheritance,
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
                let held = pyramid_load(self.seen, w);
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
                        let a = w + observed::W_INHERITANCE;
                        let at = Inheritance::of_words([
                            pyramid_at(self.seen, a),
                            pyramid_at(self.seen, a + 1),
                            pyramid_at(self.seen, a + 2),
                            pyramid_at(self.seen, a + 3),
                            pyramid_at(self.seen, a + 4),
                        ]);
                        let kind = observed::kind_of(pyramid_at(self.seen, w + observed::W_KIND));
                        let asset = [
                            pyramid_at(self.seen, w + observed::W_ASSET),
                            pyramid_at(self.seen, w + observed::W_ASSET + 1),
                        ];
                        if pyramid_load(self.seen, w) == held {
                            *n = at;
                            return In { kind, asset };
                        }
                        to = observed::NONE;
                        free = observed::NONE;
                        break;
                    }
                    // The node, asked at other zooms by an earlier frame:
                    // written over. One this frame asked so (the eye's and
                    // the light's ask the nodes near the eye apart) stands
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
                (to, key) = (v, pyramid_load(self.seen, observed::entry_at(v)));
            }
        }
        let answer = Query::node(&mut self.world, n, zoom, c, asked, t0);
        let w = observed::entry_at(to);
        if to != observed::NONE
            && (self.assets
                || answer.kind != pxl_spec_core::answer::Kind::Cell
                || pxl_spec_core::answer::transparent(answer.asset[0]))
            && key != observed::CLAIMED
            && pyramid_swap(self.seen, w, key, observed::CLAIMED) == key
        {
            pyramid_put(
                self.seen,
                w + observed::W_KIND,
                observed::kind_word(answer.kind),
            );
            pyramid_put(self.seen, w + observed::W_ASSET, answer.asset[0]);
            pyramid_put(self.seen, w + observed::W_ASSET + 1, answer.asset[1]);
            let words = n.words();
            let a = w + observed::W_INHERITANCE;
            pyramid_put(self.seen, a, words[0]);
            pyramid_put(self.seen, a + 1, words[1]);
            pyramid_put(self.seen, a + 2, words[2]);
            pyramid_put(self.seen, a + 3, words[3]);
            pyramid_put(self.seen, a + 4, words[4]);
            let k = w + observed::W_NODE;
            pyramid_put(self.seen, k, zoom);
            pyramid_put(self.seen, k + 1, c[0] as u32);
            pyramid_put(self.seen, k + 2, c[1] as u32);
            pyramid_put(self.seen, k + 3, c[2] as u32);
            pyramid_store(self.seen, w, observed::key(self.tag, asked));
        }
        answer
    }
}

/// What the observed pyramid kept forgotten, a thread a slot, where what
/// the world's query reads changed (pxl-spec-core `observed::FORGETS`):
/// every slot's key, or the key of each slot over a brick that moved
/// (`observed::forgets`), over a box of what is placed or cut that changed
/// (`observed::over_box`), or handed a slot of a child group of what is placed
/// that was written (`observed::reads_written`).
#[spirv(compute(threads(64, 1, 1), entry_point_name = "forget_cs"))]
pub fn forget_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 9)] seen: &mut Pyramid,
) {
    if id.x >= observed::ENTRIES {
        return;
    }
    let w = observed::entry_at(id.x);
    if pyramid_at(seen, observed::FORGETS) == observed::FORGETS_ALL {
        pyramid_put(seen, w + observed::W_KEY, 0);
        return;
    }
    if pyramid_at(seen, w + observed::W_KEY) == 0 {
        return;
    }
    let (zoom, x, y) = (
        pyramid_at(seen, w + observed::W_NODE),
        pyramid_at(seen, w + observed::W_NODE + 1) as i32,
        pyramid_at(seen, w + observed::W_NODE + 2) as i32,
    );
    let mut moved = [0; observed::MOVED_WORDS as usize];
    let mut k = 0;
    while k < observed::MOVED_WORDS {
        moved[k as usize] = pyramid_at(seen, observed::MOVED_AT + k);
        k += 1;
    }
    if observed::forgets(&moved, zoom, [x, y]) {
        pyramid_put(seen, w + observed::W_KEY, 0);
        return;
    }
    // Or handed a slot of a child group of what is placed that was written.
    let placed = pyramid_at(seen, w + observed::W_PLACED);
    let mut f = 0;
    while f < pyramid_at(seen, observed::WRITTEN_AT) {
        if observed::reads_written(placed, pyramid_at(seen, observed::WRITTEN_AT + 1 + f)) {
            pyramid_put(seen, w + observed::W_KEY, 0);
            return;
        }
        f += 1;
    }
    // Or over a box of what is placed or cut that changed.
    let mut b = 0;
    while b < pyramid_at(seen, observed::BOXES_AT) {
        let at = observed::BOXES_AT + 1 + 4 * b;
        let changed = [
            pyramid_at(seen, at) as i32,
            pyramid_at(seen, at + 1) as i32,
            pyramid_at(seen, at + 2) as i32,
            pyramid_at(seen, at + 3) as i32,
        ];
        if observed::over_box(zoom, [x, y], changed) {
            pyramid_put(seen, w + observed::W_KEY, 0);
            return;
        }
        b += 1;
    }
}
