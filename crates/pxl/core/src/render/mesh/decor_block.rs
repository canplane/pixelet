// pxl_core/render/mesh/decor_block.rs

//! A chunk's decor instances as lanes, and the block the scheduler hands them
//! over in.
//!
//! What an instance is: a chunk-local position, a spin about Z, a scale, and
//! where it came from — a user-placed entry in object/store.rs, or a procedural
//! scatter that no store names. The id is not stored: a placed entry is named
//! by its band and packed position in the store's own lanes, two u32s, and the
//! string id is rebuilt from those only when a hit needs it (object/store.rs's
//! entry_id_of). `hidden` is a flag lane rather than a separate list: the vertex
//! shader sends a hidden instance outside the clip volume (decor.wgsl) while the
//! hit test keeps seeing it.
//!
//! The scheduler builds a chunk's decor at one tier as one block of the mesh
//! section (crates/pxl/core/src/world/decor.rs): every group's lanes — a group
//! per (kind, variant) in the order the kinds were taught — laid out here.

pub use crate::world::decor::{HIDDEN_FLAG, NO_ENTRY};

/// One group's lanes: `len` instances, each lane's byte offset from the block's
/// own start.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DecorInstances {
    pub len: u32,
    pub x: u32,
    pub y: u32,
    pub z: u32,
    pub rotation_z: u32,
    pub scale: u32,
    /// Bit 0: hidden (set_virtual_hide is suppressing it).
    pub flags: u32,
    /// The store entry this instance is, as object/store.rs's own (band, packed
    /// position); NO_ENTRY in `band` for a procedural instance.
    pub band: u32,
    pub pos: u32,
}

/// Reads a block's header and lays offsets over its lanes. The layout is
/// decor.rs's: the group count, a count per group, then per group five f32
/// lanes, the flags padded to a word, the bands and the positions. None where a
/// group has no instance.
pub fn decor_block_carve(words: &[u32]) -> Vec<Option<DecorInstances>> {
    let Some(&group_cnt) = words.first() else { return Vec::new() };
    let mut groups = Vec::with_capacity(group_cnt as usize);
    let mut k = 1 + group_cnt;
    for g in 0..group_cnt {
        let n = words.get(1 + g as usize).copied().unwrap_or(0);
        if n == 0 {
            groups.push(None);
            continue;
        }
        let at = k * 4;
        let lane = |i: u32| at + i * n * 4;
        let padded = n.div_ceil(4);
        groups.push(Some(DecorInstances {
            len: n,
            x: lane(0),
            y: lane(1),
            z: lane(2),
            rotation_z: lane(3),
            scale: lane(4),
            flags: at + 5 * n * 4,
            band: at + (5 * n + padded) * 4,
            pos: at + (6 * n + padded) * 4,
        }));
        k += 7 * n + padded;
    }
    groups
}
