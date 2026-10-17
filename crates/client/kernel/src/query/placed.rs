// pxl-kernel/query/placed.rs

//! What is placed, as the records an edit would change: the world's answer
//! for what a player wrote, beside what the blocks compose to.

use pxl_spec::block::object::object_value_is_air;
use pxl_spec::space::coord::aabb::Aabb;

use super::world::World;
use crate::layer::object::store::{Entry, EntryAt};

impl<'a> World<'a> {
    /// What is placed at exactly `at`, air left out.
    pub fn placed(&self, at: EntryAt) -> Option<Entry> {
        self.objects.read((at.zoom(), at.at())).placed().filter(|e| !object_value_is_air(e.state))
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
