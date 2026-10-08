// pxl-spec/block/ext.rs

//! Which class an id is of: the shape its extension takes, and so the pool
//! it is kept in. By class for now: a marker carries its words.

#[path = "ext.spec.rs"]
mod spec;
pub use spec::{Class, MARKER_TEXT_CHARS};

use crate::block::id::Kind;
use crate::block::object::kind_of;

/// None for an id that carries nothing beyond itself.
pub fn class_of(id: u32) -> Option<Class> {
    match kind_of(id)? {
        Kind::marker => Some(Class::marker_text),
        Kind::cube | Kind::decor => None,
    }
}
