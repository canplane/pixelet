// pxl-spec/bake/poi_page.rs

//! The POI layer's page: per occupied `POI_LATTICE` cell, how many places
//! stand in it and, of each kind, the named place of least id with the z16
//! ground cell it stands in. Least id is stable across levels and says nothing
//! of prominence; which to show is the reader's choice. Folding sums counts
//! and keeps the least id per kind, so a count agrees with the leaves at any
//! level. Names and kinds are pooled once a page.

use std::collections::HashMap;

use crate::bake::poi_badge::{DEFAULT_COLOR, DEFAULT_ICON, KIND_CATEGORIES, KIND_COLORS, KIND_ICONS};
use crate::space::pvf3::pvf3::Pvf3;
use crate::space::pyramid::{Folds, Lattice, LevelPage, GROUND, TOP_LEVEL};
use xpute_core::status::error::MarshalError;

use crate::bake::malformed;
use crate::bake::named::{NameLanes, NamesView};

/// Places are counted in cells 2^6 ground cells wide, so no two cells'
/// badges overlap: a level draws its ground cell at three pixels or more,
/// and the widest badge is under 192 pixels, which s = 6 covers and s = 5
/// does not.
pub const POI_LATTICE: Lattice = Lattice::new(GROUND.scale(6));

const CELLS: u32 = POI_LATTICE.page_cells() * POI_LATTICE.page_cells();

/// index, count, first, len: a word each.
const CELL_BYTES: usize = 16;

/// pool index, then position: a word each.
const SHOWN_BYTES: usize = 8;

/// The longest name a page holds, in code points: what a badge shows. A
/// longer one is cut to end in an ellipsis.
pub const NAME_CHARS: usize = 12;

// A position is an offset in z16 cells inside a POI cell, a half-word a lane.
const _: () = assert!(
    POI_LATTICE.scale(TOP_LEVEL).above(GROUND.scale(0)) <= 16,
    "a POI cell's z16 ground cells no longer fit a half-word a lane"
);

/// An occupied cell. `partial` marks a count that is a floor: a source that
/// answers a few at a time gave only the first of a crowd. Its places are a
/// run of the page's `shown`, one of each kind, by kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PoiCell {
    pub cell: u32,
    pub count: u32,
    pub partial: bool,
    pub first: u32,
    pub len: u32,
}

/// Set in a wire count that is a floor.
const PARTIAL: u32 = 1 << 31;

/// A shown place: its pool index and the z16 ground cell it stands in, as an
/// offset in its POI cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shown {
    pub place: u32,
    pub at: [u32; 2],
}

/// A place: its name, its badge-table kind (0 for none) and its source id
/// hashed to a word.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Place<'a> {
    pub name: &'a str,
    pub kind: u8,
}

/// Strings end to end in one buffer, read by index.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Strings {
    bytes: String,
    ends: Vec<u32>,
}

impl Strings {
    pub fn of<'s>(items: impl IntoIterator<Item = &'s str>) -> Strings {
        let mut s = Strings::default();
        items.into_iter().for_each(|item| s.push(item));
        s
    }

    fn push(&mut self, s: &str) {
        self.bytes.push_str(s);
        self.ends.push(self.bytes.len() as u32);
    }

    pub fn get(&self, i: u32) -> &str {
        let Some(&end) = self.ends.get(i as usize) else { return "" };
        let start = if i == 0 { 0 } else { self.ends[i as usize - 1] };
        &self.bytes[start as usize..end as usize]
    }

    fn len(&self) -> u32 {
        self.ends.len() as u32
    }
}

/// A place's names by language: pairs `from..to` of a pool of (tag, name).
#[derive(Clone, Copy, Debug, Default)]
pub struct Langs<'a> {
    pool: Option<&'a Strings>,
    from: u32,
    to: u32,
}

impl<'a> Langs<'a> {
    /// Every pair of `pool`, tag then name.
    pub fn all(pool: &'a Strings) -> Langs<'a> {
        Langs {
            pool: Some(pool),
            from: 0,
            to: pool.len() / 2,
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (&'a str, &'a str)> + 'a {
        let pool = self.pool;
        (self.from..self.to).filter_map(move |p| pool.map(|s| (s.get(2 * p), s.get(2 * p + 1))))
    }
}

impl PartialEq for Langs<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.iter().eq(other.iter())
    }
}

impl Eq for Langs<'_> {}

impl std::hash::Hash for Langs<'_> {
    fn hash<H: std::hash::Hasher>(&self, h: &mut H) {
        self.iter().for_each(|pair| pair.hash(h));
    }
}

/// A page's places, each once, in a few buffers a page.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Places {
    bytes: Vec<u8>,
    ends: Vec<u32>,
    kinds: Vec<u8>,
}

impl Places {
    pub fn get(&self, i: u32) -> Place<'_> {
        Place {
            name: self.names.get(i),
            langs: Langs {
                pool: Some(&self.langs),
                from: span(i),
                to: span(i + 1),
            },
            kind: self.kinds[i as usize],
            id: self.ids.get(i),
        }
    }

    fn name(&self, i: u32) -> &str {
        let Some(&end) = self.ends.get(i as usize) else { return "" };
        let start = if i == 0 { 0 } else { self.ends[i as usize - 1] };
        // Checked as UTF-8 name by name in `decode`.
        std::str::from_utf8(&self.bytes[start as usize..end as usize]).unwrap_or("")
    }

    fn len(&self) -> u32 {
        self.ends.len() as u32
    }
}

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

/// 0 where the table has none.
pub fn kind_of(key: &str) -> u8 {
    KIND_CATEGORIES.iter().position(|k| *k == key).map_or(0, |i| i as u8 + 1)
}

pub fn kind_icon(kind: u8) -> &'static str {
    kind.checked_sub(1).and_then(|i| KIND_ICONS.get(i as usize)).copied().unwrap_or(DEFAULT_ICON)
}

pub fn kind_color(kind: u8) -> u32 {
    kind.checked_sub(1).and_then(|i| KIND_COLORS.get(i as usize)).copied().unwrap_or(DEFAULT_COLOR)
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PoiPage {
    /// Ascending by cell index.
    pub cells: Vec<PoiCell>,
    /// A cell's places in one run, by kind.
    pub shown: Vec<Shown>,
    pub places: Places,
}

impl PoiPage {
    /// A level-`level` page from places all in that page, names cut to
    /// `NAME_CHARS`; `complete` is false where the source gave only some.
    pub fn of_places<'a>(level: u32, places: impl IntoIterator<Item = (Pvf3, Place<'a>)>, complete: bool) -> PoiPage {
        let cut: Vec<_> = places
            .into_iter()
            .map(|(point, place)| {
                let cell = POI_LATTICE.cell_of(level, &point);
                (cell.index(), cell.offset_of(GROUND.cell_of(0, &point)), cut_name(place.name), place.kind)
            })
            .collect();
        gathered(cut.iter().map(|(cell, at, name, kind)| (*cell, 1, *at, Place { name, kind: *kind })).collect())
    }

    pub fn shown(&self, c: &PoiCell) -> impl Iterator<Item = (u32, Place<'_>, [u32; 2])> + '_ {
        self.shown[c.first as usize..(c.first + c.len) as usize].iter().map(|s| (s.place, self.places.get(s.place), s.at))
    }
}

impl Folds for PoiPage {
    /// A child is settled where every cell over it holds at most one place
    /// and its count is not a floor: only a crowd comes apart a level down.
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
                let places = child.shown(c).map(move |(_, place, at)| {
                    let ground = POI_LATTICE.cell_at(k, c.cell).inner_at(GROUND, 0, at);
                    (POI_LATTICE.cell_at(p, parent).offset_of(ground), place)
                });
                (parent, c.count, c.partial, places)
            }));
        }
        Some(gathered(cells))
    }
}

/// `name` cut to `NAME_CHARS` code points, ending in an ellipsis.
fn cut_name(name: &str) -> std::borrow::Cow<'_, str> {
    match name.char_indices().nth(NAME_CHARS) {
        None => name.into(),
        Some(_) => {
            let end = name.char_indices().nth(NAME_CHARS - 1).map_or(name.len(), |(at, _)| at);
            format!("{}…", &name[..end]).into()
        }
    }
}

/// Cells ascending, each once: counts summed, partial if any part is, and per
/// kind the named place of least id.
fn gathered<'a, P: Iterator<Item = ([u32; 2], Place<'a>)>>(cells: impl Iterator<Item = (u32, u32, bool, P)>) -> PoiPage {
    let mut counts: Vec<(u32, u32, bool)> = Vec::new();
    let mut places: Vec<(u32, u8, u64, &'a str, [u32; 2])> = Vec::new();
    for (cell, count, partial, of) in cells {
        counts.push((cell, count, partial));
        places.extend(of.filter(|(_, p)| !p.name.is_empty()).map(|(at, p)| (cell, p.kind, p.id, p.name, at)));
    }
    counts.sort_unstable_by_key(|c| c.0);
    places.sort_unstable();
    places.dedup_by(|b, a| (a.0, a.1) == (b.0, b.1));
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

/// Magic; counts of cells, shown, places and name bytes; cells (4 words);
/// shown (2 words, the position's lanes a half-word each); ids (2 words);
/// name ends; kinds (a byte); names.
pub fn encode(page: &PoiPage) -> Result<Vec<u8>, MarshalError> {
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

pub fn decode(bytes: &[u8]) -> Result<PoiPage, MarshalError> {
    let root = TreeReader::new(bytes)?.read_branch()?;
    let (cells, counts, partial, starts) = (
        root.at(0)?.get_array::<u32>()?,
        root.at(1)?.get_array::<u32>()?,
        root.at(2)?.get_array::<u8>()?,
        root.at(3)?.get_array::<u32>()?,
    );
    let (shown, at, kinds, ids) = (root.at(4)?.get_array::<u32>()?, root.at(5)?.get_array::<u16>()?, root.at(6)?.get_strs()?, root.at(7)?.get_strs()?);
    let (n, places) = (cells.len(), kinds.len() as usize);
    let runs_fit = starts.len() == n + 1 && starts[0] == 0 && starts.windows(2).all(|w| w[0] <= w[1]) && starts[n] as usize == shown.len();
    if n > CELLS as usize || counts.len() != n || partial.len() != n || !runs_fit || at.len() != 2 * shown.len() || shown.len() > n * KINDS_MAX || ids.len() as usize != places {
        return Err(malformed());
    }
    let names = NamesView::read(&root, 8, places)?;
    let mut pooled = Places::default();
    for i in 0..places as u32 {
        let name = names.name(i).ok_or_else(malformed)?;
        if name.is_empty() || name.chars().nth(NAME_CHARS).is_some() {
            return Err(malformed());
        }
        pooled.push(name, names.languages(i), kind_of(kinds.get(i).ok_or_else(malformed)?), ids.get(i).ok_or_else(malformed)?);
    }
    let page = PoiPage {
        cells,
        places: Places { bytes: text.to_vec(), ends, kinds },
    };
    let cells_fit = page.cells.iter().all(|c| c.cell < CELLS && c.count > 0) && page.cells.windows(2).all(|w| w[0].cell < w[1].cell);
    if !cells_fit || page.shown.iter().any(|s| s.place >= page.places.len()) {
        return Err(malformed());
    }
    Ok(page)
}

/// Cells ascending and in the page; runs contiguous; shown places one per kind,
/// ascending; positions a half-word a lane; kinds in the badge table.
fn validate(page: &PoiPage) -> Result<(), MarshalError> {
    if page.cells.iter().any(|c| c.cell >= CELLS || c.count == 0) || page.cells.windows(2).any(|w| w[0].cell >= w[1].cell) {
        return Err(malformed());
    }
    let mut next = 0;
    for c in &page.cells {
        if c.first != next || (c.first + c.len) as usize > page.shown.len() {
            return Err(malformed());
        }
        next = c.first + c.len;
        let run = &page.shown[c.first as usize..next as usize];
        if run.iter().any(|s| s.place >= page.places.len() || s.at.iter().any(|&l| l > 0xffff)) {
            return Err(malformed());
        }
        if run.windows(2).any(|w| page.places.get(w[0].place).kind >= page.places.get(w[1].place).kind) {
            return Err(malformed());
        }
    }
    let places = &page.places;
    let bad_place = |i: u32| places.kinds[i as usize] as usize > KIND_CATEGORIES.len() || places.names.get(i).is_empty() || places.names.get(i).chars().nth(NAME_CHARS).is_some();
    if next as usize != page.shown.len() || (0..places.len()).any(bad_place) {
        return Err(malformed());
    }
    Ok(())
}


#[cfg(test)]
#[path = "poi_page.test.rs"]
mod test;
