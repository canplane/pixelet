// pxl-shader/ground.rs

//! Each handed brick's ground, filled once as it is handed: its columns'
//! tops off its tile's samples (`height_cs`) and its levels (`bound_cs`),
//! which the CPU reads back; and the eye's clip levels' levels folded over
//! their bricks (`fold_cs`).

use pxl_engine::fill;
use pxl_engine::clipmap::{self, BRICK, FIRST_CELL};
use pxl_engine::{at, way};
use spirv_std::glam::{IVec4, UVec3, UVec4};
use spirv_std::spirv;

use crate::eye::Eye;
use crate::world::{brick_at, NoPalette, World};

/// The eye's clip levels folded over their bricks, a thread an entry
/// (pxl-engine `clipmap::fold`), each a row of the clip levels' buffer past
/// their slot table (`clipmap::JOINED_AT`): each clip level's levels over a
/// brick, the join of its aligned blocks of bricks, which a node wider
/// than a brick reads as one entry (`clipmap::bound_over`), a level a dispatch
/// off the one under it. Run where the clip levels' slots moved; a present
/// brick's levels stand while it does.
#[spirv(compute(threads(64, 1, 1), entry_point_name = "fold_cs"))]
#[allow(clippy::too_many_arguments)]
pub fn fold_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(num_workgroups)] groups: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] clipmap: &mut [IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] ground: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 5)] shards: &[IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 6)] overwrites: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 7)] pool: &[u32],
) {
    // The level this dispatch folds is its depth in workgroups, its last
    // slice the one that runs: the levels under it were folded before.
    let (l, blocks) = (
        groups.z,
        (clipmap::ACROSS >> groups.z) * (clipmap::ACROSS >> groups.z),
    );
    if id.z + 1 != l || id.x >= blocks * clipmap::EYE_CLIP_LEVELS {
        return;
    }
    let (cell, b) = (FIRST_CELL + id.x / blocks, id.x % blocks);
    let [x, y, bound, pad] = {
        let read: &[IVec4] = clipmap;
        let world = World {
            clipmap: read,
            ground,
            palette_indices: NoPalette(0),
            shards,
            overwrites,
            eye,
            pool: way::tree::Read(pool),
        };
        clipmap::fold(&world, cell, l, b)
    };
    let k = (clipmap::JOINED_AT
        + (cell - FIRST_CELL) * clipmap::JOINED_ENTRIES
        + clipmap::joined_idx(l, [0, 0])
        + b) as usize;
    if k < clipmap.len() {
        clipmap[k] = IVec4::new(x as i32, y as i32, bound as i32, pad as i32);
    }
}

/// The columns' ground tops of the bricks `fresh` names, the bricks
/// handed their tiles' samples this turn, one a `z`, each a record as
/// handed (`clipmap::HANDED_WORDS`); a thread a column (`ground::height`)
/// off the samples in it, an even column's laying its cell and the next
/// (`fill::heights`); and its cell's four palette indices into their own buffer
/// (`fill::put_palette_word`), which the cast draws by.
#[spirv(compute(threads(8, 8, 1), entry_point_name = "height_cs"))]
pub fn height_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 0)] fresh: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] ground: &mut [u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] palette_indices: &mut [u32],
) {
    if id.x >= BRICK || id.y >= BRICK {
        return;
    }
    fill::heights(ground, fresh, id.z, [id.x, id.y]);
    fill::put_palette_word(palette_indices, fresh, id.z, [id.x, id.y]);
}

/// Level `level.x` of the bricks `fresh` names, one a `z`: level 0 each
/// column's own, its top and the spans kept in its scratch (`fill::level`),
/// a higher one over the level under it, written before (`clipmap::join`): a
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
    let side = BRICK >> j;
    if id.x >= side || id.y >= side {
        return;
    }
    let rec = clipmap::handed_at(id.z);
    let base = brick_at(at(fresh, rec + clipmap::FILL_BRICK));
    let spans = fill::Scratch {
        words: scratch,
        at: fill::scratch_at(scratch, id.z),
    };
    fill::level(
        ground,
        base,
        &spans,
        at(fresh, rec + clipmap::FILL_CELL),
        j,
        [id.x, id.y],
    );
}
