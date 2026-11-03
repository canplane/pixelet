// pxl-kernel/domain/query/entry.rs

//! What is placed, as the records an edit would change: the world's answer
//! for what a player wrote, beside what the assets compose to.

use pxl_spec::object::is_air;
use pxl_spec::space::coord::aabb::Aabb;
use pxl_spec::space::coord::zoom::Zoom;

use super::decor::claim;
use super::world::World;
use crate::domain::layer::object::meta::decor_instance;
use crate::domain::layer::object::store::Entry;
use crate::domain::layer::object::store::ObjectQuery as _;
use crate::domain::query::decor::intersect::box_extent;
use pxl_spec::space::layer::octant::Octant;

impl<'a> World<'a> {
    /// What is placed at exactly `at`, air left out.
    pub fn entry_at(&self, at: Octant) -> Option<Entry> {
        self.object
            .read((at.zoom(), at.at()))
            .entry()
            .filter(|e| !is_air(e.state))
    }

    /// Everything placed whose cube meets `space`, air left out, X in
    /// `space`'s frame.
    pub fn placed_in(&self, space: &Aabb, visit: &mut dyn FnMut(&Entry)) {
        self.object.entries_in(space, &mut |e| {
            if !is_air(e.state) {
                visit(e);
            }
        });
    }

    /// The finest zoom anything is placed at in `space`, air included: what
    /// a dig cut finer is held as finely as a cube.
    pub(super) fn finest_in(&self, space: &Aabb) -> Option<Zoom> {
        let mut finest = None;
        self.object
            .entries_in(space, &mut |e| finest = finest.max(Some(e.zoom)));
        finest
    }

    /// A marker's text, as put up; none for anything else.
    pub fn text(&self, e: &Entry) -> Option<&'a str> {
        self.object.text(e)
    }

    /// What is placed whose own room shares `space`, air left out, that
    /// `which` takes: the room its kind says it occupies (`occupies`).
    pub fn placed_occupying(&self, space: &Aabb, which: impl Fn(&Entry) -> bool) -> Option<Entry> {
        // A decor's box reaches past its cell's corner.
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

/// The room a placed entry occupies, as its kind has it: a decor its box
/// (`decor::claim`), which holds its cell, the corner it stands at, since
/// a variant is a cell or more a side; anything else its cell.
fn occupies(e: &Entry) -> Option<Aabb> {
    match decor_instance(e) {
        Some(decor) => claim(&decor),
        None => Some(e.cube()),
    }
}
