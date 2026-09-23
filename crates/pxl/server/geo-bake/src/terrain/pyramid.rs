// pxl-geo-bake/terrain/pyramid.rs

//! The levels above the leaves: a level-k page folded from the level-(k − 1)
//! pages under it, with no database in the loop.
//!
//! A level-k page covers 2^k × 2^k leaves at 1/2^k the resolution and is the
//! same record as a leaf, so its key is a page key like any other — its
//! south-west child's coordinate shifted right a level — and the level goes in
//! the path (issue/20260921-level-step-and-zoom-are-one-number.md). Where the
//! pages are read from and written to is the caller's; this is which parents a
//! level has, which pages each one reads, and the fold, which is pxl-spec's so
//! that the bake and the client compose the same one-level function.

use std::collections::HashMap;

use pxl_spec::bake::constant::ELE_PADDED_SPAN;
use pxl_spec::bake::fold::{fold_ele, fold_ground};
use pxl_spec::bake::format::{Ground, Terrain};
use pxl_spec::space::indexer::L3_SHAMT;
use pxl_spec::space::pvf3::core::IVEC_X_W;
use pxl_spec::space::pvf3::ivec::{self, IVec};

use crate::terrain::tile_set::TileSet;

/// Pages across the world at `level`: the X lane is one world, and each level
/// halves how many pages cross it.
fn pages_across(level: u32) -> i64 {
    1i64 << (IVEC_X_W - L3_SHAMT - level)
}

/// X wrapped into the level's signed range. The lane is cyclic, so the page
/// west of the westmost is the eastmost; Y is clamped rather than cyclic, and
/// a page past its edge simply has no object.
fn wrap_x(x: i64, level: u32) -> i64 {
    let n = pages_across(level);
    (x + n / 2).rem_euclid(n) - n / 2
}

/// The page a level-(k − 1) page folds into at level k: its coordinate
/// shifted right, which on the signed lanes is a floor.
pub fn parent_of(page: IVec) -> IVec {
    ivec::gather(ivec::X(page).div_euclid(2), ivec::Y(page).div_euclid(2), 0)
}

/// Every parent a level has: one for each child that exists, in key order.
pub fn parents(children: impl IntoIterator<Item = IVec>) -> Vec<IVec> {
    let mut out: Vec<IVec> = children.into_iter().map(parent_of).collect();
    out.sort_unstable();
    out.dedup();
    out
}

/// The 4×4 level-(k − 1) pages around a level-k parent's children, in
/// `fold_ele`'s order: index (qx + 1) + 4·(qy + 1) is the page at
/// (2X + qx, 2Y + qy), the parent's own four being 5, 6, 9 and 10.
pub fn neighbourhood(parent: IVec, level: u32) -> [IVec; 16] {
    let (x, y) = (ivec::X(parent), ivec::Y(parent));
    std::array::from_fn(|k| {
        let (qx, qy) = ((k % 4) as i64 - 1, (k / 4) as i64 - 1);
        ivec::gather(wrap_x(2 * x + qx, level - 1), 2 * y + qy, 0)
    })
}

/// The parent's own four children, in `neighbourhood`'s order.
pub const OWN: [usize; 4] = [5, 6, 9, 10];

/// A level-k parent folded, or none where it has no child: a parent exists
/// exactly where one of its four children does, which is the rule the bake's
/// `parents` walks by and the dev server faults by. `ground` answers for the
/// parent's own four children and `ele` for all sixteen pages around them. A
/// page either one has no answer for is open water, which the bake leaves as
/// no object, and it reads as `ocean`.
pub fn fold_parent<'a>(parent: IVec, level: u32, ground: impl Fn(IVec) -> Option<&'a Ground>, ele: impl Fn(IVec) -> Option<&'a [i16]>, ocean: &'a Terrain) -> Option<Terrain> {
    let around = neighbourhood(parent, level);
    let own = OWN.map(|k| ground(around[k]));
    if own.iter().all(Option::is_none) {
        return None;
    }
    let mut out = vec![0i16; (ELE_PADDED_SPAN * ELE_PADDED_SPAN) as usize];
    // Always Some here: the bake has every page of the pyramid, and a page it
    // left as no object is open water and reads as `ocean`. What `fold_ele`
    // takes a None for is the client folding from what it happens to hold.
    fold_ele(around.map(|p| Some(ele(p).unwrap_or(&*ocean.ele))), &mut out);
    Some(Terrain {
        ground: fold_ground(own.map(|g| g.unwrap_or(&ocean.ground)), level),
        ele: out.into(),
    })
}

/// A level-k parent folded from the level below as a tile set holds it.
pub fn fold_stored(tiles: &TileSet, parent: IVec, level: u32, ocean: &Terrain) -> Result<Option<Terrain>, String> {
    let around = neighbourhood(parent, level);
    let mut read: HashMap<IVec, Terrain> = HashMap::new();
    for page in around {
        if let Some(t) = tiles.read(level - 1, page)? {
            read.insert(page, t);
        }
    }
    Ok(fold_parent(parent, level, |p| read.get(&p).map(|t| &t.ground), |p| read.get(&p).map(|t| &*t.ele), ocean))
}

#[cfg(test)]
#[path = "pyramid.test.rs"]
mod test;
