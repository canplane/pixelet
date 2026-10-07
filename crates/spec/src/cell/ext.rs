// pxl-spec/cell/ext.rs

//! Which subclass an id is of: the shape its extension takes, and so the pool
//! it is kept in. By class for now: a marker carries its words.

#[path = "ext.spec.rs"]
mod spec;
pub use spec::{Subclass, MARKER_TEXT_CHARS};

use crate::cell::id::Class;
use crate::cell::object::class_of;

/// None for an id that carries nothing beyond itself.
pub fn subclass_of(id: u32) -> Option<Subclass> {
    match class_of(id)? {
        Class::marker => Some(Subclass::marker_text),
        Class::cube | Class::decor => None,
    }
}
