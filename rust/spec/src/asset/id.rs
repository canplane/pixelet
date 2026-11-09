// pxl-spec/asset/id.rs

//! An asset's id: its kind over its index (`spec/spec/asset/id.json`).

#[path = "id.spec.rs"]
mod spec;
pub use spec::{Kind, IDX_BITS};

use crate::value;

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
        matches!(self, Kind::cube | Kind::decor)
    }

    /// Whether an instance of the kind stands on what is under it: a decor
    /// or a marker; a cube goes against any face, whatever is under it.
    pub const fn stands(self) -> bool {
        matches!(self, Kind::decor | Kind::marker)
    }
}

/// Whether an asset stops a body: a cube does, and so does an id in a cube's
/// range that the catalog does not give out; void and air, a decor and a
/// marker do not.
pub const fn solid(id: AssetId) -> bool {
    id != value::VOID && id != value::AIR && id >> IDX_BITS == Kind::cube as u32
}

/// The kind an id is of; none past a u16 or for a kind the layout does not
/// give out.
pub fn kind_of(id: AssetId) -> Option<Kind> {
    u16::try_from(id)
        .ok()
        .and_then(|v| Kind::of((v >> IDX_BITS) as u8))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A body is stopped by what is a cube, in the catalog or not, and by
    /// nothing else.
    #[test]
    fn what_blocks_a_body() {
        for id in [value::VOID, value::AIR, 1 << IDX_BITS, 2 << IDX_BITS] {
            assert!(!solid(id), "{id}");
        }
        for id in [1, value::UNKNOWN, value::AIR + 1, (1 << IDX_BITS) - 1] {
            assert!(solid(id), "{id}");
        }
    }
}
