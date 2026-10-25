// pxl-spec/asset/id.rs

//! An asset's id: its kind over its shape (`spec/spec/asset/id.json`).

#[path = "id.spec.rs"]
mod spec;
pub use spec::{Kind, SHAPE_BITS};

/// An asset's id, `kind << SHAPE_BITS | shape`, as the code carries it: a
/// u32 whose values a u16 holds. A type alias and not a newtype, since the
/// number crosses as a bare u32 wherever it goes — a command's and the
/// snapshot's words, the object store's leaf word, which pxl-spec-core
/// packs for the GPU too (`overwrite::leaf_word`) — and a value past a u16
/// is refused where it is read (`kind_of`, the store's write), not where
/// it is made.
pub type AssetId = u32;

/// The id of `kind`'s `shape`.
pub const fn asset_id(kind: Kind, shape: u32) -> AssetId {
    (kind as u32) << SHAPE_BITS | shape
}

impl Kind {
    /// Whether anything stands on an instance of the kind: a cube; a decor or
    /// a marker is stood on nowhere.
    pub const fn stood_on(self) -> bool {
        matches!(self, Kind::cube)
    }
}

/// The kind an id is of; none past a u16 or for a kind the layout does not
/// give out.
pub fn kind_of(id: AssetId) -> Option<Kind> {
    u16::try_from(id)
        .ok()
        .and_then(|v| Kind::of((v >> SHAPE_BITS) as u8))
}
