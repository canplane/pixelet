// pxl-spec/bake/poi_page.rs

//! The POI layer's page. A place is 2.5D: it belongs to the cell of the POI
//! lattice it stands in (`POI_LATTICE`) and is drawn at that cell's centre.
//! So a page holds, for each occupied cell, how many places stand in it —
//! and, where that is one, which place it is.
//!
//! Folding is counting. A parent cell holds what its 2×2 children hold, which
//! needs no ring and loses nothing: every level counts exactly the places
//! under it, so a count read at any level agrees with the leaves. A name is
//! not a representative chosen for a cell: it is there only where the cell
//! holds one place, so it is that place's, and a parent holding one place
//! holds the name its one child held.
//!
//! Names are held once a page, in a pool the cells index. A cell is three
//! numbers whatever its name, and a name that stands in many cells — a
//! chain's, a station's — is carried once.

use std::collections::HashMap;

use crate::bake::cell_spec::CELL_SHAMT;
use crate::space::pyramid::{Folds, Lattice, LevelPage};

const MAGIC: &[u8; 4] = b"PXP3";

/// The lattice places are counted in: a cell four times the ground's on a
/// side, sharing the ground's pages, levels and names. A badge stands for
/// its cell, and a cell the ground's size is a few pixels on the screen at
/// every distance (pxl-render look/lod.rs), far under a badge. Four times is
/// a product choice, not a derivation: the scale places merge at.
pub const POI_LATTICE: Lattice = Lattice::new(CELL_SHAMT + 2);

const CELLS: u32 = POI_LATTICE.page_cells() * POI_LATTICE.page_cells();

/// A cell with no name: it holds more than one place, or its one place has
/// none.
pub const NO_NAME: u32 = u32::MAX;

/// An occupied cell: its index in the page, how many places stand in it, and
/// the pool index of its one place's name, NO_NAME unless `count` is 1.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PoiCell {
    pub cell: u32,
    pub count: u32,
    pub name: u32,
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
    /// A page from the cell each place stands in and its name, in any order.
    pub fn of_places<'a>(places: impl IntoIterator<Item = (u32, &'a str)>) -> PoiPage {
        gathered(places.into_iter().map(|(cell, name)| (cell, 1, name)).collect())
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
            cells.extend(child.cells.iter().map(|c| (POI_LATTICE.parent_cell(q, c.cell), c.count, child.name(c).unwrap_or(""))));
        }
        Some(gathered(cells))
    }
}

/// Cells ascending, each once, holding the sum of what it was given, and a
/// name where that sum is one.
fn gathered(mut cells: Vec<(u32, u32, &str)>) -> PoiPage {
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
        .map(|(cell, count, name)| PoiCell {
            cell,
            count,
            name: if count == 1 { pool.add(name) } else { NO_NAME },
        })
        .collect();
    PoiPage { cells, names: pool.names }
}

/// The magic; how many cells, names and bytes of names; the cells, three
/// words each; each name's end; the names.
pub fn encode(page: &PoiPage) -> Result<Vec<u8>, &'static str> {
    validate(page)?;
    let names = &page.names;
    let mut out = Vec::with_capacity(16 + page.cells.len() * 12 + names.ends.len() * 4 + names.bytes.len());
    out.extend_from_slice(MAGIC);
    for n in [page.cells.len() as u32, names.len(), names.bytes.len() as u32] {
        out.extend_from_slice(&n.to_le_bytes());
    }
    for c in &page.cells {
        for w in [c.cell, c.count, c.name] {
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
    if n > CELLS as usize || names > n || bytes.len() != 16 + n * 12 + names * 4 + pool {
        return Err("a POI page of the wrong length");
    }
    let cells = (0..n)
        .map(|i| PoiCell {
            cell: word(16 + i * 12),
            count: word(20 + i * 12),
            name: word(24 + i * 12),
        })
        .collect();
    let at = 16 + n * 12;
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
/// and a name only where a cell holds one place, from the pool.
fn validate(page: &PoiPage) -> Result<(), &'static str> {
    if page.cells.iter().any(|c| c.cell >= CELLS || c.count == 0) || page.cells.windows(2).any(|w| w[0].cell >= w[1].cell) {
        return Err("POI cells out of order or empty");
    }
    if page.cells.iter().any(|c| c.name != NO_NAME && (c.count != 1 || c.name >= page.names.len())) {
        return Err("a POI name where no one place is");
    }
    Ok(())
}

#[cfg(test)]
#[path = "poi_page.test.rs"]
mod test;
