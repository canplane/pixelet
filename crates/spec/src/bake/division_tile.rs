// pxl-spec/bake/division_tile.rs

//! The divisions layer's tile: per cell of the ground's lattice the finest
//! division it is in, and every division a cell names with each of its
//! ancestors, so a tile names its cells alone. Made at `DIVISION_LEAF`; each
//! coarser tile is the majority of the four under it, and a child is kept
//! only where it says more than its parent, which a reader that holds the
//! parent then reads instead.

use std::collections::HashMap;

use xpute_core::status::error::MarshalError;
use xpute_core::wire::xtp::{encoder, NodeView, TreeEncoderOptions, TreeReader, TreeView};

use crate::bake::fold::majority;
use crate::bake::format::{occupancy_of, Occupancy};
use crate::bake::malformed;
use crate::bake::named::{NameLanes, NamesView};
use crate::division::Subtype;
use crate::space::coord::zoom::Zoom;
use crate::space::layer::lattice::{GROUND, TILE_CELLS};

/// The finest tile Overture makes of its divisions (overture-tiles
/// `profiles/Divisions.java`, `MAXZOOM`): finer views read it.
pub const DIVISION_LEAF: Zoom = 12;

/// A cell in no division, and a division with no parent.
pub const NONE: u32 = u32::MAX;

const CELLS: usize = (TILE_CELLS * TILE_CELLS) as usize;

#[derive(Clone, Debug, PartialEq)]
pub struct Division {
    /// Overture's id: what tiles compare divisions by.
    pub id: String,
    pub subtype: Subtype,
    pub name: String,
    /// `(tag, name)` pairs.
    pub names: Vec<(String, String)>,
    /// Its parent's index in the tile, `NONE` for none.
    pub parent: u32,
    /// Overture's `cartography.prominence`, 1 to 100; 0 where it gives none.
    pub prominence: u8,
    /// Held by an area; one without is found by its point.
    pub area: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DivisionTile {
    /// Each cell's division, by `GROUND.cell_index`; `NONE` in none.
    pub cells: Vec<u32>,
    pub divisions: Vec<Division>,
    pub occupancy: Occupancy,
}

impl DivisionTile {
    /// The cell's division, then each ancestor, narrow to broad.
    fn up(&self, cell: u32) -> impl Iterator<Item = &Division> {
        let first = self.cells.get(cell as usize).copied().filter(|&c| c != NONE);
        core::iter::successors(first, |&i| Some(self.divisions[i as usize].parent).filter(|&p| p != NONE)).map(|i| &self.divisions[i as usize])
    }

    /// The cell's division and each ancestor, broad to narrow; none in none.
    /// Walked again for each, so reading a name asks the heap for nothing.
    pub fn name_at(&self, cell: u32) -> impl Iterator<Item = &Division> {
        let depth = self.up(cell).count();
        (0..depth).rev().filter_map(move |k| self.up(cell).nth(k))
    }
}

impl Default for DivisionTile {
    /// In no division: what a tile the set holds no object for is.
    fn default() -> DivisionTile {
        DivisionTile {
            cells: vec![NONE; CELLS],
            divisions: Vec::new(),
            occupancy: Occupancy::ZERO,
        }
    }
}

/// One branch: the cells; the occupancy; per division its id, subtype,
/// parent, prominence and whether an area holds it, then its names (`named`).
pub fn encode(tile: &DivisionTile) -> Result<Vec<u8>, MarshalError> {
    let d = &tile.divisions;
    let parents: Vec<u32> = d.iter().map(|d| d.parent).collect();
    check(&tile.cells, &parents)?;
    let ids: Vec<&str> = d.iter().map(|d| d.id.as_str()).collect();
    let subtypes: Vec<u8> = d.iter().map(|d| d.subtype as u8).collect();
    let prominence: Vec<u8> = d.iter().map(|d| d.prominence).collect();
    let area: Vec<u8> = d.iter().map(|d| d.area as u8).collect();
    let names = NameLanes::of(d.iter().map(|d| (d.name.as_str(), d.names.iter().map(|(tag, name)| (tag.as_str(), name.as_str())))));
    let mut t: TreeView<'_> = TreeView::new();
    t.branch(Some(&mut |b| {
        b.u32_array(Some(&tile.cells)).u8(Some(tile.occupancy.bits()));
        b.strs(Some(&ids)).u8_array(Some(&subtypes)).u32_array(Some(&parents)).u8_array(Some(&prominence)).u8_array(Some(&area));
        names.write(b);
    }));
    encoder().encode(&t, TreeEncoderOptions::default())
}

/// Every cell a division of the tile or none, every parent one or none, and
/// no walk up the parents a cycle.
fn check(cells: &[u32], parents: &[u32]) -> Result<(), MarshalError> {
    let n = parents.len();
    if n >= NONE as usize {
        return Err(malformed());
    }
    let fits = |i: u32| i == NONE || (i as usize) < n;
    if cells.len() != CELLS || !cells.iter().all(|&c| fits(c)) || !parents.iter().all(|&p| fits(p)) {
        return Err(malformed());
    }
    for start in 0..n {
        let mut at = start as u32;
        for step in 0.. {
            if step > n {
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
struct DivisionTileView<'a> {
    cells: &'a [u32],
    pub occupancy: Occupancy,
    ids: xpute_core::wire::xtp::Strs<'a>,
    subtypes: &'a [u8],
    parents: &'a [u32],
    prominence: &'a [u8],
    area: &'a [u8],
    names: NamesView<'a>,
}

fn open(pkt: &[u8]) -> Result<DivisionTileView<'_>, MarshalError> {
    let root = TreeReader::new(pkt)?.read_branch()?;
    let cells = root.at(0)?.get_array::<u32>()?;
    let occupancy = occupancy_of(root.at(1)?.get_number()?)?;
    let (ids, subtypes, parents, prominence, area) = (
        root.at(2)?.get_strs()?,
        root.at(3)?.get_array::<u8>()?,
        root.at(4)?.get_array::<u32>()?,
        root.at(5)?.get_array::<u8>()?,
        root.at(6)?.get_array::<u8>()?,
    );
    let n = subtypes.len();
    if ids.len() as usize != n || parents.len() != n || prominence.len() != n || area.len() != n || subtypes.iter().any(|&s| Subtype::of(s).is_none()) {
        return Err(malformed());
    }
    check(cells, parents)?;
    Ok(DivisionTileView {
        cells,
        occupancy,
        ids,
        subtypes,
        parents,
        prominence,
        area,
        names: NamesView::read(&root, 7, n)?,
    })
}

/// One division as the packet holds it, read while decoding.
#[derive(Clone, Copy)]
struct DivisionRef<'a> {
    subtype: Subtype,
    name: &'a str,
    prominence: u8,
    approx: bool,
    names: NamesView<'a>,
    at: u32,
}

impl<'a> DivisionRef<'a> {
    fn languages(&self) -> impl Iterator<Item = (&'a str, &'a str)> {
        self.names.languages(self.at)
    }
}

impl<'a> DivisionTileView<'a> {
    fn at(&self, i: u32) -> Option<DivisionRef<'a>> {
        let k = i as usize;
        Some(DivisionRef {
            subtype: Subtype::of(*self.subtypes.get(k)?)?,
            name: self.names.name(i)?,
            prominence: self.prominence[k],
            approx: self.area[k] == 0,
            names: self.names,
            at: i,
        })
    }

    fn id(&self, i: u32) -> Option<&'a str> {
        self.ids.get(i)
    }
}

pub fn decode(pkt: &[u8]) -> Result<DivisionTile, MarshalError> {
    let v = open(pkt)?;
    let divisions = (0..v.subtypes.len())
        .map(|k| {
            let i = k as u32;
            let d = v.at(i).ok_or_else(malformed)?;
            Ok(Division {
                id: v.id(i).ok_or_else(malformed)?.to_string(),
                subtype: d.subtype,
                name: d.name.to_string(),
                names: d.languages().map(|(t, n)| (t.to_string(), n.to_string())).collect(),
                parent: v.parents[k],
                prominence: d.prominence,
                area: !d.approx,
            })
        })
        .collect::<Result<Vec<_>, MarshalError>>()?;
    Ok(DivisionTile {
        cells: v.cells.to_vec(),
        divisions,
        occupancy: v.occupancy,
    })
}

/// One zoom coarser, at `zoom`: each cell the majority of the 2×2 under it,
/// compared by id; a child is kept where it has children of its own or its
/// cells are not its parent's, each repeated.
pub fn fold_division(children: [&DivisionTile; 4], zoom: Zoom) -> DivisionTile {
    // every child's divisions under one numbering, by id
    let mut all: Vec<Division> = Vec::new();
    let mut by_id: HashMap<String, u32> = HashMap::new();
    let mut local: [Vec<u32>; 4] = Default::default();
    for (q, child) in children.iter().enumerate() {
        local[q] = child
            .divisions
            .iter()
            .map(|d| {
                *by_id.entry(d.id.clone()).or_insert_with(|| {
                    all.push(d.clone());
                    (all.len() - 1) as u32
                })
            })
            .collect();
    }
    for (q, child) in children.iter().enumerate() {
        for (k, d) in child.divisions.iter().enumerate() {
            let at = local[q][k] as usize;
            all[at].parent = if d.parent == NONE { NONE } else { local[q][d.parent as usize] };
        }
    }
    let global = |q: usize, c: u32| if c == NONE { NONE } else { local[q][c as usize] };

    let cells: Vec<u32> = (0..CELLS as u32)
        .map(|index| {
            let (q, block) = GROUND.child_cells(index);
            majority(block.map(|i| global(q, children[q].cells[i as usize])), GROUND.cell_zoom(zoom))
        })
        .collect();
    let kept: [bool; 4] = std::array::from_fn(|q| {
        let child = children[q];
        child.occupancy.bits() != 0 || (0..CELLS as u32).any(|c| global(q, child.cells[c as usize]) != cells[GROUND.parent_cell(q, c) as usize])
    });

    // only the divisions a cell names, and their ancestors, renumbered
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
        divisions.push(d.clone());
    }
    for d in &mut divisions {
        d.parent = if d.parent == NONE { NONE } else { renumber[d.parent as usize] };
    }
    DivisionTile {
        cells: cells.into_iter().map(|c| if c == NONE { NONE } else { renumber[c as usize] }).collect(),
        divisions,
        occupancy: Occupancy::of_slots(kept),
    }
}
