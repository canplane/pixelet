// pxl-kernel/layer/geocode/fetcher.rs

//! Demands the two packets a label is read out of: the dictionary, once a
//! session, and the group the map center is in, whenever the center leaves
//! the one held.
//!
//! The wishlist is two keys, so it is rebuilt every turn rather than guarded
//! against its inputs — the scheduler dedups against what it already carries,
//! and a key that has not changed costs a comparison.
//!
//! A key the store has any answer for is not asked about again, and the one
//! thing that re-opens a question is the center moving to a different group
//! (store.rs). The group's own question re-opens by itself, since the store
//! holds one group and a different one is a different key; the dictionary's
//! has to be re-opened here.
//!
//! Both sit behind every terrain page. A label is chrome over the ground and
//! must not take a credit from it.

use pxl_spec::bake::place_page::GROUP_SHAMT;
use pxl_spec::space::indexer::L3_IDX;
use pxl_spec::space::pvf3::ivec::{self, IVec};

use crate::io::demand::Category;
use crate::io::policy;
use crate::io::sched::declare;
use crate::layer::geocode::store;
use crate::shell::map_store::map_store;

pub const CATEGORY: Category = Category::geocode;
pub const DICT_CATEGORY: Category = Category::geocode_dict;

/// The dictionary is a stream of one, so its key names no page.
const DICT_SUBJECT: u64 = 0;

/// The group the map center's page is in.
pub fn center_group() -> Option<IVec> {
    map_store().map_center.map(|c| ivec::sh(L3_IDX(c.i), -(GROUP_SHAMT as i32)))
}

/// Runs every turn: what the label needs and does not have.
///
/// Said again every turn, whether or not it changed. A set that is the same
/// set changes nothing, and a failure leaves the want standing, so there is
/// nothing here to remember between turns.
pub fn geocode_fetcher_step() {
    // Answered once, either way: a bake with no dictionary is a bake whose
    // groups name nothing, and that is a store's answer rather than a hole.
    let dict = [(DICT_SUBJECT, policy::geocode())];
    declare(DICT_CATEGORY, if store::dict_answered() { &[] } else { &dict });

    match center_group() {
        Some(idx) if store::answered() != Some(idx) => declare(CATEGORY, &[(idx, policy::geocode())]),
        _ => declare(CATEGORY, &[]),
    }
}
