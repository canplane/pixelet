// pxl-spec/bake/poi_page.rs

//! The POI layer's page: per occupied `POI_LATTICE` cell, how many places
//! stand in it and, of each kind, the named place of least id with the z16
//! ground cell it stands in. Least id is stable across levels and says nothing
//! of prominence; which to show is the reader's choice. Folding sums counts
//! and keeps the least id per kind, so a count agrees with the leaves at any
//! level. A page holds what any source can give — a place's own id, its kind
//! by name, its names — so Overture, Naver or Google fill the same page; it
//! is one XTP tree, as every other baked page is.

use std::collections::HashMap;

use crate::bake::poi_badge::{DEFAULT_COLOR, DEFAULT_ICON, KIND_CATEGORIES, KIND_COLORS, KIND_ICONS};
use crate::space::pvf3::pvf3::Pvf3;
use crate::space::pyramid::{Folds, Lattice, LevelPage, GROUND, TOP_LEVEL};
use xpute_core::status::errno::Errno;
use xpute_core::status::error::MarshalError;
use xpute_core::wire::xtp::{encoder, NodeView, TreeEncoderOptions, TreeReader, TreeView};

use crate::bake::named::{NameLanes, NamesView};

/// Places are counted in cells 2^6 ground cells wide, so no two cells'
/// badges overlap: a level draws its ground cell at three pixels or more,
/// and the widest badge is under 192 pixels, which s = 6 covers and s = 5
/// does not.
pub const POI_LATTICE: Lattice = Lattice::new(GROUND.scale(6));

const CELLS: u32 = POI_LATTICE.page_cells() * POI_LATTICE.page_cells();

/// The longest name a page holds, in code points; a longer one is cut to end
/// in an ellipsis. At 32, 99% of the places Overture names in Korea are whole
/// (their names' 99th percentile, measured 2026-09-25). What a badge shows of
/// it is the client's.
pub const NAME_CHARS: usize = 32;

/// A cell holds a place of each kind, and a kind is a byte.
const KINDS_MAX: usize = 256;

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

/// A shown place: its pool index and the z16 ground cell it stands in, as an
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
    pub names: &'a [(String, String)],
    pub kind: u8,
    pub id: &'a str,
}

#[derive(Clone, Debug, Default, PartialEq)]
struct Record {
    name: String,
    names: Vec<(String, String)>,
    kind: u8,
    id: String,
}

/// A page's places, each once.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Places(Vec<Record>);

impl Places {
    pub fn get(&self, i: u32) -> Place<'_> {
        self.0.get(i as usize).map_or(Place::default(), |r| Place {
            name: &r.name,
            names: &r.names,
            kind: r.kind,
            id: &r.id,
        })
    }

    fn len(&self) -> u32 {
        self.0.len() as u32
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
        *self.at.entry(place).or_insert_with(|| {
            places.0.push(Record {
                name: place.name.to_owned(),
                names: place.names.to_vec(),
                kind: place.kind,
                id: place.id.to_owned(),
            });
            places.len() - 1
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
                (cell.index(), cell.offset_of(GROUND.cell_of(0, &point)), cut_name(place.name), place)
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
    /// and its count is not a floor: only a crowd comes apart a level down.
    fn settles(&self, p: LevelPage, child: LevelPage) -> bool {
        self.cells
            .iter()
            .all(|c| (c.count < 2 && !c.partial) || POI_LATTICE.cell_of(child.level, &POI_LATTICE.cell_at(p, c.cell).base()).page() != child)
    }

    fn fold_page<'a>(p: LevelPage, held: impl Fn(LevelPage) -> Option<&'a Self>) -> Option<Self> {
        let mut cells = Vec::new();
        let mut children = Vec::with_capacity(4);
        for k in p.children() {
            children.push((k, held(k)?));
        }
        for (q, &(k, child)) in children.iter().enumerate() {
            cells.extend(child.cells.iter().map(move |c| {
                let parent = POI_LATTICE.parent_cell(q, c.cell);
                let places = child.shown(c).map(move |(_, place, at)| {
                    let ground = POI_LATTICE.cell_at(k, c.cell).inner_at(GROUND, 0, at);
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
    let records = &page.places.0;
    let kinds: Vec<&str> = records.iter().map(|r| r.kind.checked_sub(1).map_or("", |k| KIND_CATEGORIES[k as usize])).collect();
    let ids: Vec<&str> = records.iter().map(|r| r.id.as_str()).collect();
    let names = NameLanes::of(records.iter().map(|r| (r.name.as_str(), r.names.as_slice())));
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
    let (cells, counts, partial, starts) = (root.at(0)?.get_array::<u32>()?, root.at(1)?.get_array::<u32>()?, root.at(2)?.get_array::<u8>()?, root.at(3)?.get_array::<u32>()?);
    let (shown, at, kinds, ids) = (root.at(4)?.get_array::<u32>()?, root.at(5)?.get_array::<u16>()?, root.at(6)?.get_strs()?, root.at(7)?.get_strs()?);
    let (n, places) = (cells.len(), kinds.len() as usize);
    let runs_fit = starts.len() == n + 1 && starts[0] == 0 && starts.windows(2).all(|w| w[0] <= w[1]) && starts[n] as usize == shown.len();
    if n > CELLS as usize || counts.len() != n || partial.len() != n || !runs_fit || at.len() != 2 * shown.len() || shown.len() > n * KINDS_MAX || ids.len() as usize != places {
        return Err(bad());
    }
    let names = NamesView::read(&root, 8, places)?;
    let mut records = Vec::with_capacity(places);
    for i in 0..places as u32 {
        let name = names.name(i).ok_or_else(bad)?;
        if name.is_empty() || name.chars().nth(NAME_CHARS).is_some() {
            return Err(bad());
        }
        records.push(Record {
            name: name.to_owned(),
            names: names.languages(i).map(|(tag, name)| (tag.to_owned(), name.to_owned())).collect(),
            kind: kind_of(kinds.get(i).ok_or_else(bad)?),
            id: ids.get(i).ok_or_else(bad)?.to_owned(),
        });
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
        places: Places(records),
    };
    let cells_fit = page.cells.iter().all(|c| c.cell < CELLS && c.count > 0) && page.cells.windows(2).all(|w| w[0].cell < w[1].cell);
    if !cells_fit || page.shown.iter().any(|s| s.place >= page.places.len()) {
        return Err(bad());
    }
    Ok(page)
}

/// What a bake writes: cells ascending and in the page; runs contiguous;
/// shown places one per kind, ascending; positions a half-word a lane; kinds
/// in the badge table; names whole within `NAME_CHARS`.
fn validate(page: &PoiPage) -> Result<(), MarshalError> {
    if page.cells.iter().any(|c| c.cell >= CELLS || c.count == 0) || page.cells.windows(2).any(|w| w[0].cell >= w[1].cell) {
        return Err(bad());
    }
    let mut next = 0;
    for c in &page.cells {
        if c.first != next || (c.first + c.len) as usize > page.shown.len() {
            return Err(bad());
        }
        next = c.first + c.len;
        let run = &page.shown[c.first as usize..next as usize];
        if run.iter().any(|s| s.place >= page.places.len() || s.at.iter().any(|&l| l > 0xffff)) {
            return Err(bad());
        }
        if run.windows(2).any(|w| page.places.get(w[0].place).kind >= page.places.get(w[1].place).kind) {
            return Err(bad());
        }
    }
    let records = &page.places.0;
    if next as usize != page.shown.len() || records.iter().any(|r| r.kind as usize > KIND_CATEGORIES.len() || r.name.is_empty() || r.name.chars().nth(NAME_CHARS).is_some()) {
        return Err(bad());
    }
    Ok(())
}

#[track_caller]
fn bad() -> MarshalError {
    MarshalError::new(Errno::EBADMSG)
}

#[cfg(test)]
#[path = "poi_page.test.rs"]
mod test;
