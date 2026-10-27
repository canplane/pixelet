// pxl-spec/object/ext.rs

//! Which class an asset id is of: the shape the extension a placed asset of it
//! carries takes, and so the pool it is kept in. By kind for now: a marker
//! carries its words.

#[path = "ext.spec.rs"]
mod spec;
pub use spec::Class;

use crate::asset::id::{kind_of, AssetId, Kind};

/// None for an id that carries nothing beyond itself.
pub fn class_of(id: AssetId) -> Option<Class> {
    match kind_of(id)? {
        Kind::marker => Some(Class::marker_text),
        Kind::cube | Kind::decor | Kind::actor | Kind::sky => None,
    }
}
