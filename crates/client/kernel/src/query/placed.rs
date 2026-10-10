// pxl-kernel/query/placed.rs

//! What is placed, as the records an edit would change: the world's answer
//! for what a player wrote, beside what the blocks compose to.

use pxl_spec::bake::constant::OBJECT_COARSEST;
use pxl_spec::block::object::object_value_is_air;
use pxl_spec::space::coord::pvf3::pvf3::Pvf3;
use pxl_spec::space::coord::zoom::index_of;
use pxl_spec::space::hierarchy::tree::Node;
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

use super::world::World;
use crate::layer::object::store::{Cells, Entry, EntryAt};
use crate::layer::Query;

impl<'a> World<'a> {
    /// What is placed at exactly `at`, air left out.
    pub fn placed(&self, at: EntryAt) -> Option<Entry> {
        self.cells((at.zoom(), at.at())).placed().filter(|e| !object_value_is_air(e.state))
    }

    /// What stands in the coarsest object cell `at` lies in, air left out.
    pub fn placed_in(&self, at: &Pvf3) -> Vec<Entry> {
        let mut out = Vec::new();
        self.cells((OBJECT_COARSEST, index_of(OBJECT_COARSEST, at))).entries(&mut |entry| {
            if !object_value_is_air(entry.state) {
                out.push(*entry);
            }
        });
        out
    }

    /// Whether anything, air included, is written in the coarsest object cell `at` lies in.
    pub fn written_in(&self, at: &Pvf3) -> bool {
        self.cells((OBJECT_COARSEST, index_of(OBJECT_COARSEST, at))).holds()
    }

    /// The objects' cells at a node read whole; all resident, so always answered.
    fn cells(&self, node: Node<3>) -> Cells<'a> {
        self.objects.read(node, node.0).value().or_bug(Errno::ENOTRECOVERABLE)
    }
}
