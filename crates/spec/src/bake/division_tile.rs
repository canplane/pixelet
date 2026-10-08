// pxl-spec/bake/division_tile.rs

//! The reverse-geocode bake's output: a dictionary of the divisions many
//! tiles share, fetched once, and one object per group of 16 × 16 leaf tiles
//! naming the division each tile is in, with the group's own divisions, those
//! no other group needs. A division names its parent in the dictionary, so a
//! name is read by walking up it. Readers view the packet in place; names are
//! XTP string leaves so a division is reached by index.

use xpute_core::status::error::MarshalError;
use xpute_core::wire::xtp::BranchCursor;
use xpute_core::wire::xtp::{encoder, BranchViewImpl, NodeView, TreeEncoderOptions, TreeReader, TreeView};

use crate::bake::malformed;
use crate::bake::named::{NameLanes, NamesView};

use crate::division::Subtype;
use crate::space::coord::pvf3::ivec::{self, IVec};
use crate::space::coord::zoom::Zoom;
use crate::space::layer::tile::{tile_at, LEAF_ZOOM};

pub const DICT_OBJECT: &str = "_dict";

/// 16 tiles on a side.
pub const GROUP_SHAMT: u32 = 4;

/// A group is the tile at this zoom, and named as one.
pub const GROUP_ZOOM: Zoom = LEAF_ZOOM - GROUP_SHAMT as Zoom;

pub fn group_key(group_idx: IVec) -> u128 {
    tile_at(GROUP_ZOOM, [ivec::X(group_idx), ivec::Y(group_idx)]).key()
}
pub const GROUP_TILES: usize = 1 << (2 * GROUP_SHAMT);

/// From the low bits of its tile index.
pub fn leaf_slot(x: i64, y: i64) -> usize {
    let mask = (1i64 << GROUP_SHAMT) - 1;
    ((y & mask) << GROUP_SHAMT | (x & mask)) as usize
}

/// A division with no parent, a country; and a tile in none.
pub const NONE: u32 = u32::MAX;
/// A tile's division is the group's own, numbered among its locals.
pub const LOCAL: u32 = 1 << 31;
/// A tile's division is the one nearest it rather than one containing it.
pub const APPROX: u32 = 1 << 30;

/// A name is never deeper than the subtypes, so a walk longer is a cycle.
const DEEPEST: usize = Subtype::ALL.len();

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct Division {
    pub subtype: Subtype,
    pub name: String,
    /// `(tag, name)` pairs.
    pub names: Vec<(String, String)>,
    /// Its parent's number in the dictionary, `NONE` for none.
    pub parent: u32,
}

struct Lanes<'a> {
    subtypes: Vec<u8>,
    parents: Vec<u32>,
    names: NameLanes<'a>,
}

fn lanes(divisions: &[Division]) -> Lanes<'_> {
    Lanes {
        subtypes: divisions.iter().map(|d| d.subtype as u8).collect(),
        parents: divisions.iter().map(|d| d.parent).collect(),
        names: NameLanes::of(divisions.iter().map(|d| (d.name.as_str(), d.names.iter().map(|(tag, name)| (tag.as_str(), name.as_str()))))),
    }
}

fn write_divisions<'a>(b: &mut BranchViewImpl<'a>, lanes: &'a Lanes<'a>) {
    b.u8_array(Some(&lanes.subtypes)).u32_array(Some(&lanes.parents));
    lanes.names.write(b);
}

/// A run of divisions read where it lies: the dictionary or a group's own.
#[derive(Clone, Copy)]
pub struct DivisionsView<'a> {
    subtypes: &'a [u8],
    parents: &'a [u32],
    names: NamesView<'a>,
}

fn read_divisions<'a>(root: &BranchCursor<'a>, at: u32) -> Result<DivisionsView<'a>, MarshalError> {
    let subtypes = root.at(at)?.get_array::<u8>()?;
    let parents = root.at(at + 1)?.get_array::<u32>()?;
    if parents.len() != subtypes.len() || subtypes.iter().any(|&s| Subtype::of(s).is_none()) {
        return Err(malformed());
    }
    Ok(DivisionsView {
        subtypes,
        parents,
        names: NamesView::read(root, at + 2, subtypes.len())?,
    })
}

impl<'a> DivisionsView<'a> {
    pub fn len(&self) -> usize {
        self.subtypes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.subtypes.is_empty()
    }

    pub fn at(&self, i: u32) -> Option<DivisionRef<'a>> {
        Some(DivisionRef {
            subtype: Subtype::of(*self.subtypes.get(i as usize)?)?,
            name: self.names.name(i)?,
            parent: *self.parents.get(i as usize)?,
            approx: false,
            names: self.names,
            at: i,
        })
    }
}

#[derive(Clone, Copy)]
pub struct DivisionRef<'a> {
    pub subtype: Subtype,
    /// The source's own name, in whatever language it holds.
    pub name: &'a str,
    /// Its parent's number in the dictionary, `NONE` for none.
    pub parent: u32,
    /// The division nearest the tile rather than one containing it.
    pub approx: bool,
    names: NamesView<'a>,
    at: u32,
}

impl<'a> DivisionRef<'a> {
    pub fn languages(&self) -> impl Iterator<Item = (&'a str, &'a str)> {
        self.names.languages(self.at)
    }

    /// Falls back to the source's own name: an unreadable name beats none.
    pub fn in_language(&self, tag: &str) -> &'a str {
        self.languages().find(|&(stored, _)| stored == tag).map_or(self.name, |(_, name)| name)
    }
}

/// In the order a group's and a division's numbers name them; every parent
/// is in it.
pub fn encode_division_dict(divisions: &[Division]) -> Result<Vec<u8>, MarshalError> {
    let lanes = lanes(divisions);
    let mut t: TreeView<'_> = TreeView::new();
    t.branch(Some(&mut |b| write_divisions(b, &lanes)));
    encoder().encode(&t, TreeEncoderOptions::default())
}

/// Refused unless every parent is in it and no walk up it is a cycle.
pub fn open_division_dict(pkt: &[u8]) -> Result<DivisionsView<'_>, MarshalError> {
    let view = read_divisions(&TreeReader::new(pkt)?.read_branch()?, 0)?;
    for i in 0..view.len() as u32 {
        let mut at = i;
        for step in 0.. {
            if step > DEEPEST {
                return Err(malformed());
            }
            match view.parents[at as usize] {
                NONE => break,
                p if p as usize >= view.len() => return Err(malformed()),
                p => at = p,
            }
        }
    }
    Ok(view)
}

/// One group: each tile's division, `NONE`, a dictionary number, or with
/// `LOCAL` one of the group's own; `APPROX` marks the nearest rather than
/// the containing. A division of the group's own has its parent in the
/// dictionary.
#[derive(Clone, Debug, PartialEq)]
pub struct DivisionGroup {
    pub tiles: Vec<u32>,
    pub locals: Vec<Division>,
}

pub fn encode_division_group(group: &DivisionGroup) -> Result<Vec<u8>, MarshalError> {
    let lanes = lanes(&group.locals);
    let mut t: TreeView<'_> = TreeView::new();
    t.branch(Some(&mut |b| {
        b.u32_array(Some(&group.tiles));
        write_divisions(b, &lanes);
    }));
    encoder().encode(&t, TreeEncoderOptions::default())
}

#[derive(Clone, Copy)]
pub struct DivisionGroupView<'a> {
    tiles: &'a [u32],
    locals: DivisionsView<'a>,
}

pub fn open_division_group(pkt: &[u8]) -> Result<DivisionGroupView<'_>, MarshalError> {
    let root = TreeReader::new(pkt)?.read_branch()?;
    let view = DivisionGroupView {
        tiles: root.at(0)?.get_array::<u32>()?,
        locals: read_divisions(&root, 1)?,
    };
    let local_ok = |n: u32| n == NONE || n & LOCAL == 0 || ((n & !(LOCAL | APPROX)) as usize) < view.locals.len();
    if view.tiles.len() != GROUP_TILES || !view.tiles.iter().all(|&n| local_ok(n)) {
        return Err(malformed());
    }
    Ok(view)
}

impl<'a> DivisionGroupView<'a> {
    /// The tile's division, then each ancestor, narrow to broad; a parent the
    /// dictionary lacks ends it.
    fn up(&self, slot: usize, dict: DivisionsView<'a>) -> impl Iterator<Item = DivisionRef<'a>> {
        let n = self.tiles.get(slot).copied().unwrap_or(NONE);
        let first = match n {
            NONE => None,
            n if n & LOCAL != 0 => self.locals.at(n & !(LOCAL | APPROX)),
            n => dict.at(n & !APPROX),
        }
        .map(|d| DivisionRef { approx: n & APPROX != 0, ..d });
        core::iter::successors(first, move |d| dict.at(d.parent)).take(DEEPEST)
    }

    /// Broad to narrow, read against the dictionary; None outside every division.
    pub fn name_of(&self, slot: usize, dict: &DivisionsView<'a>) -> Option<impl Iterator<Item = DivisionRef<'a>>> {
        let depth = self.up(slot, *dict).count();
        let (this, dict) = (*self, *dict);
        (depth > 0).then(move || (0..depth).rev().filter_map(move |k| this.up(slot, dict).nth(k)))
    }
}

#[cfg(test)]
#[path = "division_tile.test.rs"]
mod test;
