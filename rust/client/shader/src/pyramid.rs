// pxl-shader/pyramid.rs

//! The observed pyramid (pxl-engine `pyramid`): what the cast's walks
//! kept of the world's query, read and written by them, and forgotten where
//! what the query reads changed (`forget_cs`).

use pxl_engine::pyramid;
use spirv_std::glam::UVec3;
use spirv_std::memory::{Scope, Semantics};
use spirv_std::spirv;

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
