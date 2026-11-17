// pxl-core/domain/control/edit/write.rs

//! What an edit writes, and what it reads to decide: placing, removing and
//! carving objects. It reads through the world's query and writes to the
//! store.

use crate::domain::control::edit::collision::is_placement_blocked;
use crate::domain::control::edit::{hover, tool};
use crate::domain::layer::object::asset::Asset;
use crate::domain::layer::object::ext::Ext;
use crate::domain::preview::{Draft, RenderEdit};
use crate::spec::step::InvalidationReason;
use crate::sys::sched::invalidate;
use pxl_engine::answer::{self, PLACED_ASSET};
use pxl_global::Global;
use pxl_spec::asset::AssetId;
use pxl_spec::asset::cube::solid;
use pxl_spec::asset::marker::{MARKER, MARKER_TEXT_CHARS, text_fits};
use pxl_spec::space::coord::q3::{self, Q3};
use pxl_spec::space::coord::world;
use pxl_spec::space::coord::world::{base_of, cube};
use pxl_spec::space::coord::zoom::{UNIT_ZOOM, Zoom, units};
use pxl_spec::space::hierarchy::morton::key_of;
use pxl_spec::space::layer::lattice::GROUND;
use xpute_kit::status::errno::Errno;

use crate::domain::layer::instance::Instance;
use crate::domain::layer::object::repository;
use crate::domain::layer::object::store::Write;
use crate::domain::query::World;
use pxl_spec::asset::cube::Sentinel;
use pxl_spec::asset::decor::Decor;
use pxl_spec::space::hierarchy::morton;
use pxl_spec::space::layer::octant::Octant;

/// Whether air is written at `at`: the ground was carved there, or
/// something placed there was taken back.
fn is_carved(world: &World, at: &Q3) -> bool {
    world.asset_of(at).val() == Some(PLACED_ASSET | Sentinel::Air as u32)
}

/// A write of void collapses its path in the octree, so a redundant record
/// never stays in a save.
fn write_object(at: Octant, asset_id: AssetId) {
    submit(Write::at(at, asset_id));
}

/// Makes a write at once and, when it changed what is placed, sends it to
/// be kept (`repository::edit`). A write where the world takes no edit
/// (`World::editable`) is refused.
fn submit(write: Write) {
    let (zoom, at) = morton::node_of::<3>(write.at);
    if !World::now().editable(&cube(zoom, at)) {
        return;
    }
    repository::edit(write);
}

/// Whether placing `cube` over a whole cell only undoes a carve. That holds
/// when the cell holds an air leaf and the ground there without the leaf is
/// that cube, the stratum's cube at that depth.
fn place_is_redundant(world: World, at: &Q3, zoom: Zoom, cube: AssetId) -> bool {
    if zoom > GROUND.leaf_cell_zoom() || !is_carved(&world, at) {
        return false;
    }
    let entry = Octant::holding(zoom, at);
    // Only the resident finest leaf can tell what the ground is. Where no
    // such leaf is resident, the cube is written as asked.
    World::base().asset_of(&middle(entry)).exact() == Some(cube)
}

pub(super) fn middle(at: Octant) -> Q3 {
    let (base, half) = (
        base_of(at.zoom(), at.at()),
        q3::from_real(units(at.zoom()) / 2.0),
    );
    world::at(
        i64::from(base[0]) + half,
        i64::from(base[1]) + half,
        i64::from(base[2]) + half,
    )
}

impl Asset {
    pub(crate) fn place(self, at: &Q3, zoom: Zoom) {
        match self {
            Asset::Cube(id) => place_cube(at, zoom, id),
            Asset::Decor(id) => place_decor(at, zoom, id),
            // A draft opens there, and the UI writes its text.
            Asset::Marker => draft_new(at),
            Asset::Unknown => {}
        }
    }
}

/// Placing a cube that only undoes a carve writes void, as `remove` does.
fn place_cube(at: &Q3, zoom: Zoom, cube: AssetId) {
    let asset_id = if place_is_redundant(World::now(), at, zoom, cube) {
        Sentinel::Void as u32
    } else {
        cube
    };
    write_object(Octant::holding(zoom, at), asset_id);
}

/// Where an erase emptied a cell of the decor the scatter stands there,
/// placing that decor back writes void, so the scattered instance returns as
/// it was.
fn place_decor(at: &Q3, zoom: Zoom, id: AssetId) {
    let asset_id = if restores_scattered(at, id).is_some() {
        Sentinel::Void as u32
    } else {
        id
    };
    write_object(Octant::holding(zoom, at), asset_id);
}

/// The decor the ground generates at `at` that placing `id` would bring back
/// (`place_decor`). The item tool's preview shows it.
pub(crate) fn restores_scattered(at: &Q3, id: AssetId) -> Option<Instance> {
    is_carved(&World::now(), at).then(|| World::now().generated_instance(Decor::of(id), at))?
}

/// The marker draft the tool holds open. The frame hands it to the scene's
/// markers, and each change to it here asks for the frame that draws it.
static DRAFT: Global<Option<Draft>> = Global::new();

fn draft_mut() -> &'static mut Option<Draft> {
    DRAFT.get(|| None)
}

pub(crate) fn draft() -> Option<&'static Draft> {
    draft_mut().as_ref()
}

/// Opens a draft at `at` for a marker tool's tap, on a free place or on a
/// marker. The draft keeps the text of a draft already open there, or else
/// starts with the text of the marker that stands there.
pub(crate) fn draft_at(at: Octant) {
    let world = World::now();
    let text = match draft().filter(|d| d.at == at) {
        Some(d) => d.text.clone(),
        None => world
            .entry_at(at)
            .and_then(|e| world.text(&e))
            .unwrap_or_default()
            .to_string(),
    };
    *draft_mut() = Some(Draft { at, text });
    invalidate(InvalidationReason::Scene);
}

/// Opens a new marker's draft for a marker tool's tap on the free cell whose
/// min corner is `corner`.
pub(crate) fn draft_new(corner: &Q3) {
    draft_at(Octant::holding(UNIT_ZOOM, corner));
}

pub(crate) fn cancel_draft() {
    if draft_mut().take().is_some() {
        invalidate(InvalidationReason::Scene);
    }
}

pub(crate) fn marker_draft() -> Option<&'static str> {
    draft().map(|d| d.text.as_str())
}

/// Sets the open draft's text to `text`, the whole text as it is now. The
/// text is copied into the draft's own buffer, so a keystroke allocates only
/// when the text grows past the longest it has held. A text past
/// `MARKER_TEXT_CHARS` is refused whole (pxl-spec asset/marker.rs
/// `text_fits`), EMSGSIZE, and the earlier text stays.
pub(crate) fn set_draft_text(text: &str) -> Result<(), Errno> {
    if !text_fits(text, MARKER_TEXT_CHARS) {
        return Err(Errno::EMSGSIZE);
    }
    if let Some(d) = draft_mut() {
        d.text.clear();
        d.text.push_str(text);
        invalidate(InvalidationReason::Scene);
    }
    Ok(())
}

/// Commits the open draft with its text, if its place is still free or
/// still holds a marker. A draft without text is not committed.
pub(crate) fn commit_draft() {
    let Some(Draft { at, text }) = draft_mut().take() else {
        return;
    };
    invalidate(InvalidationReason::Scene);
    let (zoom, cell) = (at.zoom(), at.at());
    let stands = match World::now().entry_at(at) {
        Some(e) => e.asset() == Asset::Marker,
        None => !is_placement_blocked(Asset::Marker, zoom, &base_of(zoom, cell)),
    };
    if stands && !text.is_empty() {
        submit(Write {
            at: key_of::<3>(zoom, cell),
            asset_id: MARKER,
            ext: Some(Ext::MarkerText(text)),
        });
    }
}

/// Writes an air leaf, which means "nothing here, on purpose". It hides what
/// the base holds there, which has no leaf of its own to delete.
pub(crate) fn place_air(at: Octant) {
    write_object(at, Sentinel::Air as u32);
}

/// What erasing the entry at `at` writes: void where the base holds nothing
/// there that blocks a body, and air where it would fill the cell, which
/// would hide a decor the ground grows there. Only the resident finest leaf
/// can tell, as in `place_is_redundant`; where no such leaf is resident,
/// the cube is emptied with air.
fn erase_asset_id(at: Octant) -> AssetId {
    match World::base().asset_of(&middle(at)).exact() {
        Some(v) if !solid(answer::val(v)) => Sentinel::Void as AssetId,
        _ => Sentinel::Air as AssetId,
    }
}

/// Erases what is placed at `at` and leaves the cell empty. Where the ground
/// would fill the cell once the asset is gone, air is written, so an asset
/// taken out of a hole leaves the hole. A generated decor that shared the
/// asset's box comes back, because its erase is undone too.
pub(crate) fn remove(at: Octant) {
    let world = World::now();
    let erased: Vec<[i32; 3]> = world
        .generated_over(&cube(at.zoom(), at.at()))
        .into_iter()
        .filter_map(|i| i.generated_in())
        .filter(|&c| is_carved(&World::now(), &base_of(GROUND.leaf_cell_zoom(), c)))
        .collect();
    submit(Write {
        at: key_of::<3>(at.zoom(), at.at()),
        asset_id: erase_asset_id(at),
        ext: None,
    });
    for c in erased {
        write_object(
            Octant::new(GROUND.leaf_cell_zoom(), c),
            Sentinel::Void as u32,
        );
    }
}

static RENDER_EDIT: Global<RenderEdit> = Global::new();

/// What the tool held leaves out of the world as drawn, as this frame decided.
pub(crate) fn render_edit() -> RenderEdit {
    *RENDER_EDIT.get(RenderEdit::default)
}

/// Decides what the tool held leaves out of the drawing this frame and returns
/// whether it changed.
pub(crate) fn render_edit_step() -> bool {
    let now = tool().render_edit(&hover());
    core::mem::replace(RENDER_EDIT.get(RenderEdit::default), now) != now
}

#[cfg(test)]
#[path = "write.test.rs"]
mod test;
