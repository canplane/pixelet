// pxl-shader/pyramid.rs

//! The observed pyramid (pxl-engine `pyramid`): what the cast's walks
//! kept of the world's query, read and written by them; pxl-render
//! forget.wgsl forgets it where what the query reads changed.

use spirv_std::memory::{Scope, Semantics};

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
