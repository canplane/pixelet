// pxl-spec/bake/tile.rs

//! A baked tile of each layer — terrain, place, division and the Earth's
//! picture — and what every kind's says alike: which of it and its children
//! are tiles.

use xpute_kit::status::error::MarshalError;
use xpute_kit::wire::xtp::{HDR_SZ, WORD_SZ};

use crate::bake::malformed;
use crate::bake::published::BaseLayer;

pub mod division;
pub mod earth;
pub mod place;
pub mod terrain;

/// The bytes an XTP packet's parts take (xpute-kit wire/xtp encode.rs
/// `PacketWriter`), from which a kind bounds its packet by its lanes: the
/// header; a branch's length word and a word a child; a sequence's length
/// word and its payload to the word; a scalar, with the padding before what
/// follows it, a word at most.
pub mod packet {
    use super::{HDR_SZ, WORD_SZ};

    const WORD: usize = WORD_SZ as usize;

    pub const HEADER: usize = HDR_SZ as usize;

    pub const SCALAR: usize = WORD;

    pub const fn branch(children: usize) -> usize {
        WORD * (1 + children)
    }

    pub const fn seq(bytes: usize) -> usize {
        WORD + bytes.next_multiple_of(WORD)
    }
}

/// Where a tile's encoding starts its buffer; it doubles to the packet's size.
pub const ENCODE_FROM: u32 = 1 << 10;

/// The most bytes a packet of `layer` takes: what a client may hold one
/// in, and refuses one past. Terrain's and the Earth's follow from their
/// lanes; places' and divisions' are a room their bake refuses a tile past.
pub const fn max_packet_bytes(layer: BaseLayer) -> usize {
    match layer {
        BaseLayer::terrain => terrain::MAX_PACKET_BYTES,
        BaseLayer::place => place::MAX_PACKET_BYTES,
        BaseLayer::division => division::MAX_PACKET_BYTES,
        BaseLayer::earth => earth::PACKET_BYTES,
    }
}

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
