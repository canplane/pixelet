// pxl-spec/object/ext.spec.rs
//
// GENERATED from spec/spec/object/ext.json
//
// What a placed thing may carry beyond its id: its extension. Things whose
// extensions have one shape are a class, and each class keeps its
// extensions in a pool of its own; a leaf names its record by its place there,
// or none. Which class an id is of is the catalog's (pxl-spec object/ext.rs).

// Names as the spec spells them, so one record has one name on every side.
// A table is generated whole, whether or not this crate calls an entry.
#![allow(
    non_upper_case_globals,
    non_snake_case,
    clippy::absolute_paths,
    non_camel_case_types,
    dead_code,
    reason = "generated from the spec"
)]

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Class {
    marker_text = 0,
}

impl Class {
    /// Every member, in the JSON's own order.
    pub const ALL: [Class; 1] = [Class::marker_text];

    /// The member a number names, or none for a number the set does not
    /// give out.
    pub fn of(val: u8) -> Option<Class> {
        Class::ALL.into_iter().find(|member| *member as u8 == val)
    }
}

pub mod CLASS {
    pub const marker_text: u8 = 0;
}
