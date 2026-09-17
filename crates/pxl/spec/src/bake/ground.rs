// pxl-spec/bake/ground.rs

//! Which of a cell's four ground channels the viewer actually sees.
//!
//! A baked cell carries land/water/structure/transport at once, and later
//! channels paint over earlier ones. Every consumer that asks "what is this
//! cell" has to agree on that precedence or they answer differently about
//! the same cell: the terrain shader picks a texture layer from it, and the
//! object layer asks whether a placed terrain-styled cube would show
//! anything the terrain does not already show. A road over bare ground reads
//! `land = BARE` while rendering as road, so comparing one channel would
//! cancel a bare-ground cube into a road.

use crate::cell::cube::cube_types;

pub use super::format::{GroundChannelKey, GROUND_CH_KEYS};

/// Land with nothing painted still renders as bare ground rather than as
/// nothing, so LAND alone has a fallback the other channels do not.
pub const LAND_BASE_TYPE: u32 = cube_types::land::bare as u32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShownGround {
    pub channel: GroundChannelKey,
    pub type_id: u32,
}

/// `read` returns a channel's raw baked type id (0 = unpainted). STRUCTURE
/// is skipped unless shown, matching the renderer's own dev toggle — a cell
/// whose building is hidden shows what is under it.
pub fn shown_ground(read: impl Fn(GroundChannelKey) -> u32, show_structures: bool) -> ShownGround {
    let mut shown = ShownGround {
        channel: GroundChannelKey::LAND,
        type_id: LAND_BASE_TYPE,
    };
    for channel in GROUND_CH_KEYS {
        if channel == GroundChannelKey::STRUCTURE && !show_structures {
            continue;
        }
        let type_id = read(channel);
        if type_id > 0 {
            shown = ShownGround { channel, type_id };
        }
    }
    shown
}

#[cfg(test)]
#[path = "ground.test.rs"]
mod test;
