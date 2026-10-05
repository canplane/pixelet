// pxl-kernel/layer/geocode/fetcher.rs

//! Demands the dictionary, once a session, and the group the map center is
//! in. A key with any answer is not asked again. Both sit behind every
//! terrain tile: a label must not take a credit from the ground.

use pxl_spec::bake::place_tile::GROUP_SHAMT;
use pxl_spec::space::pvf3::ivec::{self, IVec};

use crate::io::demand::{Ask, Category};
use crate::io::sched::declare;
use crate::layer::geocode::store;
use pxl_spec::space::indexer::idx;
use pxl_spec::space::pyramid::LEAF_Z;

const DICT_SUBJECT: u64 = 0;

pub fn center_group() -> Option<IVec> {
    crate::control::camera::controller::map_center().map(|c| ivec::sh(idx(LEAF_Z, c.i), -(GROUP_SHAMT as i32)))
}

/// Declared every turn: an unchanged set changes nothing, so nothing is
/// remembered between turns.
pub fn geocode_fetcher_step() {
    let dict = [(DICT_SUBJECT, Ask::PLAIN)];
    declare(Category::geocode_dict, if store::dict_answered() { &[] } else { &dict });

    match center_group() {
        Some(idx) if store::answered() != Some(idx) => declare(Category::geocode, &[(idx, Ask::PLAIN)]),
        _ => declare(Category::geocode, &[]),
    }
}
