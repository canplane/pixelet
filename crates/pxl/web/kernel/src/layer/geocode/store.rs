// pxl-kernel/layer/geocode/store.rs

//! The dictionary, and the one group the map center is in: a group spans
//! sixteen pages a side, so holding more buys little. Every outcome, a 404
//! or a failed fetch included, is an answer: a missing one was re-asked
//! every turn (120 requests/s with the tile server down, which would exhaust
//! the bucket's fail-closed budget in minutes). Only moving to another group
//! asks again.

use pxl_spec::space::pvf3::ivec::IVec;

use crate::global::Global;
use crate::layer::geocode::repository::{ResidentDict, ResidentGroup};

#[derive(Default)]
struct GeocodeStore {
    /// A 404 is an answer, not a hole to ask about again.
    dict: Option<Option<ResidentDict>>,
    /// None inside where the bake wrote no object.
    group: Option<(IVec, Option<ResidentGroup>)>,
}

static STORE: Global<GeocodeStore> = Global::new();

fn store() -> &'static mut GeocodeStore {
    STORE.get(GeocodeStore::default)
}

pub fn dict() -> Option<&'static ResidentDict> {
    store().dict.as_ref()?.as_ref()
}

pub fn dict_answered() -> bool {
    store().dict.is_some()
}

/// Dropping what was held gives its block back.
pub fn set_dict(dict: Option<ResidentDict>) {
    store().dict = Some(dict);
}

pub fn answered() -> Option<IVec> {
    store().group.as_ref().map(|(idx, _)| *idx)
}

/// Only where it is the one held and has an object.
pub fn group_of(group_idx: IVec) -> Option<&'static ResidentGroup> {
    match &store().group {
        Some((idx, held)) if *idx == group_idx => held.as_ref(),
        _ => None,
    }
}

pub fn set_group(group_idx: IVec, group: Option<ResidentGroup>) {
    store().group = Some((group_idx, group));
}
