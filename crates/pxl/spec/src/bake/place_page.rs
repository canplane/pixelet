// pxl-spec/bake/place_page.rs

//! The reverse-geocode bake's output: a dictionary of places named by many
//! pages, fetched once, and one object per group of 16 × 16 leaf pages
//! holding its labels, a label index per page and its own local places.
//! Readers view the packet in place; names are XTP string leaves so a
//! place is reached by index.

use xpute_core::status::errno::Errno;
use xpute_core::status::error::MarshalError;
use xpute_core::wire::xtp::BranchCursor;
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

struct PlaceLanes<'a> {
    ranks: Vec<u8>,
    /// Where each place's language names start among the pairs; one longer than the places.
    starts: Vec<u32>,
    names: Vec<&'a str>,
    langs: Vec<&'a str>,
}

fn place_lanes(places: &[Named]) -> (Vec<u8>, NameLanes<'_>) {
    (
        places.iter().map(|d| d.rank as u8).collect(),
        NameLanes::of(places.iter().map(|d| (d.name.as_str(), d.names.iter().map(|(tag, name)| (tag.as_str(), name.as_str()))))),
    )
}

/// A run of places read where it lies: the dictionary or a group's locals.
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

#[derive(Clone)]
pub struct PlaceRef<'a> {
    pub rank: PlaceRank,
    /// The source's own name, in whatever language it holds.
    pub name: &'a str,
    /// From a place near the page rather than one containing it.
    pub approx: bool,
    langs: BranchCursor<'a>,
    from: u32,
    to: u32,
}

impl<'a> PlaceRef<'a> {
    pub fn languages(&self) -> impl Iterator<Item = (&'a str, &'a str)> {
        let langs = self.langs.clone();
        (self.from..self.to).filter_map(move |i| {
            let tag = langs.at(i * 2).and_then(|c| c.get_str_ref()).ok()?;
            Some((tag, langs.at(i * 2 + 1).and_then(|c| c.get_str_ref()).ok()?))
        })
    }

    /// Falls back to the source's own name: an unreadable name beats none.
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

/// In the order a group's numbers name them.
pub fn encode_place_dict(places: &[Named]) -> Result<Vec<u8>, MarshalError> {
    let lanes = place_lanes(places);
    let mut t: TreeView<'_> = TreeView::new();
    t.branch(Some(&mut |b| write_places(b, divisions, &lanes)));
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
        write_places(b, &group.locals, &lanes);
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
