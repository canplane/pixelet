// pxl-spec/bake/poi_page.rs

//! The POI layer's page. A place is 2.5D: it belongs to the cell of the POI
//! lattice it stands in (`POI_LATTICE`). A page holds, for each occupied
//! cell, how many places stand in it and, of each kind among them
//! (bake/poi_badge.json), one with a name and where it stands, as the z16
//! ground cell it is in: the one of least id, the source's own identity for
//! a place hashed (`Place::id`), so which one it is says nothing of how
//! prominent it is and stays the same place at every level it is held at. Which of those a cell is shown by — how prominent a
//! kind is, and which are shown at which zoom — is the reader's to decide
//! (pxl-kernel layer/poi.rs), so a change of that is no new bake: the page
//! holds what is there, never a choice made of it.
//!
//! Folding is counting and keeping one of a kind. A parent cell holds what
//! its 2×2 children hold: their counts summed, which needs no ring and loses
//! nothing, so a count read at any level agrees with the leaves; and of each
//! kind the one of least id, which is the least of that kind under it, since
//! a least of leasts is the least. What a
//! cell holds is bounded by the kinds the table has, whatever stands there.
//!
//! A place's name and kind are held once a page in a pool the cells index,
//! so a place that stands in many cells — a chain's branch, a station's
//! exits — is carried once.

use std::collections::HashMap;

use crate::bake::poi_badge::{DEFAULT_COLOR, DEFAULT_ICON, KIND_CATEGORIES, KIND_COLORS, KIND_ICONS};
use crate::space::pvf3::pvf3::Pvf3;
use crate::space::pyramid::{Folds, Lattice, LevelPage, GROUND, TOP_LEVEL};
use xpute_core::status::errno::Errno;
use xpute_core::status::error::MarshalError;

const MAGIC: &[u8; 4] = b"PXP6";

/// The lattice places are counted in: a cell sixty-four times the ground's on
/// a side — a chunk's column, six levels over the ground's cell — sharing the
/// ground's pages, levels and names. A badge stands for its cell, and no two
/// cells' badges may cover each other, so a cell is no narrower on the screen
/// than the widest badge: a level is drawn where its ground cell covers three
/// to six pixels (pxl-render look/lod.rs, `SPLIT`), so 2^s ground cells cover
/// at least 3 · 2^s, and the widest badge — an icon and `SHOWN_CHARS` of a
/// name (pxl-kernel layer/poi/badge.rs) — is under 192 pixels, which s = 6
/// covers and s = 5 does not.
pub const POI_LATTICE: Lattice = Lattice::new(GROUND.scale(6));

const CELLS: u32 = POI_LATTICE.page_cells() * POI_LATTICE.page_cells();

/// A cell on the wire: its index, its count, and where its places begin and
/// how many, a word each.
const CELL_BYTES: usize = 16;

/// A place a cell holds on the wire: its pool index and where it stands.
const SHOWN_BYTES: usize = 8;

/// The longest name a page holds, in code points, a longer one cut to end in
/// an ellipsis inside it: what a badge shows of a name (pxl-kernel
/// layer/poi/badge.rs), and no more, since nothing else reads it. A bound as
/// well, with the kinds: what a page can hold is its cells times the kinds
/// times this, whatever the data.
pub const NAME_CHARS: usize = 12;

// A position is an offset in z16 ground cells inside a POI cell, a lane
// a half-word: the widest cell, at the top level, has to fit.
const _: () = assert!(
    POI_LATTICE.scale(TOP_LEVEL).above(GROUND.scale(0)) <= 16,
    "a POI cell's z16 ground cells no longer fit a half-word a lane"
);

/// An occupied cell: its index in the page, how many places stand in it,
/// whether that is all of them or only as many as the source gave — a
/// search that answers a few at a time gives the first of a crowd, and its
/// count is then a floor, not a total — and its places, one with a name of
/// each kind among them, by kind, as a run of the page's `shown`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PoiCell {
    pub cell: u32,
    pub count: u32,
    pub partial: bool,
    pub first: u32,
    pub len: u32,
}

/// A cell's count on the wire is its low bits; this bit is set where it is
/// a floor (`PoiCell::partial`).
const PARTIAL: u32 = 1 << 31;

/// A place a cell holds: its pool index, and the z16 ground cell it stands
/// in, as that cell's offset in the cell (pxl-spec space/pyramid.rs
/// `Cell::offset_of`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shown {
    pub place: u32,
    pub at: [u32; 2],
}

/// A place as a cell names it: what it is called, its kind — its number in
/// the badge table, 0 for none (`kind_of`) — and its id, the source's own
/// identity for it hashed to a word (the Placement record's rule,
/// issue/20260913-everything-is-a-file-the-placement-and-the-palette.md).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Place<'a> {
    pub name: &'a str,
    pub kind: u8,
    pub id: u64,
}

/// A page's places, each once: place i's name is `bytes[ends[i - 1]..ends[i]]`,
/// one run of UTF-8, its kind `kinds[i]` and its id `ids[i]`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Places {
    bytes: Vec<u8>,
    ends: Vec<u32>,
    kinds: Vec<u8>,
    ids: Vec<u64>,
}

impl Places {
    /// Place `i`; nameless, of no kind and of id 0 where there is none.
    pub fn get(&self, i: u32) -> Place<'_> {
        Place {
            name: self.name(i),
            kind: self.kinds.get(i as usize).copied().unwrap_or(0),
            id: self.ids.get(i as usize).copied().unwrap_or(0),
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
        let places = &mut self.places;
        *self.at.entry(place).or_insert_with(|| {
            places.bytes.extend_from_slice(place.name.as_bytes());
            places.ends.push(places.bytes.len() as u32);
            places.kinds.push(place.kind);
            places.ids.push(place.id);
            places.ends.len() as u32 - 1
        })
    }
}

/// A kind's number, by its category; 0 where the table has none.
pub fn kind_of(key: &str) -> u8 {
    KIND_CATEGORIES.iter().position(|k| *k == key).map_or(0, |i| i as u8 + 1)
}

/// Kind `kind`'s badge icon, or the default for none.
pub fn kind_icon(kind: u8) -> &'static str {
    kind.checked_sub(1).and_then(|i| KIND_ICONS.get(i as usize)).copied().unwrap_or(DEFAULT_ICON)
}

/// Kind `kind`'s badge color, or the default for none.
pub fn kind_color(kind: u8) -> u32 {
    kind.checked_sub(1).and_then(|i| KIND_COLORS.get(i as usize)).copied().unwrap_or(DEFAULT_COLOR)
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PoiPage {
    /// Occupied cells by the pyramid's cell index, ascending.
    pub cells: Vec<PoiCell>,
    /// The cells' places, a cell's in one run, by kind.
    pub shown: Vec<Shown>,
    pub places: Places,
}

impl PoiPage {
    /// A level-`level` page from where each place stands and which it is, in
    /// any order, its name cut to `NAME_CHARS`, and whether they are all the
    /// source has there (`complete`) or as many as it gave. Every place has
    /// to stand in the one page.
    pub fn of_places<'a>(level: u32, places: impl IntoIterator<Item = (Pvf3, Place<'a>)>, complete: bool) -> PoiPage {
        let cut: Vec<_> = places
            .into_iter()
            .map(|(point, place)| {
                let cell = POI_LATTICE.cell_of(level, &point);
                (cell.index(), cell.offset_of(GROUND.cell_of(0, &point)), cut_name(place.name), place.kind, place.id)
            })
            .collect();
        gathered(
            cut.iter()
                .map(|(cell, at, name, kind, id)| (*cell, 1, !complete, [(*at, Place { name, kind: *kind, id: *id })].into_iter())),
        )
    }

    /// The places `c` holds, one of each kind among those with a name, and
    /// where each stands.
    pub fn shown(&self, c: &PoiCell) -> impl Iterator<Item = (u32, Place<'_>, [u32; 2])> + '_ {
        self.shown[c.first as usize..(c.first + c.len) as usize].iter().map(|s| (s.place, self.places.get(s.place), s.at))
    }
}

impl Folds for PoiPage {
    /// A child is settled where no cell over it holds two places: a cell of
    /// one says where that place stands to the z16 cell and which it is, and
    /// a cell of none says there is nothing, so the child's cells would say it
    /// again. Only a crowd comes apart a level down — and a count that is a
    /// floor, which says only what the source gave, not that nothing more is
    /// under it.
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
                // Where each place stands, found again from the child's cell
                // and read from the parent's.
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

/// Cells ascending, each once, holding the sum of the counts it was given,
/// a floor where any of them was one, and, of each kind among the named
/// places it was given, the one of least id, standing where it stands.
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
    let mut page = PoiPage::default();
    let mut from = 0;
    for group in counts.chunk_by(|a, b| a.0 == b.0) {
        let cell = group[0].0;
        let first = page.shown.len() as u32;
        while from < places.len() && places[from].0 == cell {
            let (_, kind, id, name, at) = places[from];
            page.shown.push(Shown {
                place: pool.add(Place { name, kind, id }),
                at,
            });
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

/// The magic; how many cells, places shown, places and bytes of names; the
/// cells, four words each; the places shown, two words each (the position's
/// two lanes a half-word each); each place's id, two words; each place's
/// name's end; each place's kind, a byte; the names.
pub fn encode(page: &PoiPage) -> Result<Vec<u8>, MarshalError> {
    validate(page)?;
    let places = &page.places;
    let mut out = Vec::with_capacity(20 + page.cells.len() * CELL_BYTES + page.shown.len() * SHOWN_BYTES + places.ends.len() * 13 + places.bytes.len());
    out.extend_from_slice(MAGIC);
    for n in [page.cells.len() as u32, page.shown.len() as u32, places.len(), places.bytes.len() as u32] {
        out.extend_from_slice(&n.to_le_bytes());
    }
    for c in &page.cells {
        for w in [c.cell, c.count | if c.partial { PARTIAL } else { 0 }, c.first, c.len] {
            out.extend_from_slice(&w.to_le_bytes());
        }
    }
    for s in &page.shown {
        for w in [s.place, s.at[0] | s.at[1] << 16] {
            out.extend_from_slice(&w.to_le_bytes());
        }
    }
    for &id in &places.ids {
        out.extend_from_slice(&id.to_le_bytes());
    }
    for &end in &places.ends {
        out.extend_from_slice(&end.to_le_bytes());
    }
    out.extend_from_slice(&places.kinds);
    out.extend_from_slice(&places.bytes);
    Ok(out)
}

pub fn decode(bytes: &[u8]) -> Result<PoiPage, MarshalError> {
    if bytes.len() < 20 || &bytes[..4] != MAGIC {
        return Err(bad());
    }
    let word = |at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
    let (n, shown, places, pool) = (word(4) as usize, word(8) as usize, word(12) as usize, word(16) as usize);
    // Checked against the page's own bound before anything is allocated.
    let kinds = KIND_CATEGORIES.len() + 1;
    if n > CELLS as usize || shown > n * kinds || places > shown || bytes.len() != 20 + n * CELL_BYTES + shown * SHOWN_BYTES + places * 13 + pool {
        return Err(bad());
    }
    let cells = (0..n)
        .map(|i| {
            let at = 20 + i * CELL_BYTES;
            PoiCell {
                cell: word(at),
                count: word(at + 4) & !PARTIAL,
                partial: word(at + 4) & PARTIAL != 0,
                first: word(at + 8),
                len: word(at + 12),
            }
        })
        .collect();
    let base = 20 + n * CELL_BYTES;
    let shown = (0..shown)
        .map(|i| {
            let at = base + i * SHOWN_BYTES;
            let w = word(at + 4);
            Shown {
                place: word(at),
                at: [w & 0xffff, w >> 16],
            }
        })
        .collect::<Vec<_>>();
    let ids_at = base + shown.len() * SHOWN_BYTES;
    let ids: Vec<u64> = (0..places).map(|i| u64::from_le_bytes(bytes[ids_at + i * 8..ids_at + i * 8 + 8].try_into().unwrap())).collect();
    let at = ids_at + places * 8;
    let ends: Vec<u32> = (0..places).map(|i| word(at + i * 4)).collect();
    let kinds = bytes[at + places * 4..at + places * 5].to_vec();
    let text = &bytes[at + places * 5..];
    if ends.windows(2).any(|w| w[0] > w[1]) || ends.last().map_or(0, |&e| e as usize) != text.len() {
        return Err(bad());
    }
    // Name by name: a pool that is UTF-8 whole could still split a character
    // between two names.
    let mut start = 0;
    for &end in &ends {
        let name = std::str::from_utf8(&text[start..end as usize]).map_err(|_| bad())?;
        if name.is_empty() || name.chars().nth(NAME_CHARS).is_some() {
            return Err(bad());
        }
        start = end as usize;
    }
    let page = PoiPage {
        cells,
        shown,
        places: Places {
            bytes: text.to_vec(),
            ends,
            kinds,
            ids,
        },
    };
    validate(&page)?;
    Ok(page)
}

/// A page is its cells ascending, each inside the page and holding a place,
/// its places a run of `shown` past the last cell's, each from the pool, of
/// one kind each, ascending, its position a half-word a lane, and every kind
/// one the badge table has.
fn validate(page: &PoiPage) -> Result<(), MarshalError> {
    if page.cells.iter().any(|c| c.cell >= CELLS || c.count == 0 || c.count & PARTIAL != 0) || page.cells.windows(2).any(|w| w[0].cell >= w[1].cell) {
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
    if next as usize != page.shown.len() || page.places.kinds.iter().any(|&k| k as usize > KIND_CATEGORIES.len()) {
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
