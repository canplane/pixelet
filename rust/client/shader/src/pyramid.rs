// pxl-shader/pyramid.rs

//! The observed pyramid (pxl-engine `pyramid`): what the cast's walks
//! kept of the world's query, read and written by them, and forgotten where
//! what the query reads changed (`forget_cs`).

use pxl_engine::answer::In;
use pxl_engine::cast::Query;
use pxl_engine::pyramid;
use pxl_engine::world::Inheritance;
use spirv_std::glam::UVec3;
use spirv_std::memory::{Scope, Semantics};
use spirv_std::spirv;

use crate::world::{PaletteWords, World};

/// The observed pyramid's buffer (pxl-engine `pyramid`), bound as a
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
/// `lane` of the head where the walks count (pxl-engine
/// `pyramid::TALLY_ON`, `tally`).
pub(crate) fn add_tally(w: &mut Pyramid, observer: u32, lane: u32, visits: u32) {
    if pyramid_load(w, pyramid::TALLY_ON) != 0 {
        // SAFETY: as `pyramid_at`.
        unsafe {
            spirv_std::arch::atomic_i_add::<
                u32,
                { Scope::QueueFamily as u32 },
                { Semantics::NONE.bits() },
            >(
                w.index_mut(pyramid::tally_at(observer, lane) as usize),
                visits,
            );
        }
    }
}

/// Whether this walk claims the slot whose key is word `i`, the key `was`
/// when it read it: the key exchanged for `CLAIMED`, the claim this walk's
/// where it was still `was`; else what it took put back, unless it took
/// another walk's claim. While it is put back the slot reads as claimed, so
/// another walk asks the world there and none claims it, and what it puts
/// back is the slot's latest key. No compare-and-swap: Safari's WGSL
/// compiler writes Metal for one that does not compile
/// (doc/experiment/20261103-shaders-on-two-phones).
fn pyramid_claim(w: &mut Pyramid, i: u32, was: u32) -> bool {
    // SAFETY: as `pyramid_at`.
    let held = unsafe {
        spirv_std::arch::atomic_exchange::<
            u32,
            { Scope::QueueFamily as u32 },
            { Semantics::NONE.bits() },
        >(w.index_mut(i as usize), pyramid::CLAIMED)
    };
    if held != was && held != pyramid::CLAIMED {
        pyramid_store(w, i, held);
    }
    held == was
}

/// The first slot of the bucket that may keep node `c` of `zoom`,
/// `pyramid::NONE` where none does (`pyramid::keeps`).
fn bucket_of(zoom: u32, c: [i32; 3]) -> u32 {
    if pyramid::keeps(zoom, c) {
        pyramid::entry_of(zoom, c)
    } else {
        pyramid::NONE
    }
}

/// Whether entry word `w` is node `c` of `zoom`, as its node words say.
fn is_node(pyramid: &Pyramid, w: u32, zoom: u32, c: [i32; 3]) -> bool {
    let n = w + pyramid::W_NODE;
    pyramid_at(pyramid, n) == zoom
        && pyramid_at(pyramid, n + 1) == c[0] as u32
        && pyramid_at(pyramid, n + 2) == c[1] as u32
        && pyramid_at(pyramid, n + 3) == c[2] as u32
}

/// The observed pyramid as a beam ray's walk, a sample's, or a pixel's that
/// missed, reads it: a node its slot keeps at the zooms asked, unless this
/// pass is still writing it (`tag`), taken where the key still stood once
/// it was read; any other asked of the world's one query there and then,
/// from what was handed it, and written where this walk claims the slot.
/// A walk holds of a node what was handed it alone (`world::Inheritance`), as the
/// world's one query does: a node's slot is its key's (`pyramid::entry_of`),
/// found again wherever the node is asked.
pub(crate) struct Writing<'a, L> {
    pub(crate) world: World<'a, L>,
    pub(crate) pyramid: &'a mut Pyramid,
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
        zooms: [u32; 2],
        t0: f32,
    ) -> In<[u32; 2]> {
        let first = bucket_of(zoom, c);
        // The slot this walk would write, and the key it read there: the
        // node's own, else a free one, else the one it takes over; none where
        // a walk of this pass keeps it.
        let (mut to, mut key) = (pyramid::NONE, pyramid::CLAIMED);
        if first != pyramid::NONE {
            let mut free = pyramid::NONE;
            let mut k = 0;
            while k < pyramid::BUCKET {
                let w = pyramid::entry_at(first + k);
                let held = pyramid_load(self.pyramid, w);
                if held != pyramid::CLAIMED && held != 0 && is_node(self.pyramid, w, zoom, c) {
                    if pyramid::holds(held, zooms) {
                        if pyramid::tag_of(held) == self.tag {
                            // Kept by a walk of this pass: asked of the
                            // world, as its words may be half written, and
                            // left to that walk.
                            to = pyramid::NONE;
                            free = pyramid::NONE;
                            break;
                        }
                        let a = w + pyramid::W_INHERITANCE;
                        let handed = [
                            pyramid_at(self.pyramid, a),
                            pyramid_at(self.pyramid, a + 1),
                            pyramid_at(self.pyramid, a + 2),
                            pyramid_at(self.pyramid, a + 3),
                        ];
                        let word = pyramid_load(self.pyramid, w + pyramid::W_KIND);
                        let asset = [
                            pyramid_at(self.pyramid, w + pyramid::W_ASSET),
                            pyramid_at(self.pyramid, w + pyramid::W_ASSET + 1),
                        ];
                        if pyramid::stamp_holds(word, held, asset, handed, zoom, c) {
                            *n = Inheritance::of_words(handed);
                            return In {
                                kind: pyramid::kind_of(word),
                                asset,
                            };
                        }
                        to = pyramid::NONE;
                        free = pyramid::NONE;
                        break;
                    }
                    // The node, asked at other zooms by an earlier frame:
                    // written over. One this frame asked so (the eye's and
                    // the light's ask the nodes near the eye apart) stands
                    // beside it.
                    if pyramid::tag_of(held) / pyramid::PASSES < self.tag / pyramid::PASSES {
                        (to, key) = (first + k, held);
                    }
                } else if held == 0 && free == pyramid::NONE {
                    free = first + k;
                }
                k += 1;
            }
            if to == pyramid::NONE && free != pyramid::NONE {
                (to, key) = (free, 0);
            } else if to == pyramid::NONE && k == pyramid::BUCKET {
                let v = first + pyramid::victim(zoom, c);
                (to, key) = (v, pyramid_load(self.pyramid, pyramid::entry_at(v)));
            }
        }
        let answer = Query::node(&mut self.world, n, zoom, c, zooms, t0);
        let w = pyramid::entry_at(to);
        if to != pyramid::NONE
            && (self.assets
                || answer.kind != pxl_engine::answer::Kind::Cell
                || pxl_engine::answer::transparent(answer.asset[0]))
            && key != pyramid::CLAIMED
            && pyramid_claim(self.pyramid, w, key)
        {
            let words = n.words();
            let key = pyramid::key(self.tag, zooms);
            pyramid_put(
                self.pyramid,
                w + pyramid::W_KIND,
                pyramid::stamped(answer.kind, key, answer.asset, words, zoom, c),
            );
            pyramid_put(self.pyramid, w + pyramid::W_ASSET, answer.asset[0]);
            pyramid_put(self.pyramid, w + pyramid::W_ASSET + 1, answer.asset[1]);
            let a = w + pyramid::W_INHERITANCE;
            pyramid_put(self.pyramid, a, words[0]);
            pyramid_put(self.pyramid, a + 1, words[1]);
            pyramid_put(self.pyramid, a + 2, words[2]);
            pyramid_put(self.pyramid, a + 3, words[3]);
            let k = w + pyramid::W_NODE;
            pyramid_put(self.pyramid, k, zoom);
            pyramid_put(self.pyramid, k + 1, c[0] as u32);
            pyramid_put(self.pyramid, k + 2, c[1] as u32);
            pyramid_put(self.pyramid, k + 3, c[2] as u32);
            pyramid_store(self.pyramid, w, key);
        }
        answer
    }
}

/// What the observed pyramid kept forgotten, a thread a slot, where what
/// the world's query reads changed (pxl-engine `pyramid::FORGETS`):
/// every slot's key, or the key of each slot over a brick that moved
/// (`pyramid::forgets`), over a box of what is placed or cut that changed
/// (`pyramid::over_box`), or handed a slot of a child group of what is placed
/// that was written (`pyramid::reads_written`).
#[spirv(compute(threads(64, 1, 1), entry_point_name = "forget_cs"))]
pub fn forget_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 9)] pyramid: &mut Pyramid,
) {
    if id.x >= pyramid::ENTRIES {
        return;
    }
    let w = pyramid::entry_at(id.x);
    if pyramid_at(pyramid, pyramid::FORGETS) == pyramid::FORGETS_ALL {
        pyramid_put(pyramid, w + pyramid::W_KEY, 0);
        return;
    }
    if pyramid_at(pyramid, w + pyramid::W_KEY) == 0 {
        return;
    }
    let (zoom, x, y) = (
        pyramid_at(pyramid, w + pyramid::W_NODE),
        pyramid_at(pyramid, w + pyramid::W_NODE + 1) as i32,
        pyramid_at(pyramid, w + pyramid::W_NODE + 2) as i32,
    );
    let mut moved = [0; pyramid::MOVED_WORDS as usize];
    let mut k = 0;
    while k < pyramid::MOVED_WORDS {
        moved[k as usize] = pyramid_at(pyramid, pyramid::MOVED_AT + k);
        k += 1;
    }
    if pyramid::forgets(&moved, zoom, [x, y]) {
        let key = pyramid_at(pyramid, w + pyramid::W_KEY);
        let split = pyramid_at(pyramid, w + pyramid::W_KIND) & pyramid::KIND_MASK
            == pyramid::kind_word(pxl_engine::answer::Kind::Split);
        pyramid_put(
            pyramid,
            w + pyramid::W_KEY,
            if split { pyramid::forgotten_split(key) } else { 0 },
        );
        return;
    }
    // Or handed a slot of a child group of what is placed that was written.
    let placed = pyramid_at(pyramid, w + pyramid::W_PLACED);
    let mut f = 0;
    while f < pyramid_at(pyramid, pyramid::WRITTEN_AT) {
        if pyramid::reads_written(placed, pyramid_at(pyramid, pyramid::WRITTEN_AT + 1 + f)) {
            pyramid_put(pyramid, w + pyramid::W_KEY, 0);
            return;
        }
        f += 1;
    }
    // Or over a box of what is placed or cut that changed.
    let mut b = 0;
    while b < pyramid_at(pyramid, pyramid::BOXES_AT) {
        let at = pyramid::BOXES_AT + 1 + 4 * b;
        let changed = [
            pyramid_at(pyramid, at) as i32,
            pyramid_at(pyramid, at + 1) as i32,
            pyramid_at(pyramid, at + 2) as i32,
            pyramid_at(pyramid, at + 3) as i32,
        ];
        if pyramid::over_box(zoom, [x, y], changed) {
            pyramid_put(pyramid, w + pyramid::W_KEY, 0);
            return;
        }
        b += 1;
    }
}
