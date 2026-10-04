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
//! The **dictionary** holds every place a polygon names — the provider's
//! divisions — once: its rank, its own name, and that name by language. It is
//! fetched once and held, so a group carries numbers rather than names.
//!
//! A part that came from a place *near* a page rather than one containing it
//! is not among those: it comes from the provider's points, a different set
//! and forty times the size, almost none of it ever named. A group carries
//! the few it needs itself, and the bit that marks a part as an
//! approximation is the bit that says where to look — the two are the same
//! set, so one bit does both.
//!
//! ```text
//!   dictionary   every place a polygon names, once
//!   group        labels: [place, ...] each   what the pages under it are in
//!                page[256] -> a label
//!                the places no polygon named, this group's own
//! ```
//!
//! The bake writes from the owned `Division` and `PlaceGroup`; a reader opens
//! a view over the packet where it lies and reads a name out of it without
//! decoding the rest. A run of places is stored as a rank a place, the place
//! its language names start at, and the names themselves — each set of
//! strings one leaf of offsets and UTF-8 (XTP's STRS) rather than a node a
//! string — so reaching the thousandth place in the dictionary is an index
//! rather than a walk.

use xpute_core::status::errno::Errno;
use xpute_core::status::error::MarshalError;
use xpute_core::wire::xtp::{BranchCursor, Strs};
use xpute_core::wire::xtp::{encoder, BranchViewImpl, NodeView, TreeEncoderOptions, TreeReader, TreeView};

use crate::bake::quadkey::{quadkey_of, Quadkey};
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

#[derive(Clone, Debug, PartialEq)]
pub struct Division {
    pub rank: PlaceRank,
    pub name: String,
    /// `(tag, name)` pairs.
    pub names: Vec<(String, String)>,
}

/// The lanes of a run of places, which the caller holds while the branch
/// that borrows them is built.
struct PlaceLanes<'a> {
    ranks: Vec<u8>,
    /// Where each place's language names start among the pairs; one longer than the places.
    starts: Vec<u32>,
    names: Vec<&'a str>,
    /// `(tag, name)` pairs, flattened.
    langs: Vec<&'a str>,
}

fn place_lanes(places: &[Division]) -> PlaceLanes<'_> {
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
    starts: &'a [u32],
    names: Strs<'a>,
    langs: Strs<'a>,
}

fn read_places<'a>(root: &BranchCursor<'a>, at: u32) -> Result<PlacesView<'a>, MarshalError> {
    let view = PlacesView {
        ranks: root.at(at)?.get_array::<u8>()?,
        starts: root.at(at + 1)?.get_array::<u32>()?,
        names: root.at(at + 2)?.get_strs()?,
        langs: root.at(at + 3)?.get_strs()?,
    };
    if view.starts.len() != view.ranks.len() + 1 || view.names.len() as usize != view.ranks.len() {
        return Err(bad());
    }
    if view.starts.windows(2).any(|w| w[0] > w[1]) {
        return Err(bad());
    }
    Ok(view)
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
            name: self.names.get(i)?,
            approx: false,
            langs: self.langs,
            from: *self.starts.get(i as usize)?,
            to: *self.starts.get(i as usize + 1)?,
        })
    }
}

#[derive(Clone)]
pub struct PlaceRef<'a> {
    pub rank: PlaceRank,
    /// The source's own name, in whatever language it holds.
    pub name: &'a str,
    /// From a place near the page rather than one containing it.
    pub approx: bool,
    langs: Strs<'a>,
    from: u32,
    to: u32,
}

impl<'a> PlaceRef<'a> {
    pub fn languages(&self) -> impl Iterator<Item = (&'a str, &'a str)> {
        let langs = self.langs;
        (self.from..self.to).filter_map(move |i| Some((langs.get(i * 2)?, langs.get(i * 2 + 1)?)))
    }

    /// Falls back to the source's own name: an unreadable name beats none.
    pub fn in_language(&self, tag: &str) -> &'a str {
        self.languages().find(|&(stored, _)| stored == tag).map_or(self.name, |(_, name)| name)
    }
}

/// In the order a group's numbers name them.
pub fn encode_place_dict(places: &[Named]) -> Result<Vec<u8>, MarshalError> {
    let lanes = place_lanes(places);
    let mut t: TreeView<'_> = TreeView::new();
    t.branch(Some(&mut |b| write_places(b, &lanes)));
    encoder().encode(&t, TreeEncoderOptions::default())
}

pub fn open_place_dict(pkt: &[u8]) -> Result<PlacesView<'_>, MarshalError> {
    read_places(&TreeReader::new(pkt)?.read_branch()?, 0)
}

/// One group. A label is a broad-to-narrow run of place numbers, each a
/// dictionary index or, with `LOCAL`, a local one; `APPROX` marks an
/// approximate part.
#[derive(Clone, Debug, PartialEq)]
pub struct PlaceGroup {
    pub labels: Vec<Vec<u32>>,
    /// Per page; `NO_LABEL` outside every known place.
    pub pages: Vec<u16>,
    pub locals: Vec<Named>,
}

pub const NO_LABEL: u16 = u16::MAX;
/// Marks a group-local place. The high bit, which groups baked before
/// `APPROX` existed also set on every local.
pub const LOCAL: u32 = 1 << 31;
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
    let lanes = place_lanes(&group.locals);
    let mut t: TreeView<'_> = TreeView::new();
    t.branch(Some(&mut |b| {
        b.u32_array(Some(&starts)).u32_array(Some(&flat)).u16_array(Some(&group.pages));
        write_places(b, &lanes);
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
        return Err(bad());
    }
    if view.starts.is_empty() || view.starts.windows(2).any(|w| w[0] > w[1]) || *view.starts.last().or_bug(Errno::ENOTRECOVERABLE) as usize > view.flat.len() {
        return Err(bad());
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
            let near = n & APPROX != 0;
            let from = if near { &locals } else { &dict };
            from.at(n & !APPROX).map(|part| PlaceRef { approx: near, ..part })
        }))
    }
}

#[cfg(test)]
#[path = "place_page.test.rs"]
mod test;
