// pxl-kernel/control/interaction/edit.rs

//! What an edit writes and what it reads to decide: placing, removing and
//! carving objects, read through the objects' query and written to the store.

use pxl_spec::cell::object::BILLBOARD;
use crate::control::interaction::collision::{is_placement_blocked, PlacementTool};
use crate::control::interaction::{holding, hover, ActiveTool};
use crate::layer::object::preview::RenderEdits;
use crate::layer::object::size::{object_size_of, DECOR_SIZE};
use crate::layer::object::store::EntryKind;
use pxl_spec::bake::constant::{OBJECT_COARSEST, OBJECT_FINEST};
use pxl_spec::cell::object::{decor_num_id, object_value_is_air, placeable_cube_num_id, PlaceableCube};
use pxl_spec::cell::{cube_types, DecorId};
use pxl_spec::space::coord::pvf3::pvf3::Pvf3;
use pxl_spec::space::coord::zoom::{index_of, of_side, units};
use pxl_spec::space::hierarchy::morton::key_of;
use pxl_spec::space::hierarchy::tree::Node;
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec::space::layer::tile::LEAF_ZOOM;
use xpute_core::status::bug::OrBug;
use xpute_core::status::errno::Errno;
use xpute_guest::global::Global;

use crate::kernel;
use crate::layer::instance::Instance;
use crate::layer::object::repository;
use crate::layer::object::store::{apply, Cells, Entry, EntryAt, Write, VOID};
use crate::layer::Query;
use crate::query::World;
use pxl_spec::cell::decor::DecorKind;

/// The objects' cells at a node read whole; all resident, so always answered.
fn cells(node: Node<3>) -> Cells<'static> {
    kernel().objects.read(node, node.0).value().or_bug(Errno::ENOTRECOVERABLE)
}

fn read_state(coord: &Pvf3) -> u32 {
    cells((OBJECT_FINEST, index_of(OBJECT_FINEST, coord))).node().0.state
}

/// Whether any leaf, air included, is written in the cell `at` lies in.
pub fn holds_entry(at: &Pvf3) -> bool {
    kernel().objects.bounds((OBJECT_COARSEST, index_of(OBJECT_COARSEST, at)), OBJECT_COARSEST).value().flatten().is_some()
}

/// What stands in the cell `at` lies in, air left out.
pub fn objects_at(at: &Pvf3) -> Vec<Entry> {
    let mut out = Vec::new();
    cells((OBJECT_COARSEST, index_of(OBJECT_COARSEST, at))).entries(&mut |entry| {
        if !object_value_is_air(entry.state) {
            out.push(*entry);
        }
    });
    out
}

pub fn get_placed_object(at: EntryAt) -> Option<Entry> {
    let state = u32::from(cells((at.zoom(), at.at())).placed()?);
    (!object_value_is_air(state)).then_some(Entry { zoom: at.zoom(), at: at.at(), state })
}

/// The never-written AIR entry the eraser's preview injects into the drawn
/// build where it hovers bare terrain: the cube of `size` holding `at`.
pub fn carve_entry(at: &Pvf3, size: f64) -> EntryAt {
    let zoom = of_side(size);
    EntryAt::new(zoom, index_of(zoom, at))
}

// ---- Mutations ----

/// A write of VOID collapses its path back, so a redundant record never sits in a save.
fn write_object_value(coord: &Pvf3, size: f64, val: u32) {
    submit(Write::at(coord, size, val));
}

/// A write is made here at once and sent to be kept.
fn submit(write: Write) {
    apply([write]);
    repository::send(&write);
}

/// Whether a whole-cell pick of a look only undoes a carve: the terrain must
/// show that look there, and the cell must already hold an AIR leaf (the look
/// is per column, so without this every cube stacked on matching ground was
/// dropped).
fn place_value_is_redundant(at: &Pvf3, size: f64, cube: PlaceableCube) -> bool {
    if size < units(GROUND.cell()) {
        return false;
    }
    let PlaceableCube::Ground(look) = cube else { return false };
    // Only the resident finest leaf can say; elsewhere the cube is written as asked.
    let Some(ground) = World::now().at(at, LEAF_ZOOM).exact() else { return false };
    if ground.cell != look as u8 {
        return false;
    }
    object_value_is_air(read_state(at))
}

/// A redundant re-paint of a carve writes VOID, as remove() does.
pub fn place(at: &Pvf3, size: f64, cube: PlaceableCube) {
    let val = if place_value_is_redundant(at, size, cube) { VOID } else { placeable_cube_num_id(cube) };
    write_object_value(at, size, val);
}

/// Placing the decor a scatter stands in a cell an erase emptied writes
/// VOID, so the scattered instance returns as it was.
pub fn place_decor(at: &Pvf3, size: f64, decor_id: DecorId) {
    let value = if restores_scattered(at, size, decor_id).is_some() { VOID } else { decor_num_id(decor_id) };
    write_object_value(at, size, value);
}

/// The decor the ground grows that `place_decor` would bring back, for its preview.
pub fn restores_scattered(at: &Pvf3, size: f64, decor_id: DecorId) -> Option<Instance> {
    (size >= units(GROUND.cell()) && object_value_is_air(read_state(at))).then(|| World::now().grown(DecorKind::of(decor_id), at))?
}

pub fn place_billboard(at: &Pvf3, size: f64) {
    write_object_value(at, size, BILLBOARD);
}

/// An AIR leaf: "nothing here, on purpose". Unlike remove(), it also hides
/// baked terrain, which has no leaf of its own to delete.
pub fn place_air(at: &Pvf3, size: f64) {
    write_object_value(at, size, cube_types::object::air as u32);
}

pub fn remove(at: EntryAt) {
    submit(Write {
        key: key_of::<3>(at.zoom(), at.at()),
        id: VOID,
    });
}

/// The edit the pointer would make, decided once a turn: the cursor's ghost
/// shows it, and what it removes is handed to the chunks to leave out.
#[derive(Clone, Copy, Default)]
pub struct Pending {
    pub object: Option<Entry>,
    pub size: f64,
    pub digging: bool,
    /// The same check as the commit, so preview and placement cannot disagree.
    pub blocked: bool,
}

static PENDING: Global<Pending> = Global::new();

pub fn pending() -> Pending {
    *PENDING.get(Pending::default)
}

/// Returns what the edit leaves out of what is drawn.
pub fn pending_step() -> RenderEdits {
    let hover = hover();
    let ui = holding();
    let hovered = hover.coord;
    let active_tool = ui.active_tool;
    let object = hover.object_at.and_then(get_placed_object);
    let scattered = hover.scattered.is_some();
    let digging = active_tool == ActiveTool::Eraser && object.is_none() && !scattered;
    // A terrain carve is a whole cell: the scan emits nothing for a zero
    // material whatever its sub-cell bytes, so the cell goes whole anyway.
    let size = match (&object, active_tool) {
        (Some(o), ActiveTool::Eraser | ActiveTool::Pointer) => o.size(),
        (_, ActiveTool::Decor | ActiveTool::Billboard) => DECOR_SIZE,
        _ if digging || scattered => units(GROUND.cell()),
        _ => object_size_of(ui.object_tier),
    };
    let blocked = match (&hovered, active_tool) {
        (Some(h), ActiveTool::Object | ActiveTool::Decor | ActiveTool::Billboard) => {
            let tool = if active_tool == ActiveTool::Object { PlacementTool::Object } else { PlacementTool::Instance };
            is_placement_blocked(tool, h, object_size_of(ui.object_tier))
        }
        _ => false,
    };
    // The real object's hide lands a tick or more after the ghost, accepted.
    let hidden = object.filter(|_| active_tool == ActiveTool::Eraser);
    let kind = hidden.map(|o| o.kind());
    let edits = RenderEdits {
        hide: hidden.map(|o| o.id()),
        hide_in: (!matches!(kind, Some(EntryKind::Decor { .. })), !matches!(kind, Some(EntryKind::Solid { .. }))),
        // What is one voxel below shows through the ghost.
        carve: hovered.filter(|_| digging).map(|h| carve_entry(&h, size)),
        scattered: hover.scattered.filter(|_| active_tool == ActiveTool::Eraser).and_then(|s| s.grown_in()),
    };
    *PENDING.get(Pending::default) = Pending { object, size, digging, blocked };
    edits
}
