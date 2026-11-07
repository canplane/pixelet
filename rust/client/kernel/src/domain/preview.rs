// pxl-kernel/domain/preview.rs

//! An edit not yet made, as both heads see it: control decides it, and the
//! turn hands it to the scene, which draws it (pxl-kernel turn.rs). What it
//! changes in what is drawn, which the world as drawn composes
//! (`World::previewing`); the ghost it shows under the pointer (`Ghost`); and
//! a marker's draft, drawn with the text it has so far.

use pxl_spec::asset::decor::Decor;
use pxl_spec::asset::AssetId;
use pxl_spec::space::coord::v3::V3;
use pxl_spec::space::coord::zoom::Zoom;
use pxl_spec::space::layer::octant::Octant;

/// What the eraser's preview changes in what is drawn only: every mechanic
/// — the pick, a walk, the camera's clearance, an edit's own reads, decor's
/// column surface — asks the stored world (`World::now`), or hover would clear,
/// the hide lift and the pick hit it again, forever, and a walker would fall
/// into a hole not dug. No revision moves, so saves do not see the hover.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RenderEdits {
    /// The placed object hidden, by its key, whatever its kind: the rays
    /// cut it and decor's records leave it out, each meeting it only where
    /// it holds it.
    pub hide: Option<Octant>,
    /// An AIR entry over bare terrain, which has no leaf to hide.
    pub carve: Option<Octant>,
    /// The scattered decor hidden, by the cell it stands in.
    pub scattered_cell: Option<[i32; 3]>,
}

/// What the pointer's tool would show where it is: nothing, a decor
/// instance, or a box, an instance in the pointer's frame. The tool decides
/// it (pxl-kernel domain/control/edit/ghost.rs `ghost`), and the scene
/// draws it half transparent as its kind is drawn
/// (domain/scene/indicator/ghost.rs `GhostDraw::set`).
#[derive(Clone, Copy)]
pub enum Ghost {
    None,
    /// `decor`'s `variant`, its base at `at` in the world's frame. `stand_in`
    /// is the box a placement would show in its place, none for an instance
    /// the world already holds.
    Decor {
        at: V3,
        decor: &'static Decor,
        variant: u32,
        rot: f64,
        scale: f64,
        stand_in: Option<GhostBox>,
    },
    Box(GhostBox),
}

/// A box the tool would fill or carve: its corner in the world's frame, the
/// zoom of its cube, the cube it would be or carve (none where there is no one cube), and
/// whether the eraser holds it.
#[derive(Clone, Copy)]
pub struct GhostBox {
    pub at: V3,
    pub zoom: Zoom,
    pub cube: Option<AssetId>,
    pub erase: bool,
}

/// A marker's draft: its place, free or holding a marker, and its text so
/// far; committed, it is the marker that stands there.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Draft {
    pub at: Octant,
    pub text: String,
}
