// pxl-kernel/layer/object/preview.rs

//! An edit not yet made, as what it leaves out of what is drawn: the pointer
//! decides it, the chunks draw without it.

use pxl_spec::bake::constant::EntryAt;

/// What the eraser's preview changes in the drawn build only: the pick and
/// the cell queries still see what it hides, or hover would clear, the hide
/// lift and the pick hit it again, forever. No revision moves, so saves do
/// not see the hover.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RenderEdits {
    /// The placed object hidden, and whether it is drawn in tiers, decor, or
    /// (already gone) either.
    pub hide: Option<EntryAt>,
    pub hide_in: (bool, bool),
    /// An AIR entry over bare terrain, which has no leaf to hide.
    pub carve: Option<EntryAt>,
    /// The scattered decor hidden, by the cell it stands in.
    pub scattered: Option<[i64; 3]>,
}
