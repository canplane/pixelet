// pxl-kernel/layer/geocode/fetcher.rs

//! Demands the dictionary, once a session, and the group the map center is
//! in. A key with any answer is not asked again. Both sit behind every
//! terrain page: a label must not take a credit from the ground.

use pxl_spec::bake::place_page::GROUP_SHAMT;
use pxl_spec::space::indexer::L3_IDX;
use pxl_spec::space::pvf3::ivec::{self, IVec};

use crate::io::demand::{Ask, Category};
use crate::io::sched::declare;
use crate::layer::geocode::store;

pub const CATEGORY: Category = Category::geocode;
pub const DICT_CATEGORY: Category = Category::geocode_dict;

const DICT_SUBJECT: u64 = 0;

pub fn center_group() -> Option<IVec> {
    crate::control::camera::controller::map_center().map(|c| ivec::sh(L3_IDX(c.i), -(GROUP_SHAMT as i32)))
}

/// Declared every turn: an unchanged set changes nothing, so nothing is
/// remembered between turns.
pub fn geocode_fetcher_step() {
    let dict = [(DICT_SUBJECT, Ask::PLAIN)];
    declare(DICT_CATEGORY, if store::dict_answered() { &[] } else { &dict });

    match center_group() {
        Some(idx) if store::answered() != Some(idx) => declare(CATEGORY, &[(idx, Ask::PLAIN)]),
        _ => declare(CATEGORY, &[]),
    }
}
