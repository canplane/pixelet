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
    /// Whether what is placed stands on an instance of the kind: a cube, or a
    /// decor on its box, which may float over the model; a marker is stood
    /// on nowhere. A walker stands on what fills a cell, which a decor does
    /// not, and asks nothing of this.
    pub const fn stood_on(self) -> bool {
        matches!(self, Kind::cube | Kind::decor)
    }

    /// Whether an instance of the kind stands on what is under it: a decor
    /// or a marker; a cube goes against any face, whatever is under it.
    pub const fn stands(self) -> bool {
        matches!(self, Kind::decor | Kind::marker)
    }
}

/// The kind an id is of; none past a u16 or for a kind the layout does not
/// give out.
pub fn kind_of(id: AssetId) -> Option<Kind> {
    u16::try_from(id)
        .ok()
        .and_then(|v| Kind::of((v >> SHAPE_BITS) as u8))
}
