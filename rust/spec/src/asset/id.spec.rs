// pxl-spec/asset/id.spec.rs
//
// GENERATED from spec/spec/asset/id.json (xpute-gen data.rs)
//
// An asset's id is a u16: its kind over its index within that kind.
// `id >> IDX_BITS` is the kind, the low bits the index. The kind is what an
// asset is as a shape: a cube is the cell itself, decor a model with a size of
// its own, a marker flat to the camera. What stands in a cell of the world's
// grid is stored as its asset's id (pxl-spec object.rs).

// Names as the spec spells them, so one record has one name on every side.
// A table is generated whole, whether or not this crate calls an entry.
#![allow(
    non_upper_case_globals,
    non_snake_case,
    non_camel_case_types,
    dead_code,
    reason = "generated from the spec"
)]

pub const IDX_BITS: u32 = 10;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Kind {
    cube = 0,
    decor = 1,
    marker = 2,
}

impl Kind {
    /// Every member, in its source's own order.
    pub const ALL: [Kind; 3] = [Kind::cube, Kind::decor, Kind::marker];

    /// The member `val` is, or none for a number no member has.
    pub const fn of(val: u8) -> Option<Kind> {
        match val {
            0 => Some(Kind::cube),
            1 => Some(Kind::decor),
            2 => Some(Kind::marker),
            _ => None,
        }
    }

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            Kind::cube => "cube",
            Kind::decor => "decor",
            Kind::marker => "marker",
        }
    }
}
