// pxl-spec/bake/poi_page.rs

//! The POI layer's page. A place is 2.5D: it belongs to the ground cell it
//! stands in and is drawn at that cell's centre, at whatever level the ground
//! under it is drawn. So a page holds, for each occupied cell, how many places
//! stand in it.
//!
//! Folding is counting. A parent cell holds what its 2×2 children hold, which
//! needs no ring and loses nothing: every level counts exactly the places
//! under it, so a count read at any level agrees with the leaves.

use crate::space::pyramid::{parent_cell, Folds, CHILDREN, PAGE_CELLS};

const MAGIC: &[u8; 4] = b"PXP1";
const CELLS: u32 = PAGE_CELLS * PAGE_CELLS;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PoiPage {
    /// Occupied cells by the pyramid's cell index, ascending, each with the
    /// number of places standing in it.
    pub cells: Vec<(u32, u32)>,
}

impl PoiPage {
    /// A page from the cell each place stands in, in any order.
    pub fn of_places(places: impl IntoIterator<Item = u32>) -> PoiPage {
        PoiPage {
            cells: counted(places.into_iter().map(|cell| (cell, 1)).collect()),
        }
    }

    /// The places in `cell`, 0 where it holds none.
    pub fn count(&self, cell: u32) -> u32 {
        self.cells.binary_search_by_key(&cell, |c| c.0).map_or(0, |i| self.cells[i].1)
    }
}

impl Folds for PoiPage {
    fn fold_held(around: [Option<&Self>; 16], _level: u32) -> Option<Self> {
        let mut cells = Vec::new();
        for (q, &k) in CHILDREN.iter().enumerate() {
            cells.extend(around[k]?.cells.iter().map(|&(cell, n)| (parent_cell(q, cell), n)));
        }
        Some(PoiPage { cells: counted(cells) })
    }
}

/// Cells ascending, each once, holding the sum of what it was given.
fn counted(mut cells: Vec<(u32, u32)>) -> Vec<(u32, u32)> {
    cells.sort_unstable_by_key(|c| c.0);
    cells.dedup_by(|next, kept| {
        let same = next.0 == kept.0;
        if same {
            kept.1 += next.1;
        }
        same
    });
    cells
}

pub fn encode(page: &PoiPage) -> Result<Vec<u8>, &'static str> {
    validate(page)?;
    let mut out = Vec::with_capacity(8 + page.cells.len() * 8);
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&(page.cells.len() as u32).to_le_bytes());
    for &(cell, n) in &page.cells {
        out.extend_from_slice(&cell.to_le_bytes());
        out.extend_from_slice(&n.to_le_bytes());
    }
    Ok(out)
}

pub fn decode(bytes: &[u8]) -> Result<PoiPage, &'static str> {
    if bytes.len() < 8 || &bytes[..4] != MAGIC {
        return Err("not a POI page");
    }
    let word = |at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
    let n = word(4) as usize;
    // Checked against the page's own bound before anything is allocated.
    if n > CELLS as usize || bytes.len() != 8 + n * 8 {
        return Err("a POI page of the wrong length");
    }
    let page = PoiPage {
        cells: (0..n).map(|i| (word(8 + i * 8), word(12 + i * 8))).collect(),
    };
    validate(&page)?;
    Ok(page)
}

/// A page is its cells ascending, each inside the page and holding a place.
fn validate(page: &PoiPage) -> Result<(), &'static str> {
    if page.cells.iter().any(|c| c.0 >= CELLS || c.1 == 0) || page.cells.windows(2).any(|w| w[0].0 >= w[1].0) {
        return Err("POI cells out of order or empty");
    }
    Ok(())
}
