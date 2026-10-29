// pxl-kernel/domain/layer/object/preview.rs

//! An edit not yet made, as what it changes in what is drawn: the pointer
//! decides it, the world as drawn composes it (`World::previewing`); the
//! ghost it shows under the pointer (`Ghost`); and a marker waiting for its
//! words, drawn with the words it has so far.

use crate::domain::layer::object::store;
use crate::sys::sched::edge::request_world_tick;
use pxl_spec::asset::cube::Cube;
use pxl_spec::asset::decor::Decor;
use pxl_spec::bake::constant::EntryAt;
use pxl_spec::space::coord::v3::V3;
use xpute_guest::global::Global;

/// What the eraser's preview changes in what is drawn only: every mechanic
/// — the pick, a walk, the camera's clearance, an edit's own reads, decor's
/// footing — asks the stored world (`World::now`), or hover would clear,
/// the hide lift and the pick hit it again, forever, and a walker would fall
/// into a hole not dug. No revision moves, so saves do not see the hover.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RenderEdits {
    /// The placed object hidden, by its key, whatever its kind: the rays
    /// cut it and decor's records leave it out, each meeting it only where
    /// it holds it.
    pub hide: Option<EntryAt>,
    /// An AIR entry over bare terrain, which has no leaf to hide.
    pub carve: Option<EntryAt>,
    /// The scattered decor hidden, by the cell it stands in.
    pub scattered: Option<[i32; 3]>,
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
/// it (pxl-kernel domain/control/interaction/ghost.rs `ghost`), and the scene
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

/// A box the tool would fill or carve: its corner in the world's frame, its
/// edge, the cube it would be or carve (none where there is no one cube), and
/// whether the eraser holds it.
#[derive(Clone, Copy)]
pub struct GhostBox {
    pub at: V3,
    pub size: f64,
    pub cube: Option<Cube>,
    pub erase: bool,
}

/// A marker the tool left waiting for its words, at a place free or holding
/// a marker: put up there with them, it is the marker that stands there.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Draft {
    pub at: EntryAt,
    pub words: String,
}

static DRAFT: Global<Option<Draft>> = Global::new();

/// The words a marker at `at` shows: the waiting one's so far, else those
/// put up there; none where no marker stands or waits.
pub fn words_at(at: EntryAt) -> Option<&'static str> {
    if let Some(d) = draft().filter(|d| d.at == at) {
        return Some(&d.words);
    }
    let object = store::object();
    object.words(&object.read((at.zoom(), at.at())).placed()?)
}

/// The one waiting, which the markers draw.
pub fn draft() -> Option<&'static Draft> {
    DRAFT.get(|| None).as_ref()
}

/// The one waiting, for the tool to change, which the next world turn draws.
pub fn redraft() -> &'static mut Option<Draft> {
    request_world_tick();
    DRAFT.get(|| None)
}
