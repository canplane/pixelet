// pxl-spec/bake/ground.rs

//! Which of a cell's four ground channels shows. Later channels paint over
//! earlier ones, and every consumer must agree: a road over bare ground reads
//! `land = BARE`, so comparing one channel would mistake it for bare ground.

use crate::cell::cube::cube_types;

use super::pack::{channel_of, GroundCell};

pub use super::format::{GroundChannelKey, GROUND_CH_KEYS};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShownGround {
    pub channel: GroundChannelKey,
    pub type_id: u32,
}

/// What an unpainted cell shows: the catalog's `base`.
pub const BASE: ShownGround = ShownGround {
    channel: cube_types::base::CHANNEL,
    type_id: cube_types::base::TYPE as u32,
};

/// A bit per channel, by its identity.
pub type ChannelMask = u8;

pub const fn channel_bit(key: GroundChannelKey) -> ChannelMask {
    1 << key as u8
}

/// `read` gives a channel's raw type id (0 = unpainted). A channel in `hidden`
/// is skipped, matching what the paint shows with it hidden.
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

/// Each channel's `shift` column, by the channel's number.
const SHIFT_COLUMNS: [&[(u8, i16)]; 4] = {
    let mut columns: [&[(u8, i16)]; 4] = [&[]; 4];
    columns[GroundChannelKey::LAND as usize] = cube_types::land::SHIFT;
    columns[GroundChannelKey::WATER as usize] = cube_types::water::SHIFT;
    columns[GroundChannelKey::STRUCTURE as usize] = cube_types::structure::SHIFT;
    columns[GroundChannelKey::TRANSPORT as usize] = cube_types::transport::SHIFT;
    columns
};

/// A raise or a sink that sets one layer's cells apart from the ground
/// around them, up positive: the catalog's `shift` column, by type.
pub fn shift(shown: ShownGround) -> i16 {
    SHIFT_COLUMNS[shown.channel as usize].iter().find(|(t, _)| *t as u32 == shown.type_id).map_or(0, |&(_, r)| r)
}

/// In cells.
pub const SHIFT_RANGE: [i16; 2] = {
    let columns = SHIFT_COLUMNS;
    let (mut lo, mut hi) = (0, 0);
    let mut c = 0;
    while c < columns.len() {
        let mut i = 0;
        while i < columns[c].len() {
            let r = columns[c][i].1;
            if r < lo {
                lo = r;
            }
            if r > hi {
                hi = r;
            }
            i += 1;
        }
        c += 1;
    }
    [lo, hi]
};

/// `shift` with no channel hidden: the height every reader takes.
pub fn cell_shift(cell: GroundCell) -> i16 {
    shift(shown_ground(|k| channel_of(cell, k) as u32, 0))
}
