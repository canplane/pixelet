// pxl-spec/bake/poi_page.rs

//! The POI layer's page. A place is 2.5D: it belongs to the cell of the POI
//! lattice it stands in (`POI_LATTICE`). A cell holding two places or more is
//! drawn as their count at its own centre; a cell holding one is that place,
//! drawn where it stands — at the centre of the ground's z16 cell it is
//! in, at every level. So a page holds, for each occupied cell, how many
//! places stand in it and, where that is one, which place it is and where.
//!
//! Folding is counting. A parent cell holds what its 2×2 children hold, which
//! needs no ring and loses nothing: every level counts exactly the places
//! under it, so a count read at any level agrees with the leaves. A name and
//! a position are not a representative chosen for a cell: they are there
//! only where the cell holds one place, so they are that place's, and a
//! parent holding one place holds what its one child held.
//!
//! Names are held once a page, in a pool the cells index. A cell is three
//! numbers whatever its name, and a name that stands in many cells — a
//! chain's, a station's — is carried once.

use std::collections::HashMap;

use crate::space::pvf3::pvf3::Pvf3;
use crate::space::pyramid::{Folds, Lattice, LevelPage, GROUND, TOP_LEVEL};

const MAGIC: &[u8; 4] = b"PXP4";

/// The lattice places are counted in: a cell sixteen times the ground's on a
/// side — four levels over it, so a POI page's cells at z16 are z12's —
/// sharing the ground's pages, levels and names. A badge stands for its
/// cell, and a cell the ground's size is a few pixels on the screen at every
/// distance (pxl-render look/lod.rs), far under a badge. Sixteen times is a
/// product choice, not a derivation: the scale places merge at, the layer's
/// own detail at a level the shared rule chooses.
pub const POI_LATTICE: Lattice = Lattice::new(GROUND.scale(4));

const CELLS: u32 = POI_LATTICE.page_cells() * POI_LATTICE.page_cells();

/// A cell on the wire: its index, count, position and name, a word each.
const CELL_BYTES: usize = 16;

/// A cell with no name: it holds more than one place, or its one place has
/// none.
pub const NO_NAME: u32 = u32::MAX;

/// A cell with no position: it holds more than one place.
const NO_AT: u32 = u32::MAX;

// A position is an offset in z16 ground cells inside a POI cell, a lane
// a half-word: the widest cell, at the top level, has to fit.
const _: () = assert!(
    POI_LATTICE.scale(TOP_LEVEL).above(GROUND.scale(0)) <= 16,
    "a POI cell's z16 ground cells no longer fit a half-word a lane"
);

/// An occupied cell: its index in the page, how many places stand in it, and
/// where `count` is 1, the pool index of its one place's name (NO_NAME where
/// it has none) and the z16 ground cell it stands in, as that cell's
/// offset in this one (pxl-spec space/pyramid.rs `Cell::offset_of`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PoiCell {
    pub cell: u32,
    pub count: u32,
    pub name: u32,
    pub at: Option<[u32; 2]>,
}

/// A page's names, each once, as one run of UTF-8: name i is
/// `bytes[ends[i - 1]..ends[i]]`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Names {
    bytes: Vec<u8>,
    ends: Vec<u32>,
}

impl Names {
    /// Name `i`; empty where there is none.
    pub fn get(&self, i: u32) -> &str {
        let Some(&end) = self.ends.get(i as usize) else { return "" };
        let start = if i == 0 { 0 } else { self.ends[i as usize - 1] };
        // Checked to be UTF-8 name by name on the way in (`decode`), or built
        // from `&str`.
        std::str::from_utf8(&self.bytes[start as usize..end as usize]).unwrap_or("")
    }

    fn len(&self) -> u32 {
        self.ends.len() as u32
    }
}

/// A pool being filled, each name once.
#[derive(Default)]
struct Pool<'a> {
    names: Names,
    at: HashMap<&'a str, u32>,
}

impl<'a> Pool<'a> {
    fn add(&mut self, name: &'a str) -> u32 {
        if name.is_empty() {
            return NO_NAME;
        }
        let names = &mut self.names;
        *self.at.entry(name).or_insert_with(|| {
            names.bytes.extend_from_slice(name.as_bytes());
            names.ends.push(names.bytes.len() as u32);
            names.ends.len() as u32 - 1
        })
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PoiPage {
    /// Occupied cells by the pyramid's cell index, ascending.
    pub cells: Vec<PoiCell>,
    pub names: Names,
}

impl PoiPage {
    /// A level-`level` page from where each place stands and its name, in
    /// any order. Every place has to stand in the one page.
    pub fn of_places<'a>(level: u32, places: impl IntoIterator<Item = (Pvf3, &'a str)>) -> PoiPage {
        gathered(
            places
                .into_iter()
                .map(|(point, name)| {
                    let cell = POI_LATTICE.cell_of(level, &point);
                    (cell.index(), 1, cell.offset_of(GROUND.cell_of(0, &point)), name)
                })
                .collect(),
        )
    }

    /// The name of the one place `c` holds; none where it holds more, or its
    /// one place has none.
    pub fn name(&self, c: &PoiCell) -> Option<&str> {
        (c.count == 1 && c.name != NO_NAME).then(|| self.names.get(c.name))
    }
}

impl Folds for PoiPage {
    fn fold_page<'a>(p: LevelPage, held: impl Fn(LevelPage) -> Option<&'a Self>) -> Option<Self> {
        let mut cells = Vec::new();
        for (q, k) in p.children().into_iter().enumerate() {
            let child = held(k)?;
            cells.extend(child.cells.iter().map(|c| {
                let parent = POI_LATTICE.parent_cell(q, c.cell);
                // The one place's ground cell, found again from the child's
                // cell and read from the parent's.
                let at = c.at.map_or([0; 2], |at| {
                    let ground = POI_LATTICE.cell_at(k, c.cell).inner_at(GROUND, 0, at);
                    POI_LATTICE.cell_at(p, parent).offset_of(ground)
                });
                (parent, c.count, at, child.name(c).unwrap_or(""))
            }));
        }
        Some(gathered(cells))
    }
}

/// Cells ascending, each once, holding the sum of what it was given, and a
/// name and a position where that sum is one.
fn gathered(mut cells: Vec<(u32, u32, [u32; 2], &str)>) -> PoiPage {
    cells.sort_unstable_by_key(|c| c.0);
    cells.dedup_by(|next, kept| {
        let same = next.0 == kept.0;
        if same {
            kept.1 += next.1;
        }
        same
    });
    let mut pool = Pool::default();
    let cells = cells
        .into_iter()
        .map(|(cell, count, at, name)| PoiCell {
            cell,
            count,
            name: if count == 1 { pool.add(name) } else { NO_NAME },
            at: (count == 1).then_some(at),
        })
        .collect();
    PoiPage { cells, names: pool.names }
}

/// The magic; how many cells, names and bytes of names; the cells, four
/// words each (the position's two lanes a half-word each); each name's end;
/// the names.
pub fn encode(page: &PoiPage) -> Result<Vec<u8>, &'static str> {
    validate(page)?;
    let names = &page.names;
    let mut out = Vec::with_capacity(16 + page.cells.len() * CELL_BYTES + names.ends.len() * 4 + names.bytes.len());
    out.extend_from_slice(MAGIC);
    for n in [page.cells.len() as u32, names.len(), names.bytes.len() as u32] {
        out.extend_from_slice(&n.to_le_bytes());
    }
    for c in &page.cells {
        let at = c.at.map_or(NO_AT, |[x, y]| x | y << 16);
        for w in [c.cell, c.count, at, c.name] {
            out.extend_from_slice(&w.to_le_bytes());
        }
    }
    for &end in &names.ends {
        out.extend_from_slice(&end.to_le_bytes());
    }
    out.extend_from_slice(&names.bytes);
    Ok(out)
}

pub fn decode(bytes: &[u8]) -> Result<PoiPage, &'static str> {
    if bytes.len() < 16 || &bytes[..4] != MAGIC {
        return Err("not a POI page");
    }
    let word = |at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
    let (n, names, pool) = (word(4) as usize, word(8) as usize, word(12) as usize);
    // Checked against the page's own bound before anything is allocated.
    if n > CELLS as usize || names > n || bytes.len() != 16 + n * CELL_BYTES + names * 4 + pool {
        return Err("a POI page of the wrong length");
    }
    let cells = (0..n)
        .map(|i| {
            let (at, w) = (16 + i * CELL_BYTES, word(24 + i * CELL_BYTES));
            PoiCell {
                cell: word(at),
                count: word(at + 4),
                at: (w != NO_AT).then_some([w & 0xffff, w >> 16]),
                name: word(at + 12),
            }
        })
        .collect();
    let at = 16 + n * CELL_BYTES;
    let ends: Vec<u32> = (0..names).map(|i| word(at + i * 4)).collect();
    let text = &bytes[at + names * 4..];
    if ends.windows(2).any(|w| w[0] > w[1]) || ends.last().map_or(0, |&e| e as usize) != text.len() {
        return Err("POI names out of order");
    }
    // Name by name: a pool that is UTF-8 whole could still split a character
    // between two names.
    let mut start = 0;
    for &end in &ends {
        std::str::from_utf8(&text[start..end as usize]).map_err(|_| "a POI name that is not UTF-8")?;
        start = end as usize;
    }
    let page = PoiPage {
        cells,
        names: Names { bytes: text.to_vec(), ends },
    };
    validate(&page)?;
    Ok(page)
}

/// A page is its cells ascending, each inside the page and holding a place,
/// and a name and a position only where a cell holds one place, the name
/// from the pool.
fn validate(page: &PoiPage) -> Result<(), &'static str> {
    if page.cells.iter().any(|c| c.cell >= CELLS || c.count == 0) || page.cells.windows(2).any(|w| w[0].cell >= w[1].cell) {
        return Err("POI cells out of order or empty");
    }
    if page.cells.iter().any(|c| c.name != NO_NAME && (c.count != 1 || c.name >= page.names.len())) {
        return Err("a POI name where no one place is");
    }
    if page.cells.iter().any(|c| c.at.is_some() != (c.count == 1) || c.at.is_some_and(|a| a.iter().any(|&l| l > 0xffff))) {
        return Err("a POI position where no one place is, or none where one is");
    }
    Ok(())
}

#[cfg(test)]
#[path = "poi_page.test.rs"]
mod test;
