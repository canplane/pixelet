// pxl-spec/bake/tile.rs

//! A baked tile of each layer — terrain, place, division and the Earth's
//! picture — and what every kind's says alike: which of it and its children
//! are tiles.

use xpute_kit::status::error::MarshalError;

use crate::bake::malformed;

pub mod division;
pub mod earth;
pub mod place;
pub mod terrain;

/// Which of a tile and its four children are tiles of their own, the rest
/// said whole by the tile (terrain's zero tile, places's uncrowded ground):
/// children in the low four bits in `slot_of`'s order, the tile itself above
/// them. A client never asks for a child its parent marks absent.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Occupancy(u8);

impl Occupancy {
    const TILE: u8 = 1 << 4;

    pub const ZERO: Occupancy = Occupancy(0);

    pub const LEAF: Occupancy = Occupancy(Occupancy::TILE);

    pub fn of_children(children: [Occupancy; 4]) -> Occupancy {
        Occupancy::of_slots(children.map(Occupancy::is_tile))
    }

    /// A tile whose children in `slot_of`'s order are tiles where `slots` says.
    pub fn of_slots(slots: [bool; 4]) -> Occupancy {
        let bits = (0..4).filter(|&q| slots[q]).fold(0, |bits, q| bits | 1 << q);
        Occupancy(if bits == 0 { 0 } else { Occupancy::TILE | bits })
    }

    pub fn is_tile(self) -> bool {
        self.0 & Occupancy::TILE != 0
    }

    pub fn has_child(self, slot: usize) -> bool {
        self.0 & (1 << slot) != 0
    }

    pub fn bits(self) -> u8 {
        self.0
    }

    /// None for a byte no tile can have: a bit past the five, or a child without its parent.
    pub fn of_bits(bits: u8) -> Option<Occupancy> {
        (bits == 0 || (bits & Occupancy::TILE != 0 && bits < Occupancy::TILE << 1)).then_some(Occupancy(bits))
    }
}

/// A whole number a byte holds, never a cast of one that is not.
pub fn occupancy_of(number: f64) -> Result<Occupancy, MarshalError> {
    let whole = number.fract() == 0.0 && (0.0..=255.0).contains(&number);
    whole.then_some(number as u8).and_then(Occupancy::of_bits).ok_or_else(malformed)
}
