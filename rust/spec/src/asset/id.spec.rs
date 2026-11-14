// pxl-spec/asset/id.spec.rs
//
// GENERATED from spec/spec/asset/id.json5 (xpute-gen data.rs)
//
// An asset's id is a u16: its kind over its index within that kind.
// `id >> IDX_BITS` is the kind, the low bits the index. The kind is what an
// asset is as a shape: a cube is the cell itself, decor a model with a size of
// its own, a marker flat to the camera. What stands in a cell of the world's
// grid is stored as its asset's id (pxl-spec object.rs).

pub const IDX_BITS: u32 = 10;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Kind {
    Cube = 0,
    Decor = 1,
    Marker = 2,
}

impl Kind {
    /// Every member, in its source's own order.
    pub const ALL: [Kind; 3] = [Kind::Cube, Kind::Decor, Kind::Marker];

    /// The member `val` is, or none for a number no member has.
    pub const fn of(val: u8) -> Option<Kind> {
        match val {
            0 => Some(Kind::Cube),
            1 => Some(Kind::Decor),
            2 => Some(Kind::Marker),
            _ => None,
        }
    }

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            Kind::Cube => "cube",
            Kind::Decor => "decor",
            Kind::Marker => "marker",
        }
    }
}
