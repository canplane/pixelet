// pxl-spec/bake/place_tile.rs

//! The places layer's tile: the places OpenStreetMap Carto would draw within
//! it at its zoom, each with the zoom it is shown from. A tile at zoom z
//! serves Carto's zoom z; the leaf serves every zoom past it too. Where icons
//! would meet, a cell an icon wide keeps one, the first by the zoom its kind
//! shows from and then its id, so what is shown only grows with the zoom and
//! a tile is its children with what shows from its zoom on
//! (decision/20261009-places-follow-carto.md). It is one XTP tree, as every
//! other baked tile is.

use std::collections::HashSet;

use crate::asset::symbol::SYMBOLS;
use crate::bake::format::{occupancy_of, Occupancy};
use crate::bake::place_theme::{CATEGORIES, DOT_ZOOM, ICON_ZOOM, SYMBOL};
use crate::cell::object::MARK_STEPS;
use crate::space::coord::pvf3::pvf3::Pvf3;
use crate::space::coord::zoom::Zoom;
use crate::space::layer::lattice::{Lattice, GROUND};
use crate::space::layer::tile::{Cell, Tile, LEAF_ZOOM};
use xpute_core::status::error::MarshalError;
use xpute_core::wire::xtp::{encoder, NodeView, TreeEncoderOptions, TreeReader, TreeView};

use crate::bake::malformed;

/// Carto's last zoom, openstreetmap.org's: the leaf places through it.
pub const PLACE_LAST: Zoom = 19;

/// An icon's cell: a mark's, which a power of two makes nest.
const ICON_STEPS: Zoom = MARK_STEPS;

/// A cell an icon wide at its tile's zoom of Carto's, scaled a step coarser
/// than the camera grid, so an icon spans two grid cells, 20 to 24 pixels
/// where Carto's is 16: which zoom of Carto's a place is drawn at there, and
/// so which kinds show, is the tile zoom of the grid's cell
/// (`Lattice::tile_zoom_of_grid`), past the leaf too.
pub const PLACE_LATTICE: Lattice = Lattice::new(LEAF_ZOOM + ICON_STEPS).scaled(-1);

/// The cell a place keeps to itself, as a zoom under the tile: four icons a
/// side, eight grid cells, some 80 to 100 pixels on a phone or a laptop,
/// which also holds a name of a few words. A product choice, as places are
/// seen: at an icon's own cell, places a few metres apart stood side by side
/// as near as the view came.
const SPACING_STEPS: Zoom = ICON_STEPS - 2;

/// The coarsest zoom any kind is shown from; no tile coarser holds a place.
pub const PLACE_FIRST: Zoom = {
    let mut first = PLACE_LAST;
    let mut k = 0;
    while k < CATEGORIES.len() {
        if let Some(z) = shown(k as u16 + 1) {
            if z < first {
                first = z;
            }
        }
        k += 1;
    }
    first
};

// A kind shown before a place's cell fits a leaf (peaks, airports) is spaced
// within its leaf alone: a leaf places every zoom inside itself.

// A place stands at its ground leaf cell, an offset in the tile a half-word a lane.
const _: () = assert!(GROUND.cell() - PLACE_FIRST <= 16, "a tile's ground leaf cells no longer fit a half-word a lane");

/// The longest name a tile holds, in code points; a longer one is cut to end
/// in an ellipsis. At 32, 99% of the places Overture names in Korea are whole
/// (their names' 99th percentile, measured 2026-09-25).
pub const NAME_CHARS: usize = 32;

/// The zoom a kind is first drawn from, as a dot or its icon; none for a kind
/// Carto draws no point for.
pub const fn shown(kind: u16) -> Option<Zoom> {
    if kind == 0 || kind as usize > CATEGORIES.len() {
        return None;
    }
    let (dot, icon) = (DOT_ZOOM[kind as usize - 1], ICON_ZOOM[kind as usize - 1]);
    match (dot, icon) {
        (0, 0) => None,
        (0, z) | (z, 0) => Some(z),
        (d, i) => Some(if d < i { d } else { i }),
    }
}

/// A kind's Carto symbol, by its place in the shipped `SYMBOLS`; none for a
/// kind Carto draws no icon for.
pub fn symbol_of(kind: u16) -> Option<u16> {
    let path = SYMBOL.get(usize::from(kind.checked_sub(1)?))?;
    SYMBOLS.binary_search(path).ok().map(|i| i as u16)
}

/// 0 where the category list has none.
pub fn kind_of(category: &str) -> u16 {
    CATEGORIES.iter().position(|k| *k == category).map_or(0, |i| i as u16 + 1)
}

/// Strings end to end in one buffer, read by index.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Strings {
    bytes: String,
    ends: Vec<u32>,
}

impl Strings {
    fn push(&mut self, s: &str) {
        self.bytes.push_str(s);
        self.ends.push(self.bytes.len() as u32);
    }

    pub fn get(&self, i: u32) -> &str {
        let Some(&end) = self.ends.get(i as usize) else { return "" };
        let start = if i == 0 { 0 } else { self.ends[i as usize - 1] };
        &self.bytes[start as usize..end as usize]
    }
}

/// A place as a source gives it: its own name — Carto labels a place by its
/// `name`, never translated — its kind and the source's id for it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Place<'a> {
    pub name: &'a str,
    pub kind: u16,
    pub id: &'a str,
}

/// A place a tile holds: what a source gave, the zoom it is shown from, and
/// the ground leaf cell it stands in, as an offset in the tile.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Held<'a> {
    pub place: Place<'a>,
    pub from: Zoom,
    pub at: [u32; 2],
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlaceTile {
    /// Each place's offset, ascending by row then column.
    at: Vec<[u16; 2]>,
    kinds: Vec<u16>,
    from: Vec<u8>,
    names: Strings,
    ids: Strings,
    /// The children that are tiles: those holding a place this tile does not.
    pub occupancy: Occupancy,
}

impl PlaceTile {
    pub fn len(&self) -> u32 {
        self.kinds.len() as u32
    }

    pub fn is_empty(&self) -> bool {
        self.kinds.is_empty()
    }

    pub fn get(&self, i: u32) -> Held<'_> {
        let k = i as usize;
        Held {
            place: Place {
                name: self.names.get(i),
                kind: self.kinds[k],
                id: self.ids.get(i),
            },
            from: self.from[k],
            at: self.at[k].map(u32::from),
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (u32, Held<'_>)> + '_ {
        (0..self.len()).map(|i| (i, self.get(i)))
    }

    fn push(&mut self, h: &Held) {
        self.at.push(h.at.map(|v| v as u16));
        self.kinds.push(h.place.kind);
        self.from.push(h.from);
        self.names.push(h.place.name);
        self.ids.push(h.place.id);
    }

    /// A leaf from the places standing in it: those of a kind Carto draws,
    /// placed at every zoom from the coarsest through Carto's last, a cell
    /// an icon wide keeping its first; names cut to `NAME_CHARS`.
    pub fn leaf<'a>(tile: Tile, places: impl IntoIterator<Item = (Pvf3, Place<'a>)>) -> PlaceTile {
        let names: Vec<(Cell, Zoom, String, Place<'a>)> = places
            .into_iter()
            .filter_map(|(point, place)| Some((GROUND.cell_of(LEAF_ZOOM, &point), shown(place.kind)?, cut_name(place.name), place)))
            .filter(|(cell, ..)| cell.tile() == tile)
            .collect();
        let mut order: Vec<usize> = (0..names.len()).collect();
        order.sort_by(|&a, &b| (names[a].1, names[a].3.id, names[a].3.name).cmp(&(names[b].1, names[b].3.id, names[b].3.name)));
        let mut from: Vec<Option<Zoom>> = vec![None; names.len()];
        let mut taken = HashSet::new();
        for zoom in PLACE_FIRST..=PLACE_LAST {
            taken.clear();
            for &p in &order {
                let (cell, shows, ..) = &names[p];
                if *shows <= zoom && taken.insert(place_cell(*cell, zoom)) && from[p].is_none() {
                    from[p] = Some(zoom);
                }
            }
        }
        let held = (0..names.len()).filter_map(|p| {
            let (cell, _, name, place) = &names[p];
            Some(Held {
                place: Place { name: name.as_str(), ..*place },
                from: from[p]?,
                at: cell.in_tile(),
            })
        });
        gathered(held, Occupancy::ZERO)
    }
}

/// The cell a place keeps at Carto's `zoom`, over the ground leaf cell it
/// stands in: past the ground's own, that cell.
fn place_cell(stands: Cell, zoom: Zoom) -> Cell {
    stands.up(GROUND.cell().saturating_sub(zoom + SPACING_STEPS))
}

/// The ground leaf cell a place a tile holds stands in.
pub fn stands(tile: Tile, at: [u32; 2]) -> Cell {
    GROUND.cell_at(tile, 0).inner_at(GROUND, LEAF_ZOOM, at)
}

/// `p` from its four children, in `Tile::children`'s order: what of theirs is
/// shown from `p`'s zoom on. A child is a tile where it holds more, or has a
/// child that does, so a place first shown deeper stays reachable.
pub fn fold_place(p: Tile, children: [&PlaceTile; 4]) -> PlaceTile {
    let side = 1u32 << (GROUND.cell() - p.zoom - 1);
    let mut held = Vec::new();
    let mut kept = [false; 4];
    for (q, (k, child)) in p.children().into_iter().zip(children).enumerate() {
        let [cx, cy] = k.node().1.map(|v| (v & 1) as u32);
        kept[q] = child.occupancy.bits() != 0 || child.iter().any(|(_, h)| h.from > p.zoom);
        held.extend(child.iter().filter(|(_, h)| h.from <= p.zoom).map(|(_, h)| Held {
            at: [cx * side + h.at[0], cy * side + h.at[1]],
            ..h
        }));
    }
    gathered(held, Occupancy::of_slots(kept))
}

/// Places ascending by where they stand.
fn gathered<'a>(held: impl IntoIterator<Item = Held<'a>>, occupancy: Occupancy) -> PlaceTile {
    let mut held: Vec<Held> = held.into_iter().collect();
    held.sort_by_key(|h| [h.at[1], h.at[0]]);
    let mut tile = PlaceTile { occupancy, ..PlaceTile::default() };
    held.iter().for_each(|h| tile.push(h));
    tile
}

/// `name` cut to `NAME_CHARS` code points, ending in an ellipsis.
fn cut_name(name: &str) -> String {
    match name.char_indices().nth(NAME_CHARS) {
        None => name.into(),
        Some(_) => {
            let end = name.char_indices().nth(NAME_CHARS - 1).map_or(name.len(), |(at, _)| at);
            format!("{}…", &name[..end])
        }
    }
}

/// One branch: per place its offset (two half-words), its kind, the zoom it
/// is shown from, the source's id and its name; the occupancy.
pub fn encode(tile: &PlaceTile) -> Result<Vec<u8>, MarshalError> {
    validate(tile)?;
    let at: Vec<u16> = tile.at.iter().flatten().copied().collect();
    let ids: Vec<&str> = (0..tile.len()).map(|i| tile.ids.get(i)).collect();
    let names: Vec<&str> = (0..tile.len()).map(|i| tile.names.get(i)).collect();
    let mut t: TreeView<'_> = TreeView::new();
    t.branch(Some(&mut |b| {
        b.u16_array(Some(&at))
            .u16_array(Some(&tile.kinds))
            .u8_array(Some(&tile.from))
            .strs(Some(&ids))
            .strs(Some(&names))
            .u8(Some(tile.occupancy.bits()));
    }));
    encoder().encode(&t, TreeEncoderOptions::default())
}

pub fn decode(bytes: &[u8]) -> Result<PlaceTile, MarshalError> {
    let root = TreeReader::new(bytes)?.read_branch()?;
    let (at, kinds, from) = (root.at(0)?.get_array::<u16>()?, root.at(1)?.get_array::<u16>()?, root.at(2)?.get_array::<u8>()?);
    let (ids, names) = (root.at(3)?.get_strs()?, root.at(4)?.get_strs()?);
    let occupancy = occupancy_of(root.at(5)?.get_number()?)?;
    let n = kinds.len();
    if at.len() != 2 * n || from.len() != n || ids.len() as usize != n || names.len() as usize != n {
        return Err(malformed());
    }
    let mut tile = PlaceTile { occupancy, ..PlaceTile::default() };
    for i in 0..n {
        tile.push(&Held {
            place: Place {
                name: names.get(i as u32).ok_or_else(malformed)?,
                kind: kinds[i],
                id: ids.get(i as u32).ok_or_else(malformed)?,
            },
            from: from[i],
            at: [u32::from(at[2 * i]), u32::from(at[2 * i + 1])],
        });
    }
    validate(&tile)?;
    Ok(tile)
}

/// What a tile holds: places ascending by where they stand, each in its own
/// cell; kinds Carto draws; each shown from no sooner than its kind and no
/// later than Carto's last zoom; names whole within `NAME_CHARS`.
fn validate(tile: &PlaceTile) -> Result<(), MarshalError> {
    let ascending = tile.at.windows(2).all(|w| [w[0][1], w[0][0]] < [w[1][1], w[1][0]]);
    let bad = |(_, h): (u32, Held)| {
        let shows = shown(h.place.kind);
        shows.is_none_or(|z| h.from < z || h.from > PLACE_LAST) || h.place.name.chars().nth(NAME_CHARS).is_some()
    };
    if !ascending || tile.iter().any(bad) {
        return Err(malformed());
    }
    Ok(())
}

#[cfg(test)]
#[path = "place_tile.test.rs"]
mod test;
