// pxl-kernel/domain/query/placed.rs

//! What is placed, as the records an edit would change: the world's answer
//! for what a player wrote, beside what the blocks compose to.

use pxl_spec::block::object::object_value_is_air;
use pxl_spec::space::coord::aabb::Aabb;
use pxl_spec::space::coord::zoom::Zoom;

use super::world::World;
use crate::domain::layer::object::store::Entry;
use pxl_spec::bake::constant::EntryAt;

impl<'a> World<'a> {
    /// What is placed at exactly `at`, air left out.
    pub fn placed(&self, at: EntryAt) -> Option<Entry> {
        self.objects.read((at.zoom(), at.at())).placed().filter(|e| !object_value_is_air(e.state))
    }

    /// Everything placed whose cube meets `space`, air left out, X in
    /// `space`'s frame.
    pub fn placed_in(&self, space: &Aabb, visit: &mut dyn FnMut(&Entry)) {
        self.objects.entries_in(space, &mut |e| {
            if !object_value_is_air(e.state) {
                visit(e);
            }
        });
    }

    /// The finest zoom anything is placed at in `space`, air included: what
    /// a dig cut finer is held as finely as a cube.
    pub(super) fn finest_in(&self, space: &Aabb) -> Option<Zoom> {
        let mut finest = None;
        self.objects.entries_in(space, &mut |e| finest = finest.max(Some(e.zoom)));
        finest
    }

    /// A marker's words, as put up; none for anything else.
    pub fn words(&self, e: &Entry) -> Option<&'a str> {
        self.objects.words(e)
    }

    /// What is placed sharing room with `space`, air left out, that `which` takes.
    pub fn placed_sharing(&self, space: &Aabb, which: impl Fn(&Entry) -> bool) -> Option<Entry> {
        let mut met = None;
        self.objects.entries_in(space, &mut |e| {
            if met.is_none() && !object_value_is_air(e.state) && e.cube().shares(*space) && which(e) {
                met = Some(*e);
            }
        });
        met
    }
}
