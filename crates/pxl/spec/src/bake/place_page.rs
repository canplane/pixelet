// pxl-spec/bake/place_page.rs

//! What the reverse-geocode bake writes: a dictionary of the places a
//! country has, and one object per group of pages saying which of them each
//! page is in.
//!
//! A label is the same for every page of a district, so a page of its own is
//! the wrong unit to store one in: the set was 167,495 objects naming 3,426
//! distinct places, each name spelled out where it appeared
//! (issue/20260923-a-place-name-is-a-rank-and-an-id.md). A **group** is the
//! page `GROUP_SHAMT` levels up — 16 leaves on a side, 256 of them — and it
//! holds the labels its leaves take and an index a leaf, so a reader crossing
//! a district fetches once rather than once a page.
//!
//! The **dictionary** holds the places the source names many pages by, once:
//! each one's rank, its own name, and that name by language. It is fetched
//! once and held, so a group carries numbers rather than names. A place the
//! source names only a stretch of pages by is a group's own instead, so the
//! dictionary is not the size of everything the source has ever named once:
//! which places go where is the source's adapter's to decide, and a reader
//! reads both the same way.
//!
//! A part is said to be an approximation where the source says so — a name
//! for near the page rather than one it is in — which is a fact of the part,
//! apart from where its name is kept.
//!
//! ```text
//!   dictionary   the places many pages are named by, once
//!   group        labels: [place, ...] each   what the pages under it are in
//!                page[256] -> a label
//!                the places only this group's pages are named by
//! ```
//!
//! The bake writes from the owned `Named` and `PlaceGroup`; a reader opens
//! a view over the packet where it lies and reads a name out of it without
//! decoding the rest. A run of places is stored as a rank a place, the place
//! its language names start at, and the names themselves — each set of
//! strings one leaf of offsets and UTF-8 (XTP's STRS) rather than a node a
//! string — so reaching the thousandth place in the dictionary is an index
//! rather than a walk.

use xpute_core::status::errno::Errno;
use xpute_core::status::error::MarshalError;
use xpute_core::wire::xtp::{encoder, BranchViewImpl, NodeView, TreeEncoderOptions, TreeReader, TreeView};
use xpute_core::wire::xtp::BranchCursor;

use crate::bake::named::{NameLanes, NamesView};

use crate::bake::quadkey::{quadkey_of, Quadkey, PAGE_KEY_DEPTH};
use crate::place::PlaceRank;
use crate::space::pvf3::ivec::IVec;
use crate::space::pyramid::PAGE_ZOOM;
use xpute_core::status::bug::OrBug;

pub const DICT_OBJECT: &str = "_dict";

/// 16 pages on a side.
pub const GROUP_SHAMT: u32 = 4;

pub const GROUP_ZOOM: u32 = PAGE_ZOOM - GROUP_SHAMT;

pub fn group_name(group_idx: IVec) -> Quadkey {
    quadkey_of(GROUP_ZOOM, group_idx)
}
pub const GROUP_PAGES: usize = 1 << (2 * GROUP_SHAMT);

/// From the low bits of its page index.
pub fn leaf_slot(x: i64, y: i64) -> usize {
    let mask = (1i64 << GROUP_SHAMT) - 1;
    ((y & mask) << GROUP_SHAMT | (x & mask)) as usize
}

/// A place a label names: what it is called and where it sits.
#[derive(Clone, Debug, PartialEq)]
pub struct Named {
    pub rank: PlaceRank,
    pub name: String,
    /// `(tag, name)` pairs.
    pub names: Vec<(String, String)>,
}

fn write_places<'a>(b: &mut BranchViewImpl<'a>, ranks: &'a [u8], names: &'a NameLanes<'a>) {
    b.u8_array(Some(ranks));
    names.write(b);
}

fn place_lanes(places: &[Named]) -> PlaceLanes<'_> {
    let mut starts = Vec::with_capacity(places.len() + 1);
    let mut at = 0u32;
    for d in places {
        starts.push(at);
        at += d.names.len() as u32;
    }
    starts.push(at);
    PlaceLanes {
        ranks: places.iter().map(|d| d.rank as u8).collect(),
        starts,
        names: places.iter().map(|d| d.name.as_str()).collect(),
        langs: places.iter().flat_map(|d| d.names.iter().flat_map(|(tag, name)| [tag.as_str(), name.as_str()])).collect(),
    }
}

fn write_places<'a>(b: &mut BranchViewImpl<'a>, lanes: &'a PlaceLanes<'a>) {
    b.u8_array(Some(&lanes.ranks)).u32_array(Some(&lanes.starts));
    b.strs(Some(&lanes.names)).strs(Some(&lanes.langs));
}

/// A run of places read where it lies. The dictionary is one of these and a
/// group's own places are another, so the two are written and read one way.
#[derive(Clone)]
pub struct PlacesView<'a> {
    ranks: &'a [u8],
    names: NamesView<'a>,
}

fn read_places<'a>(root: &BranchCursor<'a>, at: u32) -> Result<PlacesView<'a>, MarshalError> {
    let ranks = root.at(at)?.get_array::<u8>()?;
    Ok(PlacesView {
        ranks,
        names: NamesView::read(root, at + 1, ranks.len())?,
    })
}

impl<'a> PlacesView<'a> {
    pub fn len(&self) -> usize {
        self.ranks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ranks.is_empty()
    }

    pub fn at(&self, i: u32) -> Option<PlaceRef<'a>> {
        Some(PlaceRef {
            rank: PlaceRank::of(*self.ranks.get(i as usize)?)?,
            name: self.names.name(i)?,
            approx: false,
            names: self.names,
            at: i,
        })
    }
}

#[derive(Clone)]
pub struct PlaceRef<'a> {
    pub rank: PlaceRank,
    /// The source's own name for the place, in whatever language it holds
    /// it in.
    pub name: &'a str,
    /// From a place near the page rather than one containing it.
    pub approx: bool,
    names: NamesView<'a>,
    at: u32,
}

impl<'a> PlaceRef<'a> {
    /// Every language the source had a name in, as `(tag, name)`.
    pub fn languages(&self) -> impl Iterator<Item = (&'a str, &'a str)> {
        self.names.languages(self.at)
    }

    /// The name in `tag`'s language, or the source's own where it has none
    /// — a name a reader cannot read beats no name at all.
    pub fn in_language(&self, tag: &str) -> &'a str {
        self.languages().find(|&(stored, _)| stored == tag).map_or(self.name, |(_, name)| name)
    }
}

/// The dictionary's places, in the order a group's numbers name them.
pub fn encode_place_dict(places: &[Named]) -> Result<Vec<u8>, MarshalError> {
    let lanes = place_lanes(places);
    let mut t: TreeView<'_> = TreeView::new();
    t.branch(Some(&mut |b| write_places(b, &ranks, &names)));
    encoder().encode(&t, TreeEncoderOptions::default())
}

pub fn open_place_dict(pkt: &[u8]) -> Result<PlacesView<'_>, MarshalError> {
    read_places(&TreeReader::new(pkt)?.read_branch()?, 0)
}

/// One group: the labels its pages take, and which of them each page is in.
///
/// A label is a place read broad to narrow, by number: a place of the
/// dictionary's, or with `LOCAL` set one of the group's own, and with
/// `APPROX` set a part the source gives as an approximation — so a label is a
/// plain run of numbers with nothing beside it.
#[derive(Clone, Debug, PartialEq)]
pub struct PlaceGroup {
    /// Broad-to-narrow numbers, a label each.
    pub labels: Vec<Vec<u32>>,
    /// Per page; `NO_LABEL` outside every known place.
    pub pages: Vec<u16>,
    /// The places only this group's pages are named by.
    pub locals: Vec<Named>,
}

/// What a page outside every place the source knows takes.
pub const NO_LABEL: u16 = u16::MAX;
/// On a number: it names one of the group's own places rather than one of
/// the dictionary's. The high bit, which a group baked before `APPROX` was a
/// bit of its own set for every such part, so it reads as it was written.
pub const LOCAL: u32 = 1 << 31;
/// On a number: the source gives the part as an approximation.
pub const APPROX: u32 = 1 << 30;

pub fn encode_place_group(group: &PlaceGroup) -> Result<Vec<u8>, MarshalError> {
    let mut starts = Vec::with_capacity(group.labels.len() + 1);
    let mut at = 0u32;
    for label in &group.labels {
        starts.push(at);
        at += label.len() as u32;
    }
    starts.push(at);
    let flat: Vec<u32> = group.labels.iter().flatten().copied().collect();
    let (ranks, names) = place_lanes(&group.locals);
    let mut t: TreeView<'_> = TreeView::new();
    t.branch(Some(&mut |b| {
        b.u32_array(Some(&starts)).u32_array(Some(&flat)).u16_array(Some(&group.pages));
        write_places(b, &ranks, &names);
    }));
    encoder().encode(&t, TreeEncoderOptions::default())
}

#[derive(Clone)]
pub struct PlaceGroupView<'a> {
    starts: &'a [u32],
    flat: &'a [u32],
    pages: &'a [u16],
    locals: PlacesView<'a>,
}

pub fn open_place_group(pkt: &[u8]) -> Result<PlaceGroupView<'_>, MarshalError> {
    let root = TreeReader::new(pkt)?.read_branch()?;
    let view = PlaceGroupView {
        starts: root.at(0)?.get_array::<u32>()?,
        flat: root.at(1)?.get_array::<u32>()?,
        pages: root.at(2)?.get_array::<u16>()?,
        locals: read_places(&root, 3)?,
    };
    if view.pages.len() != GROUP_PAGES {
        return Err(malformed());
    }
    if view.starts.is_empty() || view.starts.windows(2).any(|w| w[0] > w[1]) || *view.starts.last().or_bug(Errno::ENOTRECOVERABLE) as usize > view.flat.len() {
        return Err(malformed());
    }
    Ok(view)
}

impl<'a> PlaceGroupView<'a> {
    pub fn label_of(&self, slot: usize) -> Option<&'a [u32]> {
        let label = *self.pages.get(slot)?;
        if label == NO_LABEL {
            return None;
        }
        let from = *self.starts.get(label as usize)? as usize;
        let to = *self.starts.get(label as usize + 1)? as usize;
        Some(&self.flat[from..to])
    }

    /// Broad to narrow, read against the dictionary.
    pub fn place_of(&self, slot: usize, dict: &PlacesView<'a>) -> Option<impl Iterator<Item = PlaceRef<'a>>> {
        let label = self.label_of(slot)?;
        let (locals, dict) = (self.locals.clone(), dict.clone());
        Some(label.iter().filter_map(move |n| {
            let from = if n & LOCAL != 0 { &locals } else { &dict };
            from.at(n & !(APPROX | LOCAL)).map(|part| PlaceRef { approx: n & APPROX != 0, ..part })
        }))
    }
}


#[cfg(test)]
#[path = "place_page.test.rs"]
mod test;
