// pxl-spec/bake/poi_page.rs

//! The POI layer's page. A place is 2.5D: it belongs to the cell of the POI
//! lattice it stands in (`POI_LATTICE`). A cell stands where its places stand
//! on average — their centroid, as the z16 ground cell it falls in — so a
//! cell holding one place stands on that place's own, at every level, and a
//! cell of two or more stands among them rather than on the lattice's grid.
//! A page holds, for each occupied cell, how many places stand in it, where
//! they stand on average, and where that is one, which place it is.
//!
//! Folding is counting. A parent cell holds what its 2×2 children hold, which
//! needs no ring and loses nothing: every level counts exactly the places
//! under it, so a count read at any level agrees with the leaves. A centroid
//! folds the same way, as its children's weighted by their counts, rounded to
//! a z16 cell at each level. A place is not a representative chosen for a
//! cell: it is there only where the cell holds one, so it is that place, and a
//! parent holding one place holds what its one child held.
//!
//! Which place a cell holds is its name and its kind (bake/poi_theme.json),
//! held once a page in a pool the cells index: both are the one place's and
//! nothing else's, so they fold together. A cell is four words whatever its
//! place, and a place that stands in many cells — a chain's branch, a
//! station's exits — is carried once.

use std::collections::HashMap;

use crate::bake::poi_theme::{PoiTheme, DEFAULT_THEME, KINDS};
use crate::space::pvf3::pvf3::Pvf3;
use crate::space::pyramid::{Folds, Lattice, LevelPage, GROUND, TOP_LEVEL};

const MAGIC: &[u8; 4] = b"PXP5";

/// The lattice places are counted in: a cell sixteen times the ground's on a
/// side — four levels over it, so a POI page's cells at z16 are z12's —
/// sharing the ground's pages, levels and names. A badge stands for its
/// cell, and a cell the ground's size is a few pixels on the screen at every
/// distance (pxl-render look/lod.rs), far under a badge. Sixteen times is a
/// product choice, not a derivation: the scale places merge at, the layer's
/// own detail at a level the shared rule chooses.
pub const POI_LATTICE: Lattice = Lattice::new(GROUND.scale(4));

const CELLS: u32 = POI_LATTICE.page_cells() * POI_LATTICE.page_cells();

/// A cell on the wire: its index, count, position and place, a word each.
const CELL_BYTES: usize = 16;

/// A cell with no place: it holds more than one, or its one place has no
/// name.
pub const NO_PLACE: u32 = u32::MAX;

/// The longest name a page holds, in code points; a longer one is cut to end
/// in an ellipsis inside it. A product number, and a bound: at 32, 99% of the
/// places Overture names in Korea are whole (the 99th percentile of their
/// names' lengths, measured 2026-09-25, 1 in 107 longer), and what a page
/// can hold is bounded by its cells, whatever the data.
pub const NAME_CHARS: usize = 32;

// A position is an offset in z16 ground cells inside a POI cell, a lane
// a half-word: the widest cell, at the top level, has to fit.
const _: () = assert!(
    POI_LATTICE.scale(TOP_LEVEL).above(GROUND.scale(0)) <= 16,
    "a POI cell's z16 ground cells no longer fit a half-word a lane"
);

/// An occupied cell: its index in the page, how many places stand in it, the
/// z16 ground cell their centroid falls in, as that cell's offset in this one
/// (pxl-spec space/pyramid.rs `Cell::offset_of`), and where `count` is 1, the
/// pool index of its one place (NO_PLACE where it has no name).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PoiCell {
    pub cell: u32,
    pub count: u32,
    pub place: u32,
    pub at: [u32; 2],
}

/// A place as a cell names it: what it is called, and its kind — its number
/// in the theme, 0 for none (`kind_of`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Place<'a> {
    pub name: &'a str,
    pub kind: u8,
}

/// A page's places, each once: place i's name is `bytes[ends[i - 1]..ends[i]]`,
/// one run of UTF-8, and its kind `kinds[i]`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Places {
    bytes: Vec<u8>,
    ends: Vec<u32>,
    kinds: Vec<u8>,
}

impl Places {
    /// Place `i`; nameless and of no kind where there is none.
    pub fn get(&self, i: u32) -> Place<'_> {
        Place {
            name: self.name(i),
            kind: self.kinds.get(i as usize).copied().unwrap_or(0),
        }
    }

    fn name(&self, i: u32) -> &str {
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

/// A pool being filled, each place once.
#[derive(Default)]
struct Pool<'a> {
    places: Places,
    at: HashMap<Place<'a>, u32>,
}

impl<'a> Pool<'a> {
    fn add(&mut self, place: Place<'a>) -> u32 {
        if place.name.is_empty() {
            return NO_PLACE;
        }
        let places = &mut self.places;
        *self.at.entry(place).or_insert_with(|| {
            places.bytes.extend_from_slice(place.name.as_bytes());
            places.ends.push(places.bytes.len() as u32);
            places.kinds.push(place.kind);
            places.ends.len() as u32 - 1
        })
    }
}

/// A kind's number, by the theme's key for it; 0 where the theme has none.
pub fn kind_of(key: &str) -> u8 {
    KINDS.iter().position(|(k, _)| *k == key).map_or(0, |i| i as u8 + 1)
}

/// How kind `kind` is drawn: its own theme, or the default for none.
pub fn theme(kind: u8) -> PoiTheme {
    kind.checked_sub(1).and_then(|i| KINDS.get(i as usize)).map_or(DEFAULT_THEME, |(_, t)| *t)
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PoiPage {
    /// Occupied cells by the pyramid's cell index, ascending.
    pub cells: Vec<PoiCell>,
    pub places: Places,
}

impl PoiPage {
    /// A level-`level` page from where each place stands and which it is, in
    /// any order, its name cut to `NAME_CHARS`. Every place has to stand in
    /// the one page.
    pub fn of_places<'a>(level: u32, places: impl IntoIterator<Item = (Pvf3, Place<'a>)>) -> PoiPage {
        let cut: Vec<_> = places
            .into_iter()
            .map(|(point, place)| {
                let cell = POI_LATTICE.cell_of(level, &point);
                (cell.index(), cell.offset_of(GROUND.cell_of(0, &point)), cut_name(place.name), place.kind)
            })
            .collect();
        gathered(cut.iter().map(|(cell, at, name, kind)| (*cell, 1, *at, Place { name, kind: *kind })).collect())
    }

    /// The one place `c` holds; none where it holds more, or its one place
    /// has no name.
    pub fn place(&self, c: &PoiCell) -> Option<Place<'_>> {
        (c.count == 1 && c.place != NO_PLACE).then(|| self.places.get(c.place))
    }
}

impl Folds for PoiPage {
    /// A child is settled where no cell over it holds two places: a cell of
    /// one says where that place stands to the z16 cell and which it is, and
    /// a cell of none says there is nothing, so the child's cells would say it
    /// again. Only a crowd comes apart a level down.
    fn settles(&self, p: LevelPage, child: LevelPage) -> bool {
        self.cells
            .iter()
            .all(|c| c.count < 2 || POI_LATTICE.cell_of(child.level, &POI_LATTICE.cell_at(p, c.cell).base()).page() != child)
    }

    fn fold_page<'a>(p: LevelPage, held: impl Fn(LevelPage) -> Option<&'a Self>) -> Option<Self> {
        let mut cells = Vec::new();
        for (q, k) in p.children().into_iter().enumerate() {
            let child = held(k)?;
            cells.extend(child.cells.iter().map(|c| {
                let parent = POI_LATTICE.parent_cell(q, c.cell);
                // The child's centroid, found again from the child's cell and
                // read from the parent's.
                let ground = POI_LATTICE.cell_at(k, c.cell).inner_at(GROUND, 0, c.at);
                let at = POI_LATTICE.cell_at(p, parent).offset_of(ground);
                (parent, c.count, at, child.place(c).unwrap_or(NAMELESS))
            }));
        }
        Some(gathered(cells))
    }
}

/// `name`, or where it runs past `NAME_CHARS`, its first code points and an
/// ellipsis, `NAME_CHARS` in all.
fn cut_name(name: &str) -> std::borrow::Cow<'_, str> {
    match name.char_indices().nth(NAME_CHARS) {
        None => name.into(),
        Some(_) => {
            let end = name.char_indices().nth(NAME_CHARS - 1).map_or(name.len(), |(at, _)| at);
            format!("{}…", &name[..end]).into()
        }
    }
}

/// A place with no name, which the pool does not hold.
const NAMELESS: Place = Place { name: "", kind: 0 };

/// Cells ascending, each once, holding the sum of the counts it was given,
/// their centroids' mean weighted by those counts, and a place where the sum
/// is one.
fn gathered(mut cells: Vec<(u32, u32, [u32; 2], Place)>) -> PoiPage {
    cells.sort_unstable_by_key(|c| c.0);
    let mut pool = Pool::default();
    let cells = cells
        .chunk_by(|a, b| a.0 == b.0)
        .map(|group| {
            let count: u32 = group.iter().map(|c| c.1).sum();
            let mean = |lane: usize| {
                let sum: u64 = group.iter().map(|c| c.2[lane] as u64 * c.1 as u64).sum();
                ((sum + count as u64 / 2) / count as u64) as u32
            };
            PoiCell {
                cell: group[0].0,
                count,
                place: if count == 1 { pool.add(group[0].3) } else { NO_PLACE },
                at: [mean(0), mean(1)],
            }
        })
        .collect();
    PoiPage { cells, places: pool.places }
}

/// The magic; how many cells, places and bytes of names; the cells, four
/// words each (the position's two lanes a half-word each); each place's
/// name's end; each place's kind, a byte; the names.
pub fn encode(page: &PoiPage) -> Result<Vec<u8>, &'static str> {
    validate(page)?;
    let places = &page.places;
    let mut out = Vec::with_capacity(16 + page.cells.len() * CELL_BYTES + places.ends.len() * 5 + places.bytes.len());
    out.extend_from_slice(MAGIC);
    for n in [page.cells.len() as u32, places.len(), places.bytes.len() as u32] {
        out.extend_from_slice(&n.to_le_bytes());
    }
    for c in &page.cells {
        for w in [c.cell, c.count, c.at[0] | c.at[1] << 16, c.place] {
            out.extend_from_slice(&w.to_le_bytes());
        }
    }
    for &end in &places.ends {
        out.extend_from_slice(&end.to_le_bytes());
    }
    out.extend_from_slice(&places.kinds);
    out.extend_from_slice(&places.bytes);
    Ok(out)
}

pub fn decode(bytes: &[u8]) -> Result<PoiPage, &'static str> {
    if bytes.len() < 16 || &bytes[..4] != MAGIC {
        return Err("not a POI page");
    }
    let word = |at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
    let (n, places, pool) = (word(4) as usize, word(8) as usize, word(12) as usize);
    // Checked against the page's own bound before anything is allocated.
    if n > CELLS as usize || places > n || bytes.len() != 16 + n * CELL_BYTES + places * 5 + pool {
        return Err("a POI page of the wrong length");
    }
    let cells = (0..n)
        .map(|i| {
            let (at, w) = (16 + i * CELL_BYTES, word(24 + i * CELL_BYTES));
            PoiCell {
                cell: word(at),
                count: word(at + 4),
                at: [w & 0xffff, w >> 16],
                place: word(at + 12),
            }
        })
        .collect();
    let at = 16 + n * CELL_BYTES;
    let ends: Vec<u32> = (0..places).map(|i| word(at + i * 4)).collect();
    let kinds = bytes[at + places * 4..at + places * 5].to_vec();
    let text = &bytes[at + places * 5..];
    if ends.windows(2).any(|w| w[0] > w[1]) || ends.last().map_or(0, |&e| e as usize) != text.len() {
        return Err("POI names out of order");
    }
    // Name by name: a pool that is UTF-8 whole could still split a character
    // between two names.
    let mut start = 0;
    for &end in &ends {
        let name = std::str::from_utf8(&text[start..end as usize]).map_err(|_| "a POI name that is not UTF-8")?;
        if name.chars().nth(NAME_CHARS).is_some() {
            return Err("a POI name past NAME_CHARS");
        }
        start = end as usize;
    }
    let page = PoiPage {
        cells,
        places: Places { bytes: text.to_vec(), ends, kinds },
    };
    validate(&page)?;
    Ok(page)
}

/// A page is its cells ascending, each inside the page and holding a place,
/// its position a half-word a lane, and a place only where a cell holds one,
/// the place from the pool and of a kind the theme has.
fn validate(page: &PoiPage) -> Result<(), &'static str> {
    if page.cells.iter().any(|c| c.cell >= CELLS || c.count == 0) || page.cells.windows(2).any(|w| w[0].cell >= w[1].cell) {
        return Err("POI cells out of order or empty");
    }
    if page.cells.iter().any(|c| c.place != NO_PLACE && (c.count != 1 || c.place >= page.places.len())) {
        return Err("a POI place where no one place is");
    }
    if page.places.kinds.iter().any(|&k| k as usize > KINDS.len()) {
        return Err("a POI place of a kind the theme does not have");
    }
    if page.cells.iter().any(|c| c.at.iter().any(|&l| l > 0xffff)) {
        return Err("a POI position past a half-word");
    }
    Ok(())
}

#[cfg(test)]
#[path = "poi_page.test.rs"]
mod test;
