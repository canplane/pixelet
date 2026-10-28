// pxl-spec/bake/tile/place.rs

//! The places layer's tile: the places OpenStreetMap Carto would draw within
//! it at its zoom, each with the zoom it is shown from. A tile at zoom z
//! serves Carto's zoom z; the leaf serves every zoom past it too. Where icons
//! would meet, a cell an icon wide keeps one, the first by the zoom its category
//! shows from and then its id, so what is shown only grows with the zoom and
//! a tile is its children with what shows from its zoom on
//! (doc/minutes/20261009-places-follow-carto.md). It is one XTP tree, as every
//! other baked tile is.

use crate::space::layer::lattice::Cells;
use fnv::FnvHashSet;

use crate::asset::symbol::SYMBOLS;
use crate::bake::place_theme::{CATEGORIES, DOT_ZOOM, ICON_ZOOM, SYMBOL};
use crate::bake::tile::{occupancy_of, Occupancy};
use crate::space::coord::q3::Q3;
use crate::space::coord::zoom::Zoom;
use crate::space::layer::lattice::{Lattice, GROUND};
use crate::space::layer::tile::{Cell, Tile};
use pxl_spec_core::param::tile::LEAF_ZOOM;
use xpute_kit::status::error::MarshalError;
use xpute_kit::wire::xtp::{encoder, NodeView, TreeEncoderOptions, TreeReader, TreeView};

use crate::bake::malformed;
use crate::param::place::SPREAD_STEPS;

/// An icon's cell: a mark's, which a power of two makes nest.
const ICON_STEPS: Zoom = crate::param::marker::STEPS;

/// A cell an icon wide at its tile's zoom of Carto's, `SPREAD_STEPS` coarser
/// than the camera grid: which zoom of Carto's a place is drawn at there,
/// and so which categories show, is the tile zoom of the grid's cell
/// (`Lattice::tile_zoom_of_grid`), past the leaf too.
pub const PLACE_LATTICE: Lattice =
    Lattice::new(LEAF_ZOOM + ICON_STEPS).scaled(-(SPREAD_STEPS as i32));

/// The cell a place keeps to itself, as a zoom under the tile. Seen at an
/// icon's own cell, places a few metres apart stood side by side as near as
/// the view came.
const SPACING_STEPS: Zoom = ICON_STEPS - SPREAD_STEPS;

/// The coarsest zoom any category is shown from; no tile coarser holds a place.
pub const PLACE_FIRST: Zoom = {
    let mut first = crate::param::place::LAST;
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

// A category shown before a place's cell fits a leaf (peaks, airports) is spaced
// within its leaf alone: a leaf places every zoom inside itself.

// A place stands at its ground leaf cell, an offset in the tile a half-word a lane.
const _: () = assert!(
    GROUND.cell() - PLACE_FIRST <= 16,
    "a tile's ground leaf cells no longer fit a half-word a lane"
);

/// The most a packet takes, a tile past it failing its bake (`encode`): the
/// set baked 2026-10-17 holds at most 107 KiB.
pub const MAX_PACKET_BYTES: usize = 1 << 20;

/// The longest name a tile holds, in code points; a longer one is cut to end
/// in an ellipsis. At 32, 99% of the places Overture names in Korea are whole
/// (their names' 99th percentile, measured 2026-09-25).
pub const NAME_CHARS: usize = 32;

/// The zoom a category is first drawn from, as a dot or its icon; none for a category
/// Carto draws no point for.
pub const fn shown(category: u16) -> Option<Zoom> {
    if category == 0 || category as usize > CATEGORIES.len() {
        return None;
    }
    let (dot, icon) = (
        DOT_ZOOM[category as usize - 1],
        ICON_ZOOM[category as usize - 1],
    );
    match (dot, icon) {
        (0, 0) => None,
        (0, z) | (z, 0) => Some(z as Zoom),
        (d, i) => Some((if d < i { d } else { i }) as Zoom),
    }
}

/// A category's Carto symbol, by its place in the shipped `SYMBOLS`; none for a
/// category Carto draws no icon for.
pub fn symbol_of(category: u16) -> Option<u16> {
    let path = SYMBOL.get(usize::from(category.checked_sub(1)?))?;
    SYMBOLS.binary_search(path).ok().map(|i| i as u16)
}

/// 0 where the category list has none.
pub fn category_of(category: &str) -> u16 {
    CATEGORIES
        .iter()
        .position(|k| *k == category)
        .map_or(0, |i| i as u16 + 1)
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
        let Some(&end) = self.ends.get(i as usize) else {
            return "";
        };
        let start = if i == 0 { 0 } else { self.ends[i as usize - 1] };
        &self.bytes[start as usize..end as usize]
    }
}

/// A place as a source gives it: its own name — Carto labels a place by its
/// `name`, never translated — its category and the source's id for it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Place<'a> {
    pub name: &'a str,
    pub category: u16,
    pub id: &'a str,
}

/// A place a tile holds: what a source gave, the zoom it is shown from, and
/// the ground leaf cell it stands in, as an offset in the tile.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placed<'a> {
    pub place: Place<'a>,
    pub from: Zoom,
    pub at: [u32; 2],
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlaceTile {
    /// Each place's offset, ascending by row then column.
    at: Vec<[u16; 2]>,
    categories: Vec<u16>,
    from: Vec<u8>,
    names: Strings,
    ids: Strings,
    /// The children that are tiles: those holding a place this tile does not.
    pub occupancy: Occupancy,
}

impl PlaceTile {
    pub fn len(&self) -> u32 {
        self.categories.len() as u32
    }

    pub fn is_empty(&self) -> bool {
        self.categories.is_empty()
    }

    pub fn get(&self, i: u32) -> Placed<'_> {
        let k = i as usize;
        Placed {
            place: Place {
                name: self.names.get(i),
                category: self.categories[k],
                id: self.ids.get(i),
            },
            from: Zoom::from(self.from[k]),
            at: self.at[k].map(u32::from),
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (u32, Placed<'_>)> + '_ {
        (0..self.len()).map(|i| (i, self.get(i)))
    }

    fn push(&mut self, h: &Placed) {
        self.at.push(h.at.map(|v| v as u16));
        self.categories.push(h.place.category);
        // The record keeps a zoom in a byte.
        self.from.push(h.from as u8);
        self.names.push(h.place.name);
        self.ids.push(h.place.id);
    }

    /// A leaf from the places standing in it: those of a category Carto draws,
    /// placed at every zoom from the coarsest through Carto's last, a cell
    /// an icon wide keeping its first; names cut to `NAME_CHARS`.
    pub fn leaf<'a>(tile: Tile, places: impl IntoIterator<Item = (Q3, Place<'a>)>) -> PlaceTile {
        let names: Vec<(Cell, Zoom, String, Place<'a>)> = places
            .into_iter()
            .filter_map(|(point, place)| {
                Some((
                    GROUND.cell_of(LEAF_ZOOM, &point),
                    shown(place.category)?,
                    cut_name(place.name),
                    place,
                ))
            })
            .filter(|(cell, ..)| cell.tile() == tile)
            .collect();
        let mut order: Vec<usize> = (0..names.len()).collect();
        order.sort_by(|&a, &b| {
            (names[a].1, names[a].3.id, names[a].3.name).cmp(&(
                names[b].1,
                names[b].3.id,
                names[b].3.name,
            ))
        });
        let mut from: Vec<Option<Zoom>> = vec![None; names.len()];
        let mut taken = FnvHashSet::default();
        for zoom in PLACE_FIRST..=crate::param::place::LAST {
            taken.clear();
            for &p in &order {
                let (cell, shows, ..) = &names[p];
                if *shows <= zoom && taken.insert(place_cell(*cell, zoom)) && from[p].is_none() {
                    from[p] = Some(zoom);
                }
            }
        }
        let placed = (0..names.len()).filter_map(|p| {
            let (cell, _, name, place) = &names[p];
            Some(Placed {
                place: Place {
                    name: name.as_str(),
                    ..*place
                },
                from: from[p]?,
                at: cell.in_tile(),
            })
        });
        gathered(placed, Occupancy::ZERO)
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
pub fn fold(p: Tile, children: [&PlaceTile; 4]) -> PlaceTile {
    let side = 1u32 << (GROUND.cell() - p.zoom - 1);
    let mut placed = Vec::new();
    let mut kept = [false; 4];
    for (q, (k, child)) in p.children().into_iter().zip(children).enumerate() {
        let [cx, cy] = k.node().1.map(|v| (v & 1) as u32);
        kept[q] = child.occupancy.bits() != 0 || child.iter().any(|(_, h)| h.from > p.zoom);
        placed.extend(
            child
                .iter()
                .filter(|(_, h)| h.from <= p.zoom)
                .map(|(_, h)| Placed {
                    at: [cx * side + h.at[0], cy * side + h.at[1]],
                    ..h
                }),
        );
    }
    gathered(placed, Occupancy::of_slots(kept))
}

/// Places ascending by where they stand.
fn gathered<'a>(placed: impl IntoIterator<Item = Placed<'a>>, occupancy: Occupancy) -> PlaceTile {
    let mut placed: Vec<Placed> = placed.into_iter().collect();
    placed.sort_by_key(|h| [h.at[1], h.at[0]]);
    let mut tile = PlaceTile {
        occupancy,
        ..PlaceTile::default()
    };
    placed.iter().for_each(|h| tile.push(h));
    tile
}

/// `name` cut to `NAME_CHARS` code points, ending in an ellipsis.
fn cut_name(name: &str) -> String {
    match name.char_indices().nth(NAME_CHARS) {
        None => name.into(),
        Some(_) => {
            let end = name
                .char_indices()
                .nth(NAME_CHARS - 1)
                .map_or(name.len(), |(at, _)| at);
            format!("{}…", &name[..end])
        }
    }
}

/// One branch: per place its offset (two half-words), its category, the zoom it
/// is shown from, the source's id and its name; the occupancy.
pub fn encode(tile: &PlaceTile) -> Result<Vec<u8>, MarshalError> {
    validate(tile)?;
    let at: Vec<u16> = tile.at.iter().flatten().copied().collect();
    let ids: Vec<&str> = (0..tile.len()).map(|i| tile.ids.get(i)).collect();
    let names: Vec<&str> = (0..tile.len()).map(|i| tile.names.get(i)).collect();
    let mut t: TreeView<'_> = TreeView::new();
    t.branch(Some(&mut |b| {
        b.u16_array(Some(&at))
            .u16_array(Some(&tile.categories))
            .u8_array(Some(&tile.from))
            .strs(Some(&ids))
            .strs(Some(&names))
            .u8(Some(tile.occupancy.bits()));
    }));
    encoder().encode(
        &t,
        TreeEncoderOptions {
            init_cap: super::ENCODE_FROM,
            max_cap: Some(MAX_PACKET_BYTES as u32),
        },
    )
}

pub fn decode(bytes: &[u8]) -> Result<PlaceTile, MarshalError> {
    let root = TreeReader::new(bytes)?.read_branch()?;
    let (at, categories, from) = (
        root.at(0)?.get_array::<u16>()?,
        root.at(1)?.get_array::<u16>()?,
        root.at(2)?.get_array::<u8>()?,
    );
    let (ids, names) = (root.at(3)?.get_strs()?, root.at(4)?.get_strs()?);
    let occupancy = occupancy_of(root.at(5)?.get_number()?)?;
    let n = categories.len();
    if at.len() != 2 * n || from.len() != n || ids.len() as usize != n || names.len() as usize != n
    {
        return Err(malformed());
    }
    let mut tile = PlaceTile {
        occupancy,
        ..PlaceTile::default()
    };
    for i in 0..n {
        tile.push(&Placed {
            place: Place {
                name: names.get(i as u32).ok_or_else(malformed)?,
                category: categories[i],
                id: ids.get(i as u32).ok_or_else(malformed)?,
            },
            from: Zoom::from(from[i]),
            at: [u32::from(at[2 * i]), u32::from(at[2 * i + 1])],
        });
    }
    validate(&tile)?;
    Ok(tile)
}

/// What a tile holds: places ascending by where they stand, each in its own
/// cell; categories Carto draws; each shown from no sooner than its category and no
/// later than Carto's last zoom; names whole within `NAME_CHARS`.
fn validate(tile: &PlaceTile) -> Result<(), MarshalError> {
    let ascending = tile
        .at
        .windows(2)
        .all(|w| [w[0][1], w[0][0]] < [w[1][1], w[1][0]]);
    let bad = |(_, h): (u32, Placed)| {
        let shows = shown(h.place.category);
        shows.is_none_or(|z| h.from < z || h.from > crate::param::place::LAST)
            || h.place.name.chars().nth(NAME_CHARS).is_some()
    };
    if !ascending || tile.iter().any(bad) {
        return Err(malformed());
    }
    Ok(())
}

#[cfg(test)]
#[path = "place.test.rs"]
mod test;
