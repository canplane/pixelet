// pxl-kernel/layer/geocode/store.rs

//! What the layer holds: the dictionary, and the one group the map centre is
//! in.
//!
//! One group rather than a table of them. A group covers sixteen pages a
//! side, so panning crosses one rarely, and the object behind it is cached
//! by the edge and the browser — holding the ones a viewer has left would
//! buy a fetch they mostly do not make, at the cost of a residency rule to
//! get wrong.
//!
//! **Every outcome is an answer**, whether an object came back, the bake
//! wrote none, or the fetch failed. A demand is made from "this key has no
//! answer", so an outcome that left none would be re-made the turn after it
//! was refused: measured at 120 requests a second with the tile server down,
//! which against the bucket is a budget that fails closed reached in a
//! quarter of an hour (issue/20260816-deployment-hosting.md). What re-opens
//! a question is the centre moving to a different group — the one event that
//! says the viewer is still looking for a label — so a retry runs at the
//! speed a hand pans rather than at the turn rate.

use pxl_spec::space::pvf3::ivec::IVec;

use crate::global::Global;
use crate::layer::geocode::repository::{ResidentDict, ResidentGroup};

#[derive(Default)]
struct GeocodeStore {
    /// Whether the dictionary has been asked about, and what came back: a
    /// bake with no `_dict` object answers 404, which is an answer and not a
    /// hole to ask about again. Every read through the bucket is a Worker
    /// request against a budget that fails closed
    /// (issue/20260816-deployment-hosting.md), so a permanent absence has to
    /// end the asking.
    dict: Option<Option<ResidentDict>>,
    /// The group the answer below is for, and what it was: none where the
    /// bake wrote no object for it.
    group: Option<(IVec, Option<ResidentGroup>)>,
}

static STORE: Global<GeocodeStore> = Global::new();

fn store() -> &'static mut GeocodeStore {
    STORE.get(GeocodeStore::default)
}

pub fn dict() -> Option<&'static ResidentDict> {
    store().dict.as_ref()?.as_ref()
}

/// Whether the dictionary has been answered for, whatever the answer was.
pub fn dict_answered() -> bool {
    store().dict.is_some()
}

/// Forgets a dictionary answered with nothing, so it is asked about again.
/// A dictionary that arrived is left alone: it does not go stale.
pub fn forget_absent_dict() {
    if let Some(None) = store().dict {
        store().dict = None;
    }
}

/// Whatever was held is dropped, which is what gives its block back.
pub fn set_dict(dict: Option<ResidentDict>) {
    store().dict = Some(dict);
}

/// A dictionary that did not arrive answers only where nothing had. The
/// stream is one key, so a failure and an arrival name the same slot — and
/// a demand is not made while the slot is full, which is what makes this
/// unreachable rather than merely unlikely. It is written down because what
/// it would cost is a freed packet a view still points into.
pub fn dict_failed() {
    if !dict_answered() {
        store().dict = Some(None);
    }
}

/// The group the store has an answer for, whatever the answer was.
pub fn answered() -> Option<IVec> {
    store().group.as_ref().map(|(idx, _)| *idx)
}

/// The group the centre is in, where it is the one held and it has an
/// object.
pub fn group_of(group_idx: IVec) -> Option<&'static ResidentGroup> {
    match &store().group {
        Some((idx, held)) if *idx == group_idx => held.as_ref(),
        _ => None,
    }
}

/// As `set_dict`: the group this one replaces gives its block back with it.
pub fn set_group(group_idx: IVec, group: Option<ResidentGroup>) {
    store().group = Some((group_idx, group));
}
