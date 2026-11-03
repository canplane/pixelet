// pxl-kernel/domain/layer/object/preview.rs

//! An edit not yet made, as what it changes in what is drawn: the pointer
//! decides it, the world as drawn composes it (`World::previewing`); the
//! ghost it shows under the pointer (`Ghost`); and a marker's draft, drawn
//! with the text it has so far.

use crate::sys::sched::edge::request_world_tick;
use pxl_spec::asset::decor::Decor;
use pxl_spec::asset::AssetId;
use pxl_spec::space::coord::v3::V3;
use pxl_spec::space::coord::zoom::Zoom;
use pxl_spec::space::layer::octant::Octant;
use xpute_guest::global::Global;

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

/// The pointer's preview, the one copy of it.
static PREVIEW: Global<RenderEdits> = Global::new();

/// Hands the pointer's preview over once a turn: the one it replaced, where
/// it changed.
pub fn set(edits: RenderEdits) -> Option<RenderEdits> {
    let was = core::mem::replace(PREVIEW.get(RenderEdits::default), edits);
    (was != edits).then_some(was)
}

/// The preview in force.
pub fn cur() -> RenderEdits {
    *PREVIEW.get(RenderEdits::default)
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

static DRAFT: Global<Option<Draft>> = Global::new();

/// The draft, which the markers draw.
pub fn draft() -> Option<&'static Draft> {
    DRAFT.get(|| None).as_ref()
}

/// The draft, for the tool to change, which the next world turn draws.
pub fn draft_mut() -> &'static mut Option<Draft> {
    request_world_tick();
    DRAFT.get(|| None)
}
