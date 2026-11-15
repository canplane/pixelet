// pxl-spec/asset/id.rs

//! An asset's id: its kind over its index (`spec/spec/asset/id.json5`).

#[path = "id.spec.rs"]
pub mod spec;
pub use spec::{IDX_BITS, Kind};

/// An asset's id, `kind << IDX_BITS | idx`, as the code carries it: a
/// u32 whose values a u16 holds. A value past a u16 is refused where it is
/// read (`kind_of`, the store's write).
pub type AssetId = u32;

pub(crate) const fn asset_id(kind: Kind, idx: u32) -> AssetId {
    (kind as u32) << IDX_BITS | idx
}

/// `id`'s index within its kind, which is its place in its kind's table.
pub const fn idx_of(id: AssetId) -> u32 {
    id & ((1 << IDX_BITS) - 1)
}

impl Kind {
    /// Whether what is placed stands on an instance of the kind: a cube, or a
    /// decor on its box, which may float over the model; a marker is stood
    /// on nowhere.
    pub const fn stood_on(self) -> bool {
        matches!(self, Kind::Cube | Kind::Decor)
    }

    /// Whether an instance of the kind stands on what is under it: a decor
    /// or a marker; a cube goes against any face, whatever is under it.
    pub const fn stands(self) -> bool {
        matches!(self, Kind::Decor | Kind::Marker)
    }
}

/// The kind an id is of; none past a u16 or for a kind the layout does not
/// give out.
pub fn kind_of(id: AssetId) -> Option<Kind> {
    u16::try_from(id)
        .ok()
        .and_then(|v| Kind::of((v >> IDX_BITS) as u8))
}
