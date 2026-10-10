// pxl-spec/bake/pack.rs

//! A ground cell's four channels packed into one `u16` (fourteen bits used).
//! A field holds a dense index, not the catalog's number; `ground_pack`'s
//! generated tables map between them, so a new type widens its own field
//! with nothing here edited.

use crate::cell::cube::ground_pack;

use super::format::{GroundChannelKey, GROUND_CH_KEYS};

pub type GroundCell = u16;

const _: () = assert!(
    ground_pack::CELL_BITS <= GroundCell::BITS,
    "a ground cell no longer fits its word: a channel grew past the room the pack has for it"
);

/// The index `ground_pack`'s rows use for `key`: the key's own number.
const fn field_of(key: GroundChannelKey) -> usize {
    key as usize
}

/// The catalog number channel `key` holds; 0 where unpainted.
pub fn channel_of(cell: GroundCell, key: GroundChannelKey) -> u8 {
    let f = field_of(key);
    let mask = (1u32 << ground_pack::BITS[f]) - 1;
    let index = (cell as u32 >> ground_pack::SHIFT[f]) & mask;
    // A word the pack did not write can index past the table; it reads as UNKNOWN.
    let row = ground_pack::NUMBER[f];
    row[(index as usize).min(row.len() - 1)]
}

/// Channel `key` set to `number`; a number the channel lacks becomes UNKNOWN.
pub fn with_channel(cell: GroundCell, key: GroundChannelKey, number: u8) -> GroundCell {
    let f = field_of(key);
    let mask = ((1u32 << ground_pack::BITS[f]) - 1) as GroundCell;
    let index = ground_pack::INDEX[f][number as usize] as GroundCell;
    (cell & !(mask << ground_pack::SHIFT[f])) | (index << ground_pack::SHIFT[f])
}

/// In `GROUND_CH_KEYS`' order.
pub fn pack(numbers: [u8; 4]) -> GroundCell {
    let mut cell = 0;
    for (key, number) in GROUND_CH_KEYS.into_iter().zip(numbers) {
        cell = with_channel(cell, key, number);
    }
    cell
}

/// `pack`'s inverse.
pub fn unpack(cell: GroundCell) -> [u8; 4] {
    GROUND_CH_KEYS.map(|key| channel_of(cell, key))
}

#[cfg(test)]
#[path = "pack.test.rs"]
mod test;
