// pxl-spec/bake/poi_page.rs

//! The POI layer's page: per occupied `POI_LATTICE` cell, how many places
//! stand in it and, of each kind, the named place of least id with the leaf
//! cell it stands in. Least id is stable across zooms and says nothing
//! of prominence; which to show is the reader's choice. Folding sums counts
//! and keeps the least id per kind, so a count agrees with the leaves at any
//! zoom. A page holds what any source can give — a place's own id, its kind
//! by name, its names — so Overture, Naver or Google fill the same page; it
//! is one XTP tree, as every other baked page is.

use std::collections::HashMap;

use crate::bake::constant::CELL;
use crate::bake::poi_badge::{DEFAULT_COLOR, DEFAULT_ICON, KIND_CATEGORIES, KIND_COLORS, KIND_ICONS};
use crate::space::pvf3::pvf3::Pvf3;
use crate::space::pyramid::{Folds, Lattice, Page, GROUND, LEAF_Z, TOP_Z};
use xpute_core::status::error::MarshalError;
use xpute_core::wire::xtp::{encoder, NodeView, TreeEncoderOptions, TreeReader, TreeView};

use crate::bake::malformed;
use crate::bake::named::{NameLanes, NamesView};
use crate::space::zoom::Zoom;

/// Places are counted in cells 2^6 ground cells wide, so no two cells'
/// badges overlap: a zoom draws its ground cell at three pixels or more,
/// and the widest badge is under 192 pixels, which s = 6 covers and s = 5
/// does not.
pub const POI_LATTICE: Lattice = Lattice::new(CELL - 6);

const CELLS: u32 = POI_LATTICE.page_cells() * POI_LATTICE.page_cells();

/// The longest name a page holds, in code points; a longer one is cut to end
/// in an ellipsis. At 32, 99% of the places Overture names in Korea are whole
/// (their names' 99th percentile, measured 2026-09-25). What a badge shows of
/// it is the client's.
pub const NAME_CHARS: usize = 32;

/// A cell holds a place of each kind, and a kind is a byte.
const KINDS_MAX: usize = 256;

// A position is an offset in leaf cells inside a POI cell, a half-word a lane.
const _: () = assert!(
    GROUND.cell_zoom(LEAF_Z) - POI_LATTICE.cell_zoom(TOP_Z) <= 16,
    "a POI cell's leaf cells no longer fit a half-word a lane"
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

/// A shown place: its pool index and the leaf cell it stands in, as an
/// offset in its POI cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shown {
    pub place: u32,
    pub at: [u32; 2],
}

/// A place: its own name and its names by language, its badge-table kind (0
/// for none) and the source's id for it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Place<'a> {
    pub name: &'a str,
    pub langs: Langs<'a>,
    pub kind: u8,
    pub id: &'a str,
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
    names: Strings,
    ids: Strings,
    kinds: Vec<u8>,
    /// Where each place's pairs start in `langs`; one longer than the places.
    lang_starts: Vec<u32>,
    langs: Strings,
}

impl Places {
    pub fn get(&self, i: u32) -> Place<'_> {
        if i >= self.len() {
            return Place::default();
        }
        let span = |k: u32| self.lang_starts.get(k as usize).copied().unwrap_or(0);
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

    fn push(&mut self, name: &str, langs: impl Iterator<Item = (impl AsRef<str>, impl AsRef<str>)>, kind: u8, id: &str) -> u32 {
        if self.lang_starts.is_empty() {
            self.lang_starts.push(0);
        }
        self.names.push(name);
        self.ids.push(id);
        self.kinds.push(kind);
        for (tag, name) in langs {
            self.langs.push(tag.as_ref());
            self.langs.push(name.as_ref());
        }
        self.lang_starts.push(self.langs.len() / 2);
        self.len() - 1
    }

    fn len(&self) -> u32 {
        self.kinds.len() as u32
    }
}

#[derive(Default)]
struct Pool<'a> {
    places: Places,
    at: HashMap<Place<'a>, u32>,
}

impl<'a> Pool<'a> {
    fn add(&mut self, place: Place<'a>) -> u32 {
        let places = &mut self.places;
        *self.at.entry(place).or_insert_with(|| places.push(place.name, place.langs.iter(), place.kind, place.id))
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
    /// A page at `z` from places all in that page, names cut to
    /// `NAME_CHARS`; `complete` is false where the source gave only some.
    pub fn of_places<'a>(z: Zoom, places: impl IntoIterator<Item = (Pvf3, Place<'a>)>, complete: bool) -> PoiPage {
        let cut: Vec<_> = places
            .into_iter()
            .map(|(point, place)| {
                let cell = POI_LATTICE.cell_of(z, &point);
                (cell.index(), cell.offset_of(GROUND.cell_of(LEAF_Z, &point)), cut_name(place.name), place)
            })
            .collect();
        gathered(cut.iter().map(|(cell, at, name, place)| (*cell, 1, !complete, [(*at, Place { name, ..*place })].into_iter())))
    }

    pub fn shown(&self, c: &PoiCell) -> impl Iterator<Item = (u32, Place<'_>, [u32; 2])> + '_ {
        self.shown[c.first as usize..(c.first + c.len) as usize].iter().map(|s| (s.place, self.places.get(s.place), s.at))
    }
}

impl Folds for PoiPage {
    /// A child is settled where every cell over it holds at most one place
    /// and its count is not a floor: only a crowd comes apart a zoom finer.
    fn settles(&self, p: Page, child: Page) -> bool {
        self.cells
            .iter()
            .all(|c| (c.count < 2 && !c.partial) || POI_LATTICE.cell_of(child.z, &POI_LATTICE.cell_at(p, c.cell).base()).page() != child)
    }

    fn fold_page<'a>(p: Page, held: impl Fn(Page) -> Option<&'a Self>) -> Option<Self> {
        let mut cells = Vec::new();
        let mut children = Vec::with_capacity(4);
        for k in p.children() {
            children.push((k, held(k)?));
        }
        for (q, &(k, child)) in children.iter().enumerate() {
            cells.extend(child.cells.iter().map(move |c| {
                let parent = POI_LATTICE.parent_cell(q, c.cell);
                let places = child.shown(c).map(move |(_, place, at)| {
                    let ground = POI_LATTICE.cell_at(k, c.cell).inner_at(GROUND, LEAF_Z, at);
                    (POI_LATTICE.cell_at(p, parent).offset_of(ground), place)
                });
                (parent, c.count, c.partial, places)
            }));
        }
        Some(gathered(cells.into_iter()))
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
    let mut places: Vec<(u32, [u32; 2], Place<'a>)> = Vec::new();
    for (cell, count, partial, of) in cells {
        counts.push((cell, count, partial));
        places.extend(of.filter(|(_, p)| !p.name.is_empty()).map(|(at, p)| (cell, at, p)));
    }
    counts.sort_unstable_by_key(|c| c.0);
    places.sort_unstable_by(|a, b| (a.0, a.2.kind, a.2.id, a.2.name, a.1).cmp(&(b.0, b.2.kind, b.2.id, b.2.name, b.1)));
    places.dedup_by(|b, a| (a.0, a.2.kind) == (b.0, b.2.kind));
    let mut pool = Pool::default();
    let mut page = PoiPage::default();
    let mut from = 0;
    for group in counts.chunk_by(|a, b| a.0 == b.0) {
        let cell = group[0].0;
        let first = page.shown.len() as u32;
        while from < places.len() && places[from].0 == cell {
            let (_, at, place) = places[from];
            page.shown.push(Shown { place: pool.add(place), at });
            from += 1;
        }
        page.cells.push(PoiCell {
            cell,
            count: group.iter().map(|c| c.1).sum(),
            partial: group.iter().any(|c| c.2),
            first,
            len: page.shown.len() as u32 - first,
        });
    }
    page.places = pool.places;
    page
}

/// One branch: per cell its index, count, whether the count is a floor and
/// where its run of shown starts; per shown its place and position (two
/// half-words); per place its kind by name ("" for none), the source's id and
/// its names (`named`).
pub fn encode(page: &PoiPage) -> Result<Vec<u8>, MarshalError> {
    validate(page)?;
    let cells: Vec<u32> = page.cells.iter().map(|c| c.cell).collect();
    let counts: Vec<u32> = page.cells.iter().map(|c| c.count).collect();
    let partial: Vec<u8> = page.cells.iter().map(|c| c.partial as u8).collect();
    let starts: Vec<u32> = page.cells.iter().map(|c| c.first).chain([page.shown.len() as u32]).collect();
    let shown: Vec<u32> = page.shown.iter().map(|s| s.place).collect();
    let at: Vec<u16> = page.shown.iter().flat_map(|s| s.at.map(|l| l as u16)).collect();
    let places = &page.places;
    let kinds: Vec<&str> = places.kinds.iter().map(|&k| k.checked_sub(1).map_or("", |k| KIND_CATEGORIES[k as usize])).collect();
    let ids: Vec<&str> = (0..places.len()).map(|i| places.ids.get(i)).collect();
    let names = NameLanes::of((0..places.len()).map(|i| (places.names.get(i), places.get(i).langs.iter())));
    let mut t: TreeView<'_> = TreeView::new();
    t.branch(Some(&mut |b| {
        b.u32_array(Some(&cells)).u32_array(Some(&counts)).u8_array(Some(&partial)).u32_array(Some(&starts));
        b.u32_array(Some(&shown)).u16_array(Some(&at)).strs(Some(&kinds)).strs(Some(&ids));
        names.write(b);
    }));
    encoder().encode(&t, TreeEncoderOptions::default())
}

/// A kind this reader's table does not name reads as none, so a page from a
/// newer table still opens.
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
        cells: (0..n)
            .map(|i| PoiCell {
                cell: cells[i],
                count: counts[i],
                partial: partial[i] != 0,
                first: starts[i],
                len: starts[i + 1] - starts[i],
            })
            .collect(),
        shown: shown.iter().zip(at.as_chunks::<2>().0).map(|(&place, &[x, y])| Shown { place, at: [x as u32, y as u32] }).collect(),
        places: pooled,
    };
    let cells_fit = page.cells.iter().all(|c| c.cell < CELLS && c.count > 0) && page.cells.windows(2).all(|w| w[0].cell < w[1].cell);
    if !cells_fit || page.shown.iter().any(|s| s.place >= page.places.len()) {
        return Err(malformed());
    }
    Ok(page)
}

/// What a bake writes: cells ascending and in the page; runs contiguous;
/// shown places one per kind, ascending; positions a half-word a lane; kinds
/// in the badge table; names whole within `NAME_CHARS`.
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
