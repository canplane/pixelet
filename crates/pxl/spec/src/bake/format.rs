// pxl-spec/bake/format.rs

//! A baked terrain page and its wire: `[ele, cells, extent, occupancy, decks]`.

use xpute_core::status::error::MarshalError;
use xpute_core::wire::xtp::{encoder, NodeView, TreeEncoderOptions, TreeReader, TreeView};

use std::borrow::Cow;

use super::constant::ELE_PADDED_SPAN;
use super::deck::{self, Decks};
use super::ground::SHIFT_RANGE;
use super::malformed;
use super::pack::{channel_of, GroundCell};
use super::surface::extent_of;
use crate::space::pyramid::{EXTENT, PAGE_CELLS};

/// A page's lane lengths; a page is these and no other size.
pub const GROUND_LANE: usize = (PAGE_CELLS * PAGE_CELLS) as usize;
pub const ELE_LANE: usize = (ELE_PADDED_SPAN * ELE_PADDED_SPAN) as usize;
pub const EXTENT_LANE: usize = (EXTENT.page_cells() * EXTENT.page_cells()) as usize;
pub const BLOCK_LANE: usize = EXTENT.page_blocks() as usize;

pub use crate::cell::cube_type::GroundChannelKey;

/// The ground channels' order, which is also paint order: a later channel's
/// non-void type wins. Structure paints last because a small building that
/// loses its street-facing cells to a road can stop existing.
pub const GROUND_CH_KEYS: [GroundChannelKey; 4] = [GroundChannelKey::LAND, GroundChannelKey::WATER, GroundChannelKey::TRANSPORT, GroundChannelKey::STRUCTURE];

pub const fn ch_index(key: GroundChannelKey) -> usize {
    let mut i = 0;
    while i < GROUND_CH_KEYS.len() {
        if GROUND_CH_KEYS[i] as u8 == key as u8 {
            return i;
        }
        i += 1;
    }
    panic!()
}

const _: () = {
    let mut id = 0;
    while id < GROUND_CH_KEYS.len() as u32 {
        match GroundChannelKey::from_id(id) {
            Some(k) => assert!(GROUND_CH_KEYS[ch_index(k)] as u8 == k as u8),
            None => panic!("an identity with no channel"),
        }
        id += 1;
    }
};

/// A page's ground: one packed word a cell (`bake/pack.rs`), the same shape
/// from the bake through the wire to the client.
#[derive(Clone, Debug, PartialEq)]
pub struct Ground {
    pub cells: Cow<'static, [GroundCell]>,
}

impl Ground {
    pub fn channel(&self, i: usize, key: GroundChannelKey) -> u8 {
        channel_of(self.cells[i], key)
    }

    pub fn len(&self) -> usize {
        self.cells.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }
}

/// Which of a page and its four children are real pages rather than the zero
/// (ocean) page: children in the low four bits in `slot_of`'s order, the page
/// itself above them. A client never asks for a child its parent marks absent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Occupancy(u8);

impl Occupancy {
    const PAGE: u8 = 1 << 4;

    pub const ZERO: Occupancy = Occupancy(0);

    pub const LEAF: Occupancy = Occupancy(Occupancy::PAGE);

    pub fn of_children(children: [Occupancy; 4]) -> Occupancy {
        let bits = (0..4).filter(|&q| children[q].is_page()).fold(0, |bits, q| bits | 1 << q);
        Occupancy(if bits == 0 { 0 } else { Occupancy::PAGE | bits })
    }

    pub fn is_page(self) -> bool {
        self.0 & Occupancy::PAGE != 0
    }

    pub fn has_child(self, slot: usize) -> bool {
        self.0 & (1 << slot) != 0
    }

    pub fn bits(self) -> u8 {
        self.0
    }

    /// None for a byte no page can have: a bit past the five, or a child without its parent.
    pub fn of_bits(bits: u8) -> Option<Occupancy> {
        (bits == 0 || (bits & Occupancy::PAGE != 0 && bits < Occupancy::PAGE << 1)).then_some(Occupancy(bits))
    }
}

/// What a ray and a fetch need before the pages under one are held. At every
/// zoom, the leaf too, so a page has one contract.
#[derive(Clone, Debug, PartialEq)]
pub struct Index {
    extent: Cow<'static, [[i16; 2]]>, // lo, hi for each `EXTENT` cell, over this zoom and all finer: a ray above hi misses them
    blocks: Cow<'static, [Block]>,    // `extent` joined over 2^j squares: rebuilt from it, never on the wire
    pub occupancy: Occupancy,         // which children are pages: none is asked for that is not
}

/// An aligned block's lowest and highest extent and widest single-cell span;
/// a block is exactly its four quarters' join.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(C)]
pub struct Block {
    pub lo: i16,
    pub hi: i16,
    pub span: u16,
}

/// The object every page of open water resolves to.
pub const OCEAN_KEY: &str = "_ocean";

/// A partial version's list, beside its pages: the full version it overlays
/// and the pages it writes or removes over it, by quadkey. A version without
/// one is full.
pub const SNAPSHOT_KEY: &str = "_snapshot";

/// How far past the lane's facts the client draws a cell, down and up: the
/// ground's shift and a bore's floor. Every read of the lane widens by it.
pub const DRAWN_PAST: [i16; 2] = {
    let [lo, hi] = SHIFT_RANGE;
    // Ground shifted down, or the way's own cell under a bore's or a cutting's surface.
    [if lo < -1 { lo } else { -1 }, hi]
};

pub fn drawn([lo, hi]: [i16; 2]) -> [i16; 2] {
    [lo.saturating_add(DRAWN_PAST[0]), hi.saturating_add(DRAWN_PAST[1])]
}

impl Block {
    /// One cell as a block: its bound widened by `drawn`, its span as the lane holds it.
    pub fn of(cell: [i16; 2]) -> Block {
        let [lo, hi] = drawn(cell);
        Block {
            lo,
            hi,
            span: (i32::from(cell[1]) - i32::from(cell[0])).max(0) as u16,
        }
    }

    fn join(self, o: Block) -> Block {
        Block {
            lo: self.lo.min(o.lo),
            hi: self.hi.max(o.hi),
            span: self.span.max(o.span),
        }
    }
}

impl Index {
    pub fn new(extent: Vec<[i16; 2]>, occupancy: Occupancy) -> Index {
        let mut blocks = vec![Block::default(); BLOCK_LANE];
        blocks_of(&extent, &mut blocks);
        Index {
            extent: extent.into(),
            blocks: blocks.into(),
            occupancy,
        }
    }

    pub fn extent(&self) -> &[[i16; 2]] {
        &self.extent
    }

    /// The block of `across` × `across` cells at (x, y); `across` a power of two and (x, y) a multiple of it.
    pub fn over(&self, [x, y]: [u32; 2], across: u32) -> Block {
        match across.trailing_zeros() {
            0 => Block::of(self.extent[EXTENT.cell_index(x, y) as usize]),
            j => self.blocks[EXTENT.block_index(j, x, y) as usize],
        }
    }
}

/// Each block size from the one under it.
fn blocks_of(lane: &[[i16; 2]], blocks: &mut [Block]) {
    xpute_core::ensure!(lane.len() == EXTENT_LANE && blocks.len() == BLOCK_LANE, ENOTRECOVERABLE);
    let n = EXTENT.page_cells();
    for j in 1..=n.trailing_zeros() {
        let side = 1u32 << j;
        for by in (0..n).step_by(side as usize) {
            for bx in (0..n).step_by(side as usize) {
                let half = side / 2;
                let quarter = |dx: u32, dy: u32| match j {
                    1 => Block::of(lane[EXTENT.cell_index(bx + dx, by + dy) as usize]),
                    _ => blocks[EXTENT.block_index(j - 1, bx + dx, by + dy) as usize],
                };
                let b = quarter(0, 0).join(quarter(half, 0)).join(quarter(0, half)).join(quarter(half, half));
                blocks[EXTENT.block_index(j, bx, by) as usize] = b;
            }
        }
    }
}

/// A page: one lane a fact the client cannot make. What it draws by rule —
/// a slab, a bore, a pier — is not here.
#[derive(Clone, Debug, PartialEq)]
pub struct Terrain {
    pub ground: Ground,           // a class a cell: the source rasterized
    pub decks: Decks,             // ways off the ground: source lines, tops solved offline
    pub ele: Cow<'static, [i16]>, // padded height samples, in z16 cells: from the raster
    pub index: Index,             // bounds over the page and all under it: no one page holds them
}

// Made on first use: a constant array of zeros would ship in the module's data.
static VOID_ELE: std::sync::LazyLock<Vec<i16>> = std::sync::LazyLock::new(|| vec![0; ELE_LANE]);
static VOID_CELLS: std::sync::LazyLock<Vec<GroundCell>> = std::sync::LazyLock::new(|| vec![0; GROUND_LANE]);

impl Terrain {
    /// Where the source has nothing at all: void cells at sea level, so
    /// nothing is drawn while the ground is still known and stays asked for,
    /// and no child, so none is asked for under it.
    pub fn void() -> Terrain {
        Terrain {
            ground: Ground { cells: Cow::Borrowed(&VOID_CELLS[..]) },
            decks: Decks::default(),
            ele: Cow::Borrowed(&VOID_ELE[..]),
            index: Index::new(vec![[0, 0]; EXTENT_LANE], Occupancy::ZERO),
        }
    }

    /// A z16 page, its index taken from its own surface and decks.
    pub fn leaf(ele: Vec<i16>, ground: Ground, decks: Decks) -> Terrain {
        let extent = extent_of(&ele, &decks);
        Terrain {
            ground,
            decks,
            ele: ele.into(),
            index: Index::new(extent, Occupancy::LEAF),
        }
    }
}

// Cells go out packed, as the client holds them, so decoding is a length check and a copy.
pub fn encode_terrain(page: &Terrain) -> Result<Vec<u8>, MarshalError> {
    let mut t: TreeView<'_> = TreeView::new();
    t.branch(Some(&mut |b| {
        b.i16_array(Some(&page.ele))
            .u16_array(Some(&page.ground.cells))
            .i16_array(Some(page.index.extent.as_flattened()))
            .u8(Some(page.index.occupancy.bits()));
        deck::write_branch(&page.decks, b);
    }));
    encoder().encode(&t, TreeEncoderOptions::default())
}

/// The record's lanes in `.fixed.terrain` a page is decoded into.
pub struct PageRoom {
    pub cells: &'static mut [GroundCell],
    pub ele: &'static mut [i16],
    pub extent: &'static mut [[i16; 2]],
    pub blocks: &'static mut [Block],
    pub decks: deck::Room,
}

struct Lanes<'a> {
    ele: &'a [i16],
    cells: &'a [GroundCell],
    extent: &'a [[i16; 2]],
    occupancy: Occupancy,
    decks: deck::Lanes<'a>,
}

/// Refused unless every lane is a page's length.
fn read_page(pkt: &[u8]) -> Result<Lanes<'_>, MarshalError> {
    let root = TreeReader::new(pkt)?.read_branch()?;
    let ele = root.at(0)?.get_array::<i16>()?;
    let cells = root.at(1)?.get_array::<GroundCell>()?;
    let extent = pairs(root.at(2)?.get_array::<i16>()?)?;
    let occupancy = occupancy_of(root.at(3)?.get_number()?)?;
    let decks = deck::read_branch(root.at_branch(4)?)?;
    if ele.len() != ELE_LANE || cells.len() != GROUND_LANE || extent.len() != EXTENT_LANE {
        return Err(malformed());
    }
    Ok(Lanes { ele, cells, extent, occupancy, decks })
}

/// Decode straight into the page's resident room; the index's blocks are
/// built there before the page is returned.
pub fn decode_terrain_into(pkt: &[u8], room: PageRoom) -> Result<Terrain, MarshalError> {
    let Lanes { ele, cells, extent, occupancy, decks } = read_page(pkt)?;
    if room.ele.len() != ELE_LANE || room.cells.len() != GROUND_LANE || room.extent.len() != EXTENT_LANE {
        return Err(malformed());
    }
    room.ele.copy_from_slice(ele);
    room.cells.copy_from_slice(cells);
    room.extent.copy_from_slice(extent);
    blocks_of(room.extent, room.blocks);
    let PageRoom {
        cells,
        ele,
        extent,
        blocks,
        decks: deck_room,
    } = room;
    Ok(Terrain {
        ground: Ground { cells: (&*cells).into() },
        decks: decks.into_room(deck_room),
        ele: (&*ele).into(),
        index: Index {
            extent: (&*extent).into(),
            blocks: (&*blocks).into(),
            occupancy,
        },
    })
}

// Copied out, for a page that outlives its packet.
pub fn decode_terrain(pkt: &[u8]) -> Result<Terrain, MarshalError> {
    let Lanes { ele, cells, extent, occupancy, decks } = read_page(pkt)?;
    Ok(Terrain {
        ele: ele.to_vec().into(),
        ground: Ground { cells: cells.to_vec().into() },
        decks: decks.to_decks(),
        index: Index::new(extent.to_vec(), occupancy),
    })
}

fn pairs(flat: &[i16]) -> Result<&[[i16; 2]], MarshalError> {
    match flat.as_chunks::<2>() {
        (pairs, []) => Ok(pairs),
        _ => Err(malformed()),
    }
}

fn occupancy_of(number: f64) -> Result<Occupancy, MarshalError> {
    u8::try_from(number as i64).ok().and_then(Occupancy::of_bits).ok_or_else(malformed)
}

#[cfg(test)]
#[path = "format.test.rs"]
mod test;
