// pxl-spec/object/ext.spec.rs
//
// GENERATED from spec/spec/object/ext.json (xpute-gen data.rs)
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
    /// Every member, in its source's own order.
    pub const ALL: [Class; 1] = [Class::marker_text];

    /// The member `val` is, or none for a number no member has.
    pub const fn of(val: u8) -> Option<Class> {
        match val {
            0 => Some(Class::marker_text),
            _ => None,
        }
    }

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            Class::marker_text => "marker_text",
        }
    }
}
