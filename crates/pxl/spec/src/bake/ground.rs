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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShownGround {
    pub channel: GroundChannelKey,
    pub type_id: u32,
}

/// What a cell shows when no channel has painted it, the catalog's `base`
/// (cube_type.json): a cell with nothing on it is still ground.
pub const BASE: ShownGround = ShownGround {
    channel: match GroundChannelKey::from_name(cube_types::base::CHANNEL) {
        Some(channel) => channel,
        None => panic!("the catalog's base names no ground channel"),
    },
    type_id: cube_types::base::TYPE as u32,
};

/// A set of ground channels, a bit a channel by its identity (`key as u8`).
pub type ChannelMask = u8;

pub const fn channel_bit(key: GroundChannelKey) -> ChannelMask {
    1 << key as u8
}

/// `read` returns a channel's raw baked type id (0 = unpainted). A channel in
/// `hidden` is stepped over as though it were unpainted, so a cell whose
/// building is hidden shows what is under it — the same answer the paint
/// gives with the same channels hidden.
pub fn shown_ground(read: impl Fn(GroundChannelKey) -> u32, hidden: ChannelMask) -> ShownGround {
    let mut shown = BASE;
    for channel in GROUND_CH_KEYS {
        if hidden & channel_bit(channel) != 0 {
            continue;
        }
        let type_id = read(channel);
        if type_id > 0 {
            shown = ShownGround { channel, type_id };
        }
    }
    shown
}

/// How far below its sampled height a cell showing `shown` sits, in units:
/// the catalog's `recess` column, which is how water meets land a step lower
/// rather than at the same level. A column of the type, so it is the same
/// whichever channel the type came from — the sea is a land type and sits
/// where a river does.
pub fn recess(shown: ShownGround) -> i16 {
    let column = match shown.channel {
        GroundChannelKey::LAND => cube_types::land::RECESS,
        GroundChannelKey::WATER => cube_types::water::RECESS,
        GroundChannelKey::STRUCTURE => cube_types::structure::RECESS,
        GroundChannelKey::TRANSPORT => cube_types::transport::RECESS,
    };
    column.iter().find(|(t, _)| *t as u32 == shown.type_id).map_or(0, |&(_, r)| r)
}

#[cfg(test)]
#[path = "ground.test.rs"]
mod test;
