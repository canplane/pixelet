// pxl-spec/bake/tile/division.rs

//! The divisions layer's tile: per cell of the ground's lattice the finest
//! division it is in, and every division a cell names with each of its
//! ancestors, so a tile names its cells alone. Made at `DIVISION_LEAF`;
//! each coarser tile is the majority of the four under it, and a child is kept
//! only where it says more than its parent, which a reader that holds the
//! parent then reads instead.

use crate::space::layer::lattice::{Grid, GROUND, TILE_CELLS};
use fnv::FnvHashMap;
use std::borrow::Cow;

use xpute_kit::status::error::MarshalError;
use xpute_kit::wire::xtp::shape;

use crate::bake::malformed;
use crate::bake::publication::BaseLayer;
use crate::bake::tile::{occupancy_of, Occupancy, StringTable, Tile};
use crate::division::{Subtype, NAME_BYTES};
use crate::ground::majority;
use crate::space::coord::adapter::epsg3857::cell_zoom_of;
use crate::space::coord::zoom::Zoom;
use xpute_kit::wire::xtp::StrList;

/// The finest tile Overture makes of its divisions (overture-tiles
/// `profiles/Divisions.java`, `MAXZOOM`): finer views read it.
pub const DIVISION_LEAF: Zoom = 12;

/// The most a packet takes, a tile past it failing its bake (`encode`).
pub(crate) const MAX_PKT_BYTES: usize = 4 << 20;

/// A cell in no division, and a division with no parent.
pub const NONE: u32 = u32::MAX;

/// A tile's cells, and so the room a client decodes them into.
pub const CELLS: usize = (TILE_CELLS * TILE_CELLS) as usize;

/// The cells; the occupancy; per division its id, kind, parent, prominence
/// and whether an area holds it; its name.
type Packet = (
    Vec<u32>,
    u8,
    Vec<String>,
    Vec<u8>,
    Vec<u32>,
    Vec<u8>,
    Vec<u8>,
    Vec<String>,
);

/// One division of a tile, read from its columns.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Division<'a> {
    /// Overture's id: what tiles compare divisions by.
    pub id: &'a str,
    pub kind: Subtype,
    pub name: &'a str,
    /// Its parent's index in the tile, `NONE` for none.
    pub parent: u32,
    /// Overture's `cartography.prominence`, 1 to 100; 0 where it gives none.
    pub prominence: u8,
    /// Held by an area; one without is found by its point.
    pub area: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DivisionTile {
    /// Each cell's division, by `GROUND.cell_idx`; `NONE` in none. The bake owns its cells; a
    /// client's are borrowed from the room it decoded them into.
    pub cells: Cow<'static, [u32]>,
    ids: StringTable,
    kinds: Vec<Subtype>,
    names: StringTable,
    parents: Vec<u32>,
    prominences: Vec<u8>,
    areas: Vec<bool>,
    pub occupancy: Occupancy,
}

impl Tile for DivisionTile {
    const LAYER: BaseLayer = BaseLayer::division;

    fn occupancy(&self) -> Occupancy {
        self.occupancy
    }
}

impl Division<'_> {
    /// The zoom its name shows from, as Overture's own tiles place it
    /// (overture-tiles `profiles/Divisions.java`): a web map's zoom by its
    /// prominence where it has one, else by its kind, whose pixel is the
    /// cell at the zoom returned (`epsg3857::cell_zoom_of`).
    pub fn shown_from(&self) -> Zoom {
        let web = match (self.prominence, self.kind) {
            (1.., _) => (100u8.saturating_sub(self.prominence) / 5 + 1).min(10),
            (0, Subtype::Country | Subtype::Dependency) => 0,
            (0, Subtype::Region) => 4,
            (0, Subtype::County) => 8,
            (0, _) => 10,
        };
        cell_zoom_of(i32::from(web))
    }
}

impl DivisionTile {
    /// A tile of these cells and divisions.
    pub fn of<'a>(
        cells: Cow<'static, [u32]>,
        divisions: impl IntoIterator<Item = Division<'a>>,
        occupancy: Occupancy,
    ) -> DivisionTile {
        let mut tile = DivisionTile {
            cells,
            ids: StringTable::default(),
            kinds: Vec::new(),
            names: StringTable::default(),
            parents: Vec::new(),
            prominences: Vec::new(),
            areas: Vec::new(),
            occupancy,
        };
        divisions.into_iter().for_each(|d| tile.push(&d));
        tile
    }

    fn push(&mut self, d: &Division) {
        self.ids.push(d.id);
        self.kinds.push(d.kind);
        self.names.push(d.name);
        self.parents.push(d.parent);
        self.prominences.push(d.prominence);
        self.areas.push(d.area);
    }

    fn len(&self) -> u32 {
        self.kinds.len() as u32
    }

    fn get(&self, i: u32) -> Division<'_> {
        let k = i as usize;
        Division {
            id: self.ids.get(i),
            kind: self.kinds[k],
            name: self.names.get(i),
            parent: self.parents[k],
            prominence: self.prominences[k],
            area: self.areas[k],
        }
    }

    fn iter(&self) -> impl Iterator<Item = Division<'_>> + '_ {
        (0..self.len()).map(|i| self.get(i))
    }

    /// The cell's division, then each ancestor, narrow to broad.
    fn up(&self, cell: u32) -> impl Iterator<Item = Division<'_>> {
        let first = self
            .cells
            .get(cell as usize)
            .copied()
            .filter(|&c| c != NONE);
        core::iter::successors(first, |&i| {
            Some(self.parents[i as usize]).filter(|&p| p != NONE)
        })
        .map(|i| self.get(i))
    }

    /// The cell's division and each ancestor, broad to narrow; none in none.
    /// Walked again for each, so reading a name asks the heap for nothing.
    pub fn name_at(&self, cell: u32) -> impl Iterator<Item = Division<'_>> {
        let depth = self.up(cell).count();
        (0..depth).rev().filter_map(move |k| self.up(cell).nth(k))
    }
}

impl Default for DivisionTile {
    /// A tile in no division, which is what a tile the set holds no object for is.
    fn default() -> DivisionTile {
        DivisionTile::of(vec![NONE; CELLS].into(), [], Occupancy::ZERO)
    }
}

pub fn encode(tile: &DivisionTile) -> Result<Vec<u8>, MarshalError> {
    check(&tile.cells, &tile.parents)?;
    let ids: Vec<&str> = tile.iter().map(|d| d.id).collect();
    let kinds: Vec<u8> = tile.kinds.iter().map(|&k| k as u8).collect();
    let areas: Vec<u8> = tile.areas.iter().map(|&a| a as u8).collect();
    let names: Vec<&str> = tile.iter().map(|d| d.name).collect();
    check_names(names.iter().copied())?;
    shape::encode::<Packet>(
        (
            &tile.cells,
            tile.occupancy.bits(),
            &ids,
            &kinds,
            &tile.parents,
            &tile.prominences,
            &areas,
            &names,
        ),
        MAX_PKT_BYTES as u32,
    )
}

/// Every name within NAME_BYTES.
fn check_names<'a>(mut names: impl Iterator<Item = &'a str>) -> Result<(), MarshalError> {
    if names.any(|n| n.len() > NAME_BYTES) {
        return Err(malformed());
    }
    Ok(())
}

/// Every cell a division of the tile or none, every parent one or none, and
/// no walk up the parents a cycle or longer than a division of each subtype.
fn check(cells: &[u32], parents: &[u32]) -> Result<(), MarshalError> {
    let n = parents.len();
    if n >= NONE as usize {
        return Err(malformed());
    }
    let fits = |i: u32| i == NONE || (i as usize) < n;
    if cells.len() != CELLS || !cells.iter().all(|&c| fits(c)) || !parents.iter().all(|&p| fits(p))
    {
        return Err(malformed());
    }
    for start in 0..n {
        let mut at = start as u32;
        for step in 0.. {
            if step >= Subtype::ALL.len() {
                return Err(malformed());
            }
            match parents[at as usize] {
                NONE => break,
                p => at = p,
            }
        }
    }
    Ok(())
}

/// A tile read where it lies; names are never decoded into strings.
#[derive(Clone, Copy)]
struct Table<'a> {
    cells: &'a [u32],
    occupancy: Occupancy,
    ids: StrList<'a>,
    kinds: &'a [u8],
    parents: &'a [u32],
    prominences: &'a [u8],
    areas: &'a [u8],
    names: StrList<'a>,
}

fn read_tile(pkt: &[u8]) -> Result<Table<'_>, MarshalError> {
    let (cells, occupancy, ids, kinds, parents, prominences, areas, names) =
        shape::get_deep::<Packet>(pkt)?;
    let occupancy = occupancy_of(occupancy)?;
    let n = kinds.len();
    if ids.len() as usize != n
        || parents.len() != n
        || prominences.len() != n
        || areas.len() != n
        || names.len() as usize != n
        || kinds.iter().any(|&k| Subtype::of(k).is_none())
    {
        return Err(malformed());
    }
    check(cells, parents)?;
    check_names(names.iter())?;
    Ok(Table {
        cells,
        occupancy,
        ids,
        kinds,
        parents,
        prominences,
        areas,
        names,
    })
}

/// Decodes a tile into a copy, as the bake holds it.
pub fn decode(pkt: &[u8]) -> Result<DivisionTile, MarshalError> {
    let v = read_tile(pkt)?;
    Ok(DivisionTile {
        cells: v.cells.to_vec().into(),
        ..tile_of(&v)
    })
}

/// Decoded straight into its resident room, `CELLS` long: its cells; its
/// divisions, a handful, on the heap.
pub fn decode_into(pkt: &[u8], room: &'static mut [u32]) -> Result<DivisionTile, MarshalError> {
    let v = read_tile(pkt)?;
    if room.len() != CELLS {
        return Err(malformed());
    }
    let tile = tile_of(&v);
    room.copy_from_slice(v.cells);
    Ok(DivisionTile {
        cells: Cow::Borrowed(room),
        ..tile
    })
}

/// The divisions and the occupancy, the cells left empty.
fn tile_of(v: &Table<'_>) -> DivisionTile {
    let kinds = v.kinds.iter().filter_map(|&k| Subtype::of(k));
    let divisions =
        v.ids
            .iter()
            .zip(kinds)
            .zip(v.names.iter())
            .enumerate()
            .map(|(k, ((id, kind), name))| Division {
                id,
                kind,
                name,
                parent: v.parents[k],
                prominence: v.prominences[k],
                area: v.areas[k] != 0,
            });
    DivisionTile::of(Cow::Borrowed(&[]), divisions, v.occupancy)
}

/// One zoom coarser, at `zoom`: each cell the majority of the 2×2 under it,
/// compared by id; a child is kept where it has children of its own or its
/// cells are not its parent's, each repeated.
pub fn fold(children: [&DivisionTile; 4], zoom: Zoom) -> DivisionTile {
    // Every child's divisions under one numbering, by id.
    let mut all: Vec<Division> = Vec::new();
    let mut by_id: FnvHashMap<&str, u32> = FnvHashMap::default();
    let mut local: [Vec<u32>; 4] = Default::default();
    for (q, child) in children.iter().enumerate() {
        local[q] = child
            .iter()
            .map(|d| {
                *by_id.entry(d.id).or_insert_with(|| {
                    all.push(d);
                    (all.len() - 1) as u32
                })
            })
            .collect();
    }
    for (q, child) in children.iter().enumerate() {
        for (k, d) in child.iter().enumerate() {
            let at = local[q][k] as usize;
            all[at].parent = if d.parent == NONE {
                NONE
            } else {
                local[q][d.parent as usize]
            };
        }
    }
    let global = |q: usize, c: u32| {
        if c == NONE {
            NONE
        } else {
            local[q][c as usize]
        }
    };

    let cells: Vec<u32> = (0..CELLS as u32)
        .map(|idx| {
            let (q, block) = GROUND.child_cells(idx);
            majority(
                block.map(|i| global(q, children[q].cells[i as usize])),
                GROUND.cell_zoom(zoom),
            )
        })
        .collect();
    let has_child: [bool; 4] = std::array::from_fn(|q| {
        let child = children[q];
        child.occupancy.bits() != 0
            || (0..CELLS as u32).any(|c| {
                global(q, child.cells[c as usize]) != cells[GROUND.parent_cell(q, c) as usize]
            })
    });

    // Only the divisions a cell names, and their ancestors, renumbered.
    let mut used = vec![false; all.len()];
    for &c in cells.iter().filter(|&&c| c != NONE) {
        let mut at = c;
        while at != NONE && !used[at as usize] {
            used[at as usize] = true;
            at = all[at as usize].parent;
        }
    }
    let mut renumber = vec![NONE; all.len()];
    let mut divisions = Vec::new();
    for (k, d) in all.iter().enumerate().filter(|&(k, _)| used[k]) {
        renumber[k] = divisions.len() as u32;
        divisions.push(*d);
    }
    for d in &mut divisions {
        d.parent = if d.parent == NONE {
            NONE
        } else {
            renumber[d.parent as usize]
        };
    }
    DivisionTile::of(
        cells
            .into_iter()
            .map(|c| {
                if c == NONE {
                    NONE
                } else {
                    renumber[c as usize]
                }
            })
            .collect::<Vec<_>>()
            .into(),
        divisions,
        Occupancy::of_slots(has_child),
    )
}

#[cfg(test)]
#[path = "division.test.rs"]
mod test;
