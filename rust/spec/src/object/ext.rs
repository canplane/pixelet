// pxl-spec/object/ext.rs

//! Which class an asset id is of: the shape of the extension a placed asset of it carries,
//! and so the pool the extension is kept in. The class goes by kind: a marker carries its
//! text.

/// What a placed thing may carry beyond its id: its extension. Things whose
/// extensions have one shape are a class, and each class keeps its
/// extensions in a pool of its own; a leaf names its record by its place
/// there, or none.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    MarkerText,
}

impl Class {
    /// Every class, each a pool.
    pub const ALL: [Class; 1] = [Class::MarkerText];
}

use crate::asset::id::{kind_of, AssetId, Kind};

/// None for an id that carries nothing beyond itself.
pub fn class_of(id: AssetId) -> Option<Class> {
    match kind_of(id)? {
        Kind::Marker => Some(Class::MarkerText),
        Kind::Cube | Kind::Decor => None,
    }
}
