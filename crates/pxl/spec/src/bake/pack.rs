// pxl-spec/bake/pack.rs

//! A ground cell's four channels in one word.
//!
//! A page held its ground as four planes, a byte a cell each, and what those
//! four bytes carry is fourteen bits: land names one of sixteen, water one of
//! five, structure one of seven and transport one of twelve, counting the
//! VOID a channel nothing painted shows and the UNKNOWN it shows where the
//! source data was missing. Packed into a `u16` the page halves, and nothing
//! above this file changes — the four channels are still four, each still
//! answerable on its own, which is what a map's layers need
//! (issue/20260923-a-ground-cell-is-fourteen-bits.md).
//!
//! A field holds a **dense index**, not the catalog's number. The catalog
//! numbers an identity rather than a sequence — `land::sea` is 129 among
//! fourteen land types — because a placed cube names one of those numbers in
//! the object store, where a browser is already holding it. `ground_pack`'s
//! tables turn one into the other, and they are generated beside the catalog
//! they come from, widths included, so a new type widens its own field and
//! moves the fields above it with nothing here edited.

use crate::cell::cube::ground_pack;

use super::format::{const_str_eq, GroundChannelKey, GROUND_CH_KEYS};

/// A ground cell, its four channels packed.
pub type GroundCell = u16;

const _: () = assert!(
    ground_pack::CELL_BITS <= GroundCell::BITS,
    "a ground cell no longer fits its word: a channel grew past the room the pack has for it"
);

/// Where `key`'s field sits among the packed ones, which is where
/// `ground_pack`'s rows are indexed from.
const fn field_of(key: GroundChannelKey) -> usize {
    let mut i = 0;
    while i < ground_pack::CHANNELS.len() {
        if const_str_eq(ground_pack::CHANNELS[i], key.name()) {
            return i;
        }
        i += 1;
    }
    panic!("a ground channel with no field in the packed cell")
}

/// The catalog number channel `key` holds in `cell`, as the four planes held
/// it: 0 where nothing painted the cell.
pub fn channel_of(cell: GroundCell, key: GroundChannelKey) -> u8 {
    let f = field_of(key);
    let mask = (1u32 << ground_pack::BITS[f]) - 1;
    let index = (cell as u32 >> ground_pack::SHIFT[f]) & mask;
    // A word the pack did not write can name an index past the table; the
    // catalog's own UNKNOWN is what that reads as, as an unknown byte does.
    let row = ground_pack::NUMBER[f];
    row[(index as usize).min(row.len() - 1)]
}

/// Channel `key` set to the catalog number `number`, the other fields left
/// alone. A number the channel does not give out is UNKNOWN, which is what an
/// unreadable byte in a plane already meant.
pub fn with_channel(cell: GroundCell, key: GroundChannelKey, number: u8) -> GroundCell {
    let f = field_of(key);
    let mask = ((1u32 << ground_pack::BITS[f]) - 1) as GroundCell;
    let index = ground_pack::INDEX[f][number as usize] as GroundCell;
    (cell & !(mask << ground_pack::SHIFT[f])) | (index << ground_pack::SHIFT[f])
}

/// A cell from the four catalog numbers a page's planes held, in
/// `GROUND_CH_KEYS`' order.
pub fn pack(numbers: [u8; 4]) -> GroundCell {
    let mut cell = 0;
    for (key, number) in GROUND_CH_KEYS.into_iter().zip(numbers) {
        cell = with_channel(cell, key, number);
    }
    cell
}

/// The four catalog numbers a cell holds, in `GROUND_CH_KEYS`' order: the
/// reverse of `pack`.
pub fn unpack(cell: GroundCell) -> [u8; 4] {
    GROUND_CH_KEYS.map(|key| channel_of(cell, key))
}

#[cfg(test)]
#[path = "pack.test.rs"]
mod test;
