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
//! its language names start at, and the names themselves, so reaching the
//! thousandth place in the dictionary is an index rather than a walk.

use xpute_core::status::errno::Errno;
use xpute_core::status::error::MarshalError;
use xpute_core::wire::xtp::BranchCursor;
use xpute_core::wire::xtp::{encoder, BranchViewImpl, NodeView, TreeEncoderOptions, TreeReader, TreeView};

use crate::bake::quadkey::{quadkey_of, Quadkey};
use crate::place::PlaceRank;
use crate::space::pvf3::ivec::IVec;
use crate::space::pyramid::PAGE_ZOOM;
use xpute_core::status::bug::OrBug;

/// The one object the dictionary is under, beside the groups.
pub const DICT_OBJECT: &str = "_dict";

/// Levels a group is above a page: 16 pages on a side.
pub const GROUP_SHAMT: u32 = 4;

/// The zoom a group is named at: `GROUP_SHAMT` coarser than a page's.
pub const GROUP_ZOOM: u32 = PAGE_ZOOM - GROUP_SHAMT;

/// A group's name in its bake (bake/quadkey.rs).
pub fn group_name(group_idx: IVec) -> Quadkey {
    quadkey_of(GROUP_ZOOM, group_idx)
}
/// Pages under one group.
pub const GROUP_PAGES: usize = 1 << (2 * GROUP_SHAMT);

/// A leaf's place among its group's, from the low bits of its page index.
pub fn leaf_slot(x: i64, y: i64) -> usize {
    let mask = (1i64 << GROUP_SHAMT) - 1;
    ((y & mask) << GROUP_SHAMT | (x & mask)) as usize
}

/// A place the dictionary holds: what it is called and where it sits.
#[derive(Clone, Debug, PartialEq)]
pub struct Division {
    pub rank: PlaceRank,
    pub name: String,
    /// The same name by language tag, flattened for the wire: `(tag, name)`.
    pub names: Vec<(String, String)>,
}

/// The two number lanes of a run of places, which the caller holds while the
/// branch that borrows them is built.
struct PlaceLanes {
    ranks: Vec<u8>,
    /// Where each place's language names start among the flattened pairs,
    /// one longer than the places so the last one's end is there too.
    starts: Vec<u32>,
}

fn place_lanes(places: &[Division]) -> PlaceLanes {
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
    }
}

fn write_places<'a>(b: &mut BranchViewImpl<'a>, places: &'a [Division], lanes: &'a PlaceLanes) {
    b.u8_array(Some(&lanes.ranks)).u32_array(Some(&lanes.starts));
    b.branch(Some(&mut |s| {
        for d in places {
            s.str(Some(&d.name));
        }
    }));
    b.branch(Some(&mut |s| {
        for d in places {
            for (tag, name) in &d.names {
                s.str(Some(tag)).str(Some(name));
            }
        }
    }));
}

/// A run of places read where it lies. The dictionary is one of these and a
/// group's own places are another, so the two are written and read one way.
#[derive(Clone)]
pub struct PlacesView<'a> {
    ranks: &'a [u8],
    starts: &'a [u32],
    names: BranchCursor<'a>,
    langs: BranchCursor<'a>,
}

fn read_places<'a>(root: &BranchCursor<'a>, at: u32) -> Result<PlacesView<'a>, MarshalError> {
    let view = PlacesView {
        ranks: root.at(at)?.get_array::<u8>()?,
        starts: root.at(at + 1)?.get_array::<u32>()?,
        names: root.at(at + 2)?.as_branch()?,
        langs: root.at(at + 3)?.as_branch()?,
    };
    if view.starts.len() != view.ranks.len() + 1 {
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

    /// The place stored at `i`, or none where the packet has no readable one
    /// there.
    pub fn at(&self, i: u32) -> Option<PlaceRef<'a>> {
        Some(PlaceRef {
            rank: PlaceRank::of(*self.ranks.get(i as usize)?)?,
            name: self.names.at(i).ok()?.get_str_ref().ok()?,
            approx: false,
            langs: self.langs.clone(),
            from: *self.starts.get(i as usize)?,
            to: *self.starts.get(i as usize + 1)?,
        })
    }
}

/// One part of a label, read out of the packet it was stored in.
#[derive(Clone)]
pub struct PlaceRef<'a> {
    pub rank: PlaceRank,
    /// The provider's own name for the place, in whatever language it holds
    /// it in.
    pub name: &'a str,
    /// The part came from a place near the page rather than one containing
    /// it.
    pub approx: bool,
    langs: BranchCursor<'a>,
    from: u32,
    to: u32,
}

impl<'a> PlaceRef<'a> {
    /// Every language the provider had a name in, as `(tag, name)`.
    pub fn languages(&self) -> impl Iterator<Item = (&'a str, &'a str)> {
        let langs = self.langs.clone();
        (self.from..self.to).filter_map(move |i| {
            let tag = langs.at(i * 2).and_then(|c| c.get_str_ref()).ok()?;
            Some((tag, langs.at(i * 2 + 1).and_then(|c| c.get_str_ref()).ok()?))
        })
    }

    /// The name in `tag`'s language, or the provider's own where it has none
    /// — a name a reader cannot read beats no name at all.
    pub fn in_language(&self, tag: &str) -> &'a str {
        for i in self.from..self.to {
            let Ok(stored) = self.langs.at(i * 2).and_then(|c| c.get_str_ref()) else { continue };
            if stored == tag {
                if let Ok(name) = self.langs.at(i * 2 + 1).and_then(|c| c.get_str_ref()) {
                    return name;
                }
            }
        }
        self.name
    }
}

/// Every place a polygon names, in the order a group's numbers name them.
pub fn encode_place_dict(divisions: &[Division]) -> Result<Vec<u8>, MarshalError> {
    let lanes = place_lanes(divisions);
    let mut t: TreeView<'_> = TreeView::new();
    t.branch(Some(&mut |b| write_places(b, divisions, &lanes)));
    encoder().encode(&t, TreeEncoderOptions::default())
}

/// The dictionary read where it lies.
pub fn open_place_dict(pkt: &[u8]) -> Result<PlacesView<'_>, MarshalError> {
    read_places(&TreeReader::new(pkt)?.read_branch()?, 0)
}

/// One group: the labels its pages take, and which of them each page is in.
///
/// A label is a place read broad to narrow, by dictionary number. The high
/// bit of a number marks the part as an approximation — it came from a place
/// near the page rather than one containing it — so a label is a plain run of
/// numbers with nothing beside it.
#[derive(Clone, Debug, PartialEq)]
pub struct PlaceGroup {
    /// Broad-to-narrow numbers, a label each: a place in the dictionary, or
    /// with `APPROX` set, one of this group's own.
    pub labels: Vec<Vec<u32>>,
    /// The label each of the group's pages takes, or `NO_LABEL`.
    pub pages: Vec<u16>,
    /// The places no polygon named — the ones a page took from a point near
    /// it. They are this group's because almost none of the provider's are
    /// ever named, and the dictionary would be forty times the size to carry
    /// them all.
    pub locals: Vec<Division>,
}

/// What a page outside every place the provider knows takes.
pub const NO_LABEL: u16 = u16::MAX;
/// On a number: the part came from a place near the page rather than one
/// containing it, and the number names one of the group's own places rather
/// than one of the dictionary's.
pub const APPROX: u32 = 1 << 31;

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
        write_places(b, &group.locals, &lanes);
    }));
    encoder().encode(&t, TreeEncoderOptions::default())
}

/// One group read where it lies.
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
    /// The numbers naming the place page `slot` is in.
    pub fn label_of(&self, slot: usize) -> Option<&'a [u32]> {
        let label = *self.pages.get(slot)?;
        if label == NO_LABEL {
            return None;
        }
        let from = *self.starts.get(label as usize)? as usize;
        let to = *self.starts.get(label as usize + 1)? as usize;
        Some(&self.flat[from..to])
    }

    /// The place page `slot` is in, broad to narrow, read against the
    /// dictionary.
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

#[track_caller]
fn bad() -> MarshalError {
    MarshalError::new(Errno::EBADMSG)
}

#[cfg(test)]
#[path = "place_page.test.rs"]
mod test;
