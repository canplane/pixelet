// pxl-shader/lib.rs

//! The GPU's compute entries, a module of their own each when built, over
//! the world as the GPU holds it (world.rs) and the eye it is cast from
//! (eye.rs): the eye's rays (beam.rs, cast.rs) through the observed pyramid
//! (observed.rs), the world's depth from the sun (sun.rs), each handed
//! square's ground (ground.rs) and ways (ways.rs), and the decor the ground
//! grows (decor.rs). An entry is named by `entry_point_name`, never by its
//! module's path, so each keeps the name pxl-tool shader.rs `ENTRIES` reads.

#![cfg_attr(target_arch = "spirv", no_std)]
// The workspace's clippy never builds this crate: its own build refuses a
// warning, as `-D warnings` does every other.
#![deny(warnings)]

mod beam;
mod cast;
mod decor;
mod eye;
mod ground;
mod observed;
mod sun;
mod ways;
mod world;

pub use beam::{beam_cs, cells_cs, sample_cs};
pub use cast::{cast_cs, miss_cs, oracle_cs};
pub use decor::{decor_args_cs, decor_count_cs, decor_cs, decor_mark_cs, decor_place_cs};
pub use eye::Eye;
pub use ground::{bound_cs, heights_cs, joined_cs};
pub use observed::forget_cs;
pub use sun::{sun_cs, sun_mark_cs, SunMoved};
pub use ways::{ways_cs, ways_free_cs, ways_freed_cs, ways_tree_cs};
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
