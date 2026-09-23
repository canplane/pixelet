// pxl-kernel/layer/geocode/fetcher.rs

//! Demands the two packets a label is read out of: the dictionary, once a
//! session, and the group the map centre is in, whenever the centre leaves
//! the one held.
//!
//! The wishlist is two keys, so it is rebuilt every turn rather than guarded
//! against its inputs — the scheduler dedups against what it already carries,
//! and a key that has not changed costs a comparison.
//!
//! A key the store has any answer for is not asked about again, and the one
//! thing that re-opens a question is the centre moving to a different group
//! (store.rs). The group's own question re-opens by itself, since the store
//! holds one group and a different one is a different key; the dictionary's
//! has to be re-opened here.
//!
//! Both sit behind every terrain page. A label is chrome over the ground and
//! must not take a credit from it.

use pxl_spec::bake::place_page::GROUP_SHAMT;
use pxl_spec::space::indexer::L3_IDX;
use pxl_spec::space::pvf3::ivec::{self, IVec};

use crate::global::Global;
use crate::io::demand::{Category, Demand, DemandKey};
use crate::io::scheduler::{fetch_sync, page_key};
use crate::layer::geocode::store;
use crate::layer::terrain::tile_fetcher::BEHIND_TERRAIN;
use crate::shell::map_store::map_store;

pub const CATEGORY: Category = Category::Geocode;
pub const DICT_CATEGORY: Category = Category::GeocodeDict;

/// The dictionary is a stream of one, so its key names no page.
const DICT_SUBJECT: u64 = 0;

/// The group the map centre's page is in.
pub fn centre_group() -> Option<IVec> {
    map_store().map_center.map(|c| ivec::sh(L3_IDX(c.i), -(GROUP_SHAMT as i32)))
}

/// The group the last run saw, against which a move is a move.
static LAST_GROUP: Global<Option<IVec>> = Global::new();

/// Runs every turn: what the label needs and does not have.
pub fn geocode_fetcher_step() {
    let moved = {
        let last = LAST_GROUP.get(|| None);
        let now = centre_group();
        core::mem::replace(last, now) != now
    };
    if moved {
        store::forget_absent_dict();
    }

    let dict = match store::dict_answered() {
        true => Vec::new(),
        false => vec![Demand {
            key: DemandKey::new(DICT_CATEGORY, DICT_SUBJECT),
            priority: BEHIND_TERRAIN,
        }],
    };
    fetch_sync(DICT_CATEGORY, &dict);

    // Asked about once: a group the bake wrote no object for is an answer
    // the store keeps, not a hole to ask about again.
    let group = match centre_group() {
        Some(idx) if store::answered() != Some(idx) => vec![Demand {
            key: page_key(CATEGORY, idx),
            // Behind the dictionary, which it is read against: a group that
            // arrives first names places nothing can look up yet.
            priority: BEHIND_TERRAIN + 1.0,
        }],
        _ => Vec::new(),
    };
    fetch_sync(CATEGORY, &group);
}
