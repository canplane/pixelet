// pxl-shader/lib.rs

//! The GPU's compute entries, a module of their own each when built, over
//! the world as the GPU holds it (world.rs) and the eye it is cast from
//! (eye.rs): the passes a cast takes, written once over the observer it
//! casts for (observer.rs), a pass both observers take one entry
//! (either.rs), the eye's rays (beam.rs, raster.rs, cast.rs) through the
//! observed pyramid (observed.rs), the world's depth from the sun (sun.rs),
//! each handed square's ground (ground.rs) and ways (way.rs), and the decor
//! the ground grows (decor.rs). An entry is named
//! by `entry_point_name`, never by its module's path, so each keeps the
//! name pxl-tool shader.rs `ENTRIES` reads.

#![cfg_attr(target_arch = "spirv", no_std)]
// The workspace's clippy never builds this crate: its own build refuses a
// warning, as `-D warnings` does every other.
#![deny(warnings)]

mod beam;
mod cast;
mod decor;
mod either;
mod eye;
mod ground;
mod observed;
mod observer;
mod raster;
mod sun;
mod way;
mod world;

pub use beam::{beam_cs, cell_cs, origin_cs, sample_cs};
pub use cast::{cast_cs, miss_cs, miss_list_cs};
pub use decor::{
    decor_args_cs, decor_count_cs, decor_cs, decor_mark_cs, decor_place_cs, decor_stand_cs,
};
pub use eye::Eye;
pub use ground::{bound_cs, height_cs, joined_cs};
pub use observed::forget_cs;
pub use raster::{face_cs, sample_box_cs};
pub use sun::{sun_cs, sun_mark_cs, sun_mark_drawn_cs, sun_miss_cs, sun_reach_cs, Moved};
pub use way::{way_cs, way_free_cs, way_freed_cs, way_tree_cs};
pub use world::{Looks, Unlooked, World};

use spirv_std::memory::{Scope, Semantics};

/// An atomic add of `v` on word `i` of `s`, its old value; none past `s`.
/// Bounded here, not by an index that panics: a panic's path is an early
/// return, which rust-gpu carries out of every loop about it as a flag a
/// lane's own, and no barrier after such a loop is in uniform control flow.
fn add(s: &mut [u32], i: u32, v: u32) -> u32 {
    if (i as usize) < s.len() {
        // SAFETY: just bounded by the slice's own length.
        unsafe {
            spirv_std::arch::atomic_i_add::<
                u32,
                { Scope::QueueFamily as u32 },
                { Semantics::NONE.bits() },
            >(s.get_unchecked_mut(i as usize), v)
        }
    } else {
        0
    }
}

/// Word `i` of `s` raised to `v` where it is less, none past `s`.
fn most(s: &mut [u32], i: u32, v: u32) {
    if (i as usize) < s.len() {
        // SAFETY: just bounded by the slice's own length.
        unsafe {
            spirv_std::arch::atomic_u_max::<
                u32,
                { Scope::QueueFamily as u32 },
                { Semantics::NONE.bits() },
            >(s.get_unchecked_mut(i as usize), v)
        };
    }
}
