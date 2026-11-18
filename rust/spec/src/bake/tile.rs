// pxl-spec/bake/tile.rs

//! A baked tile of each layer — terrain, place, division and the imagery
//! picture — and what every kind's says alike: which of it and its children
//! are tiles.

use xpute_kit::status::error::XputeError;
use xpute_kit::wire::xtp::{HDR_SZ, WORD_SZ};

use crate::bake::malformed;
use crate::bake::publication::BaseKind;
use crate::space::coord::zoom::{LEAF_ZOOM, Zoom};
use crate::space::layer::lattice::{GROUND, Lattice, coarsest};

pub mod division;
pub mod imagery;
pub mod place;
pub mod terrain;

/// Strings end to end in one buffer, read by index.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct StringTable {
    bytes: String,
    ends: Vec<u32>,
}

impl StringTable {
    fn push(&mut self, s: &str) {
        self.bytes.push_str(s);
        self.ends.push(self.bytes.len() as u32);
    }

    pub(crate) fn get(&self, i: u32) -> &str {
        let Some(&end) = self.ends.get(i as usize) else {
            return "";
        };
        let start = if i == 0 { 0 } else { self.ends[i as usize - 1] };
        &self.bytes[start as usize..end as usize]
    }
}

/// The bytes an XTP packet's parts take (xpute-kit wire/xtp encode.rs
/// `PacketWriter`), from which a kind bounds its packet by its lanes: the
/// header; a branch's length word and a word a child; a sequence's length
/// word and its payload to the word.
pub mod pkt {
    use super::{HDR_SZ, WORD_SZ};

    const WORD: usize = WORD_SZ as usize;

    pub(crate) const HDR: usize = HDR_SZ as usize;

    pub const fn branch(children: usize) -> usize {
        WORD * (1 + children)
    }

    pub(crate) const fn seq(bytes: usize) -> usize {
        WORD + bytes.next_multiple_of(WORD)
    }
}

/// What each kind's tiles are, the one table of them (`BaseKind::row`).
pub struct Row {
    /// How its tiles lie on the camera grid, which says what part of the grid
    /// a tile at each zoom draws.
    pub lattice: Lattice,
    /// The zoom its tiles are made at; every coarser one is folded.
    pub leaf: Zoom,
    /// The most bytes a packet of it takes: what a client may hold one in,
    /// and refuses one past. The imagery's follows from its lanes; every other
    /// kind's is a room its bake refuses a tile past.
    pub pkt_cap: u32,
}

impl BaseKind {
    pub const fn row(self) -> Row {
        let (lattice, leaf, pkt_cap) = match self {
            BaseKind::Terrain => (GROUND, LEAF_ZOOM, terrain::PKT_CAP),
            BaseKind::Place => (place::PLACE_LATTICE, LEAF_ZOOM, place::PKT_CAP),
            BaseKind::Division => (GROUND, division::DIVISION_LEAF, division::PKT_CAP),
            BaseKind::Imagery => (GROUND, imagery::IMAGERY_ZOOM, imagery::PKT_SZ),
        };
        Row {
            lattice,
            leaf,
            pkt_cap,
        }
    }
}

/// A kind's tile as a type, the client's and the bake's alike: its row, and
/// what the tile says of its children.
pub trait Tile {
    const KIND: BaseKind;

    const LATTICE: Lattice = Self::KIND.row().lattice;

    const LEAF: Zoom = Self::KIND.row().leaf;

    /// The coarsest zoom the kind's tiles are read at, up to which the bake
    /// folds them (`lattice::coarsest`).
    const COARSEST: Zoom = coarsest(Self::LATTICE);

    /// Which of the tile's children have tiles of their own, as the bake
    /// wrote them.
    fn occupancy(&self) -> Occupancy;
}

/// Which of a tile and its four children are tiles of their own, the rest said whole by the
/// tile (terrain's zero tile, places' uncrowded ground): the children in the low four bits in
/// `child_idx`'s order, and the tile itself above them. A client never asks for a child its
/// parent marks absent.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Occupancy(u8);

impl Occupancy {
    const TILE: u8 = 1 << 4;

    pub const ZERO: Occupancy = Occupancy(0);

    pub const LEAF: Occupancy = Occupancy(Occupancy::TILE);

    pub(crate) fn of_children(children: [Occupancy; 4]) -> Occupancy {
        Occupancy::of_slots(children.map(Occupancy::is_tile))
    }

    /// A tile whose children in `child_idx`'s order are tiles where `slots` says.
    pub fn of_slots(slots: [bool; 4]) -> Occupancy {
        let bits = (0..4)
            .filter(|&q| slots[q])
            .fold(0, |bits, q| bits | 1 << q);
        Occupancy(if bits == 0 { 0 } else { Occupancy::TILE | bits })
    }

    pub(crate) fn is_tile(self) -> bool {
        self.0 & Occupancy::TILE != 0
    }

    pub fn has_child(self, slot: usize) -> bool {
        self.0 & (1 << slot) != 0
    }

    pub fn bits(self) -> u8 {
        self.0
    }

    /// None for a byte no tile can have: a bit past the five, or a child without its parent.
    pub(crate) fn of_bits(bits: u8) -> Option<Occupancy> {
        (bits == 0 || (bits & Occupancy::TILE != 0 && bits < Occupancy::TILE << 1))
            .then_some(Occupancy(bits))
    }
}

/// The occupancy a byte holds, refused rather than cast where the byte is not one.
pub(crate) fn occupancy_of(bits: u8) -> Result<Occupancy, XputeError> {
    Occupancy::of_bits(bits).ok_or_else(malformed)
}
