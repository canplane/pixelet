// pxl-kernel/layer/geocode/fetcher.rs

//! Demands the dictionary, once a session, and the group the asker's center
//! is in. A key with any answer is not asked again. Both sit behind every
//! terrain tile: a label must not take a credit from the ground.

use pxl_spec::bake::place_tile::GROUP_SHAMT;
use pxl_spec::space::coord::pvf3::ivec::{self, IVec};
use pxl_spec::space::coord::pvf3::Pvf3;

use crate::io::demand::{Ask, Category};
use crate::io::sched::declare;
use crate::layer::geocode::store;
use pxl_spec::space::coord::zoom::idx;
use pxl_spec::space::layer::tile::LEAF_ZOOM;

const DICT_SUBJECT: u64 = 0;

/// The group a point is in.
pub fn group_of(at: &Pvf3) -> IVec {
    ivec::sh(idx(LEAF_ZOOM, at.i), -(GROUP_SHAMT as i32))
}

/// Declared every turn: an unchanged set changes nothing, so nothing is
/// remembered between turns.
pub fn geocode_fetcher_step(center: Option<IVec>) {
    let dict = [(DICT_SUBJECT, Ask::PLAIN)];
    declare(Category::geocode_dict, if store::dict_answered() { &[] } else { &dict });

    match center {
        Some(idx) if store::answered() != Some(idx) => declare(Category::geocode, &[(idx, Ask::PLAIN)]),
        _ => declare(Category::geocode, &[]),
    }
}
