// pxl-core/domain/preview.rs

//! An edit not yet made, in the form both heads share. Control decides the
//! preview, and the frame hands it to the scene, which draws it (frame.rs). A
//! preview has three parts: what it changes in the world as drawn; the ghost
//! it shows under the pointer; and a marker's draft, drawn with the text
//! typed so far.

use pxl_spec::asset::AssetId;
use pxl_spec::asset::decor::Decor;
use pxl_spec::space::coord::v3::V3;
use pxl_spec::space::coord::zoom::Zoom;
use pxl_spec::space::layer::octant::Octant;

/// What the eraser's preview changes in the world as drawn, and nowhere
/// else. Every mechanic (the pick, a walk, the camera's clearance, an edit's
/// own reads, decor's column surface) asks the stored world (`World::now`).
/// If the pick saw the preview, a hover would hide what it points at, the
/// hide would lift, and the pick would hit it again, forever. A body would
/// fall into a hole not yet dug. The preview moves no revision, so saves
/// never see a hover.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct RenderEdit {
    /// The key of the placed object hidden, whatever its kind. The rays cut
    /// it and decor's records leave it out, each reader only where it holds
    /// that object.
    pub hide: Option<Octant>,
    /// Where the eraser over bare terrain draws air, because bare terrain
    /// has no leaf to hide.
    pub carve: Option<Octant>,
    /// The cell whose scattered decor is hidden.
    pub scattered_cell: Option<[i32; 3]>,
}

/// What the pointer's tool would show where the pointer is. The tool decides
/// it (domain/control/edit/ghost.rs `ghost`), and the scene draws it half
/// transparent, the way its kind is drawn (domain/scene/indicator/ghost.rs
/// `GhostDraw::set`).
#[derive(Clone, Copy)]
pub(crate) enum Ghost {
    None,
    /// A decor instance whose base stands at `at` in the world's frame.
    /// `stand_in` is the box a placement shows in its place, and is None for
    /// an instance the world already holds.
    Decor {
        at: V3,
        decor: &'static Decor,
        variant: u32,
        rot: f64,
        stand_in: Option<GhostBox>,
    },
    Box(GhostBox),
}

/// A box the tool would fill or carve. `at` is its corner in the world's
/// frame and `zoom` the zoom of its cube. `cube` is the cube it would place
/// or carve, and is None where the box is not one cube. `erase` is set when
/// the tool is the eraser.
#[derive(Clone, Copy)]
pub(crate) struct GhostBox {
    pub at: V3,
    pub zoom: Zoom,
    pub cube: Option<AssetId>,
    pub erase: bool,
}

/// A marker being written. Its place may be free or may already hold a
/// marker. Once committed, the draft is the marker at that place.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Draft {
    pub at: Octant,
    pub text: String,
}
