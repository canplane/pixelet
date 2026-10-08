// pxl-kernel/layer/instance.rs

//! A thing drawn as one instance of its shape, whichever layer answers it:
//! its id is its kind over its shape (the block catalog's), and `meta` is what
//! its class carries. The ground grows its own; an object's is the placement's.

use pxl_spec::block::decor::DecorMeta;
use pxl_spec::space::coord::v3::V3;

use crate::layer::object::store::EntryAt;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Instance {
    pub id: u16,
    /// Where it stands: its footprint's middle, at the height of its base.
    pub at: V3,
    pub source: Source,
    pub meta: Meta,
}

/// Which layer's record it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// Grown by the ground, in the leaf cell it stands on.
    Ground([i64; 3]),
    /// Placed, at its entry.
    Object(EntryAt),
}

impl Instance {
    /// The leaf cell the ground grew it in; none for a placed one.
    pub fn grown_in(&self) -> Option<[i64; 3]> {
        match self.source {
            Source::Ground(cell) => Some(cell),
            Source::Object(_) => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Meta {
    Decor(DecorMeta),
}
