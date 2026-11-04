// pxl-shader/lib.rs

//! The GPU's compute entries rust-gpu still makes, a module of their own each
//! when built, over the eye they are cast from (eye.rs): the cells' starts,
//! written once over the observer it casts for (observer.rs, beam.rs), the
//! missed pixels' hits (cast.rs) through the observed pyramid (pyramid.rs),
//! their image bound only where rays.rs lets it be, and the light's head
//! (light.rs). An entry is named by `entry_point_name`, never by its module's
//! path, so each keeps the name pxl-tool shader.rs `ENTRIES` reads.

#![cfg_attr(target_arch = "spirv", no_std)]
// The workspace's clippy never builds this crate.
#![deny(warnings)]

mod beam;
mod rays;
mod eye;
mod pyramid;
mod observer;
mod light;

use rays::cast;

pub use beam::cell_cs;
pub use cast::{hit_faces_cs, hit_lead_cs};
pub use eye::Eye;
pub use light::Head;

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

/// Word `i` of `w`, a dispatch's workgroups of `per` lanes over a list,
/// raised to hold place `at` by the place that opens a workgroup alone:
/// a list's places are taken one by one from 0 (`add`), so
/// each workgroup's first place raises it and the rest leave it as it is.
fn opens<W: beam::Words + ?Sized>(w: &mut W, i: u32, at: u32, per: u32) {
    if at % per == 0 {
        w.most(i, at / per + 1);
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
