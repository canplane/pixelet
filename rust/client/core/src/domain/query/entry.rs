// pxl-core/domain/query/entry.rs

//! What is placed, read as the records an edit would change. This is the
//! world's answer for what a player wrote, as opposed to what the assets
//! compose to (`node.rs`).

use pxl_spec::object::is_air;
use pxl_spec::space::coord::aabb::Aabb;
use pxl_spec::space::coord::zoom::Zoom;

use super::decor::claim;
use super::world::World;
use crate::domain::layer::object::meta::decor_instance;
use crate::domain::layer::object::store::Entry;
use crate::domain::query::decor::box_extent;
use pxl_spec::space::layer::octant::Octant;

impl World {
    /// What is placed at exactly `at`, air left out.
    pub fn entry_at(&self, at: Octant) -> Option<Entry> {
        let node = (at.zoom(), at.at());
        self.object
            .entry_at(node.0, node.1, self.object.cursor(node))
            .filter(|e| !is_air(e.state))
    }

    /// Calls `visit` with everything placed whose cube meets `space`, air
    /// left out, with X on the lap of `space`.
    pub fn placed_in(&self, space: &Aabb, visit: &mut dyn FnMut(&Entry)) {
        self.object.entries_in(space, &mut |e| {
            if !is_air(e.state) {
                visit(e);
            }
        });
    }

    /// The finest zoom anything is placed at in `space`. Air counts, since a
    /// dig that cut finer is held as finely as a placed cube.
    pub(super) fn finest_in(&self, space: &Aabb) -> Option<Zoom> {
        let mut finest = None;
        self.object
            .entries_in(space, &mut |e| finest = finest.max(Some(e.zoom)));
        finest
    }

    /// A marker's text as it was put up. None for any other entry.
    pub fn text(&self, e: &Entry) -> Option<&'static str> {
        self.object.text(e)
    }

    /// The first placed entry that `which` accepts and whose own room shares
    /// `space`, air left out. An entry's room is what its kind says it
    /// occupies (`occupies`).
    pub fn placed_occupying(&self, space: &Aabb, which: impl Fn(&Entry) -> bool) -> Option<Entry> {
        // A decor's box reaches up to `box_extent` past its min corner, so an
        // entry whose cell starts that far below `space` may still reach it.
        let around = Aabb {
            min: space.min.map(|v| v - box_extent()),
            max: space.max,
        };
        let mut met = None;
        self.object.entries_in(&around, &mut |e| {
            if met.is_none()
                && !is_air(e.state)
                && which(e)
                && occupies(e).is_some_and(|room| room.shares(*space))
            {
                met = Some(*e);
            }
        });
        met
    }
}

/// The room a placed entry occupies, as its kind has it. A decor occupies
/// its box (`decor::claim`). The box always holds the decor's own cell, at
/// the corner it stands on, since every variant is a cell or more on a side.
/// Anything else occupies its cell.
fn occupies(e: &Entry) -> Option<Aabb> {
    match decor_instance(e) {
        Some(decor) => claim(&decor),
        None => Some(e.cube()),
    }
}
