// pxl-shader/ground.rs

//! Each handed square's ground, filled once as it is handed: its columns'
//! tops off its tile's samples (`heights_cs`) and its levels (`bound_cs`),
//! which the CPU reads back; and the eye's windows' levels folded over
//! their squares (`joined_cs`).

use pxl_spec_core::fill;
use pxl_spec_core::window::{self, FIRST_CELL, SQUARE};
use pxl_spec_core::{at, ways};
use spirv_std::glam::{IVec4, UVec3, UVec4};
use spirv_std::spirv;

use crate::eye::Eye;
use crate::world::{square_at, Unlooked, World};

/// The eye's windows folded over their squares, a thread an entry
/// (pxl-spec-core `window::fold`), each a row of the windows' buffer past
/// their slot table (`window::JOINED_AT`): each window's levels over a
/// square, the join of its aligned blocks of squares, which a node wider
/// than a square reads as one entry (`window::bound_over`), a level a dispatch
/// off the one under it. Run where the windows' slots moved; a present
/// square's levels stand while it does.
#[spirv(compute(threads(64, 1, 1), entry_point_name = "joined_cs"))]
#[allow(clippy::too_many_arguments)]
pub fn joined_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(num_workgroups)] groups: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] window: &mut [IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] ground: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 5)] shards: &[IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 6)] overwrites: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 7)] pool: &[u32],
) {
    // The level this dispatch folds is its depth in workgroups, its last
    // slice the one that runs: the levels under it were folded before.
    let (l, blocks) = (
        groups.z,
        (window::ACROSS >> groups.z) * (window::ACROSS >> groups.z),
    );
    if id.z + 1 != l || id.x >= blocks * window::EYE_WINDOWS {
        return;
    }
    let (cell, b) = (FIRST_CELL + id.x / blocks, id.x % blocks);
    let [x, y, bound, pad] = {
        let read: &[IVec4] = window;
        let world = World {
            window: read,
            ground,
            looks: Unlooked(0),
            shards,
            overwrites,
            eye,
            pool: ways::tree::Read(pool),
        };
        window::fold(&world, cell, l, b)
    };
    let k = (window::JOINED_AT
        + (cell - FIRST_CELL) * window::JOINED_ENTRIES
        + window::joined_index(l, [0, 0])
        + b) as usize;
    if k < window.len() {
        window[k] = IVec4::new(x as i32, y as i32, bound as i32, pad as i32);
    }
}

/// The columns' ground tops of the squares `fresh` names, the squares
/// handed their tiles' samples this turn, one a `z`, each a record as
/// handed (`window::HANDED_WORDS`); a thread a column (`ground::height`)
/// off the samples in it, an even column's laying its cell and the next
/// (`fill::heights`); and its cell's four looks into their own buffer
/// (`fill::look`), which the cast draws by.
#[spirv(compute(threads(8, 8, 1), entry_point_name = "heights_cs"))]
pub fn heights_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 0)] fresh: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] ground: &mut [u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] looks: &mut [u32],
) {
    if id.x >= SQUARE || id.y >= SQUARE {
        return;
    }
    fill::heights(ground, fresh, id.z, [id.x, id.y]);
    fill::look(looks, fresh, id.z, [id.x, id.y]);
}

/// Level `level.x` of the squares `fresh` names, one a `z`: level 0 each
/// column's own, its top and the spans kept in its scratch (`fill::level`),
/// a higher one over the level under it, written before (`window::join`): a
/// dispatch a level, from 0 up.
#[spirv(compute(threads(8, 8, 1), entry_point_name = "bound_cs"))]
pub fn bound_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] level: &UVec4,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] fresh: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] ground: &mut [u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 3)] scratch: &[u32],
) {
    let j = level.x;
    let side = SQUARE >> j;
    if id.x >= side || id.y >= side {
        return;
    }
    let rec = window::handed_at(id.z);
    let base = square_at(at(fresh, rec + window::FRESH_SQUARE));
    let kept = fill::Scratch {
        words: scratch,
        at: fill::scratch_at(scratch, id.z),
    };
    fill::level(
        ground,
        base,
        &kept,
        at(fresh, rec + window::FRESH_CELL),
        j,
        [id.x, id.y],
    );
}
