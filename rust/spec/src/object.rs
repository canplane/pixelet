// pxl-spec/object.rs

//! What the object layer stores in a cell: the asset placed there, by its asset id, which is
//! the kind over the index (`asset/id.json5`). A cube's asset id is its value (value.rs), so a
//! placed ground cube stores its own asset id. The decor kind is the decors
//! (`asset/decor.json5`), and the marker kind the pin. Which class an asset id is of is the
//! shape of the extension a placed asset of it carries (`Class`), and so the pool the
//! extension is kept in. The class goes by kind: a marker carries its text.

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
