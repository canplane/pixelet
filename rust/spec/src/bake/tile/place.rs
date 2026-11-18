// pxl-spec/bake/tile/place.rs

//! The places layer's tile: the places OpenStreetMap Carto would draw within
//! it at its zoom, each with the zoom it is shown from. A tile at zoom z
//! serves Carto's zoom z; the leaf serves every zoom past it too. Where icons
//! would meet, a cell an icon wide keeps one, the first by the zoom its category
//! shows from and then its id, so what is shown only grows with the zoom and
//! a tile is its children with what shows from its zoom on. It is one XTP
//! tree, as every other baked tile is.

use crate::space::layer::lattice::{Cell, GROUND, Grid, Lattice};
use fnv::FnvHashSet;

use crate::asset::marker::MARK_STEP_CNT;
use crate::bake::place_theme::{CATEGORIES, DOT_ZOOM, ICON_ZOOM, SYMBOL};
use crate::bake::publication::BaseKind;
use crate::bake::tile::{Occupancy, StringTable, Tile, occupancy_of};
use crate::space::coord::q3::Q3;
use crate::space::coord::zoom::{LEAF_ZOOM, Zoom};
use crate::space::layer::quadrant::Quadrant;
use xpute_kit::status::error::XputeError;
use xpute_kit::wire::xtp::shape;

use crate::bake::malformed;
use crate::param::place::SPREAD_STEP_CNT;

/// Carto's last zoom, openstreetmap.org's: the leaf places through it.
pub(crate) const PLACE_LAST: Zoom = 19;

/// An icon's cell: a mark's, which a power of two makes nest.
const ICON_STEP_CNT: Zoom = MARK_STEP_CNT;

/// A cell an icon wide at its tile's Carto zoom, `SPREAD_STEP_CNT` coarser than the camera grid.
/// The Carto zoom at which a place is drawn there, and so which categories show, is the tile
/// zoom of the grid's cell (`Lattice::tile_zoom_of_grid`), past the leaf too.
pub const PLACE_LATTICE: Lattice =
    Lattice::new(LEAF_ZOOM + ICON_STEP_CNT).scaled(-(SPREAD_STEP_CNT as i32));

/// The cell a place keeps to itself, as a zoom under the tile.
const SPACING_STEP_CNT: Zoom = ICON_STEP_CNT - SPREAD_STEP_CNT;

/// The coarsest zoom any category is shown from; no tile coarser holds a place.
pub const PLACE_FIRST: Zoom = {
    let mut first = PLACE_LAST;
    let mut k = 0;
    while k < CATEGORIES.len() {
        if let Some(z) = first_zoom(k as u16 + 1)
            && z < first
        {
            first = z;
        }
        k += 1;
    }
    first
};

// A category shown before a place's cell fits a leaf (peaks, airports) is spaced
// within its leaf alone: a leaf places every zoom inside itself.

// A place stands at its ground leaf cell, an offset in the tile of a half-word on each axis.
const _: () = assert!(
    GROUND.leaf_cell_zoom() - PLACE_FIRST <= 16,
    "a tile's ground leaf cells no longer fit a half-word a lane"
);

/// The most a packet takes, a tile past it failing its bake (`encode`).
pub(crate) const PKT_CAP: u32 = 1 << 20;

/// Per place its offset (two half-words), its category, the zoom it is shown
/// from, the source's id and its name; the occupancy.
type Packet = (Vec<u16>, Vec<u16>, Vec<u8>, Vec<String>, Vec<String>, u8);

/// The longest name a tile holds, in code points; a longer one is cut to end
/// in an ellipsis.
pub(crate) const NAME_CHARS: usize = 32;

/// The zoom a category is first drawn from, as a dot or its icon; none for a category
/// Carto draws no point for.
pub const fn first_zoom(kind: u16) -> Option<Zoom> {
    if kind == 0 || kind as usize > CATEGORIES.len() {
        return None;
    }
    let (dot, icon) = (DOT_ZOOM[kind as usize - 1], ICON_ZOOM[kind as usize - 1]);
    match (dot, icon) {
        (0, 0) => None,
        (0, z) | (z, 0) => Some(z as Zoom),
        (d, i) => Some((if d < i { d } else { i }) as Zoom),
    }
}

/// A category's symbol, by its place among the masks
/// (asset/symbol.spec.rs); none for a category Carto draws no icon for.
pub fn symbol_of(kind: u16) -> Option<u16> {
    SYMBOL
        .get(usize::from(kind.checked_sub(1)?))?
        .checked_sub(1)
}

/// The number of a category by its word; 0 where the category list has none.
pub fn category_of(category: &str) -> u16 {
    CATEGORIES
        .iter()
        .position(|k| *k == category)
        .map_or(0, |i| i as u16 + 1)
}

/// A place as a source gives it: its own name — Carto labels a place by its
/// `name`, never translated — its category and the source's id for it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SourcePlace<'a> {
    pub name: &'a str,
    pub kind: u16,
    pub id: &'a str,
}

impl Tile for PlaceTile {
    const KIND: BaseKind = BaseKind::Place;

    fn occupancy(&self) -> Occupancy {
        self.occupancy
    }
}

/// One place of a tile, read from its lanes: what a source gave, the zoom
/// it is shown from, and the ground leaf cell it stands in, as an offset in
/// the tile.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Place<'a> {
    pub name: &'a str,
    pub kind: u16,
    pub id: &'a str,
    pub first_zoom: Zoom,
    pub off: [u32; 2],
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlaceTile {
    /// Each place's offset, ascending by row then column.
    offs: Vec<[u16; 2]>,
    kinds: Vec<u16>,
    first_zooms: Vec<u8>,
    names: StringTable,
    ids: StringTable,
    /// The children that are tiles: those holding a place this tile does not.
    pub occupancy: Occupancy,
}

impl PlaceTile {
    /// A tile of these places, in the order given.
    fn of<'a>(places: impl IntoIterator<Item = Place<'a>>, occupancy: Occupancy) -> PlaceTile {
        let mut tile = PlaceTile {
            occupancy,
            ..PlaceTile::default()
        };
        places.into_iter().for_each(|h| tile.push(&h));
        tile
    }

    pub fn len(&self) -> u32 {
        self.kinds.len() as u32
    }

    pub fn is_empty(&self) -> bool {
        self.kinds.is_empty()
    }

    fn get(&self, i: u32) -> Place<'_> {
        let k = i as usize;
        Place {
            name: self.names.get(i),
            kind: self.kinds[k],
            id: self.ids.get(i),
            first_zoom: Zoom::from(self.first_zooms[k]),
            off: self.offs[k].map(u32::from),
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = Place<'_>> + '_ {
        (0..self.len()).map(|i| self.get(i))
    }

    fn push(&mut self, h: &Place) {
        self.offs.push(h.off.map(|v| v as u16));
        self.kinds.push(h.kind);
        // The lane keeps a zoom in a byte.
        self.first_zooms.push(h.first_zoom as u8);
        self.names.push(h.name);
        self.ids.push(h.id);
    }

    /// A leaf from the places standing in it: those of a category Carto draws,
    /// placed at every zoom from the coarsest through Carto's last, a cell
    /// an icon wide keeping its first; names cut to `NAME_CHARS`.
    pub fn leaf<'a>(
        tile: Quadrant,
        places: impl IntoIterator<Item = (Q3, SourcePlace<'a>)>,
    ) -> PlaceTile {
        let names: Vec<(Cell, Zoom, String, SourcePlace<'a>)> = places
            .into_iter()
            .filter_map(|(point, place)| {
                Some((
                    GROUND.cell_of(LEAF_ZOOM, &point),
                    first_zoom(place.kind)?,
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
        let mut first_zooms: Vec<Option<Zoom>> = vec![None; names.len()];
        let mut taken = FnvHashSet::default();
        for zoom in PLACE_FIRST..=PLACE_LAST {
            taken.clear();
            for &p in &order {
                let (cell, shows, ..) = &names[p];
                if *shows <= zoom
                    && taken.insert(place_cell(*cell, zoom))
                    && first_zooms[p].is_none()
                {
                    first_zooms[p] = Some(zoom);
                }
            }
        }
        let placed = (0..names.len()).filter_map(|p| {
            let (cell, _, name, place) = &names[p];
            Some(Place {
                name: name.as_str(),
                kind: place.kind,
                id: place.id,
                first_zoom: first_zooms[p]?,
                off: cell.in_tile(),
            })
        });
        PlaceTile::of(by_position(placed), Occupancy::ZERO)
    }
}

/// The cell a place keeps at Carto's `zoom`, above the ground leaf cell it stands in; finer
/// than the ground's own cell, it is that leaf cell.
fn place_cell(cell: Cell, zoom: Zoom) -> Cell {
    cell.up(GROUND
        .leaf_cell_zoom()
        .saturating_sub(zoom + SPACING_STEP_CNT))
}

/// The ground leaf cell a place a tile holds stands in.
pub fn stands(tile: Quadrant, at: [u32; 2]) -> Cell {
    GROUND.cell_at(tile, 0).inner_at(GROUND, LEAF_ZOOM, at)
}

/// `p` from its four children, in `Quadrant::children`'s order: what of theirs is
/// shown from `p`'s zoom on. A child is a tile where it holds more, or has a
/// child that does, so a place first shown deeper stays reachable.
pub fn fold(p: Quadrant, children: [&PlaceTile; 4]) -> PlaceTile {
    let side = 1u32 << (GROUND.leaf_cell_zoom() - p.zoom - 1);
    let mut placed = Vec::new();
    let mut has_child = [false; 4];
    for (q, (k, child)) in p.children().into_iter().zip(children).enumerate() {
        let [cx, cy] = k.node().1.map(|v| (v & 1) as u32);
        has_child[q] = child.occupancy.bits() != 0 || child.iter().any(|h| h.first_zoom > p.zoom);
        placed.extend(
            child
                .iter()
                .filter(|h| h.first_zoom <= p.zoom)
                .map(|h| Place {
                    off: [cx * side + h.off[0], cy * side + h.off[1]],
                    ..h
                }),
        );
    }
    PlaceTile::of(by_position(placed), Occupancy::of_slots(has_child))
}

/// Places ascending by where they stand.
fn by_position<'a>(placed: impl IntoIterator<Item = Place<'a>>) -> Vec<Place<'a>> {
    let mut placed: Vec<Place> = placed.into_iter().collect();
    placed.sort_by_key(|h| [h.off[1], h.off[0]]);
    placed
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

pub fn encode(tile: &PlaceTile) -> Result<Vec<u8>, XputeError> {
    validate(tile)?;
    let offs: Vec<u16> = tile.offs.iter().flatten().copied().collect();
    let ids: Vec<&str> = tile.iter().map(|h| h.id).collect();
    let names: Vec<&str> = tile.iter().map(|h| h.name).collect();
    shape::encode::<Packet>(
        (
            &offs,
            &tile.kinds,
            &tile.first_zooms,
            &ids,
            &names,
            tile.occupancy.bits(),
        ),
        PKT_CAP,
    )
}

pub fn decode(pkt: &[u8]) -> Result<PlaceTile, XputeError> {
    let (offs, kinds, first_zooms, ids, names, occupancy) = shape::get_deep::<Packet>(pkt)?;
    let occupancy = occupancy_of(occupancy)?;
    let n = kinds.len();
    if offs.len() != 2 * n
        || first_zooms.len() != n
        || ids.len() as usize != n
        || names.len() as usize != n
    {
        return Err(malformed());
    }
    let places = names
        .iter()
        .zip(ids.iter())
        .enumerate()
        .map(|(i, (name, id))| Place {
            name,
            kind: kinds[i],
            id,
            first_zoom: Zoom::from(first_zooms[i]),
            off: [u32::from(offs[2 * i]), u32::from(offs[2 * i + 1])],
        });
    let tile = PlaceTile::of(places, occupancy);
    validate(&tile)?;
    Ok(tile)
}

/// What a tile holds: places ascending by where they stand, each in its own
/// cell; categories Carto draws; each shown from no sooner than its category and no
/// later than Carto's last zoom; names whole within `NAME_CHARS`.
fn validate(tile: &PlaceTile) -> Result<(), XputeError> {
    let ascending = tile
        .offs
        .windows(2)
        .all(|w| [w[0][1], w[0][0]] < [w[1][1], w[1][0]]);
    let bad = |h: Place| {
        let shows = first_zoom(h.kind);
        shows.is_none_or(|z| h.first_zoom < z || h.first_zoom > PLACE_LAST)
            || h.name.chars().nth(NAME_CHARS).is_some()
    };
    if !ascending || tile.iter().any(bad) {
        return Err(malformed());
    }
    Ok(())
}

#[cfg(test)]
#[path = "place.test.rs"]
mod test;
