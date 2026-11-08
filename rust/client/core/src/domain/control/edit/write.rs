// pxl-core/domain/control/edit/write.rs

//! What an edit writes, and what it reads to decide: placing, removing and
//! carving objects. It reads through the world's query and writes to the
//! store.

use crate::domain::control::edit::collision::{is_placement_blocked, placement_reads};
use crate::domain::control::edit::{hover, tool, Tool};
use crate::domain::layer::object::asset::Asset;
use crate::domain::layer::object::ext::Ext;
use crate::domain::layer::object::size::MARKER_ZOOM;
use crate::domain::preview::{Draft, RenderEdits};
use crate::sys::sched::edge::request_world_tick;
use pxl_engine::answer::PLACED_ASSET;
use pxl_spec::asset::marker::MARKER;
use pxl_spec::asset::marker::MARKER_TEXT_CHARS;
use pxl_spec::asset::{cube, AssetId};
use pxl_spec::space::coord::q3::{self, Q3};
use pxl_spec::space::coord::world;
use pxl_spec::space::coord::zoom::{base_of, cube, units, Zoom};
use pxl_spec::space::hierarchy::morton::key_of;
use pxl_spec::space::layer::lattice::GROUND;
use xpute_core::global::Global;
use xpute_kit::status::errno::Errno;

use crate::domain::layer::instance::Instance;
use crate::domain::layer::object::repository;
use crate::domain::layer::object::store::{Entry, Write, AIR, VOID};
use crate::domain::query::changes;
use crate::domain::query::World;
use pxl_engine::answer;
use pxl_spec::asset::decor::Decor;
use pxl_spec::space::hierarchy::morton;
use pxl_spec::space::layer::octant::Octant;

/// Whether AIR is written at `at`: the ground was carved there, or
/// something placed there was taken back.
fn is_carved(world: &World, at: &Q3) -> bool {
    world.asset_of(at).val() == Some(PLACED_ASSET | AIR)
}

// ---- Mutations ----

/// A write of VOID collapses its path in the octree, so a redundant record
/// never stays in a save.
fn write_object_val(at: Octant, val: u32) {
    submit(Write::at(at, val));
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
/// when the cell holds an AIR leaf and the ground there without the leaf is
/// that cube, the stratum's cube at that depth.
fn place_val_is_redundant(world: World, at: &Q3, zoom: Zoom, cube: AssetId) -> bool {
    if zoom > GROUND.leaf_cell_zoom() || !is_carved(&world, at) {
        return false;
    }
    let entry = Octant::holding(zoom, at);
    // Only the resident finest leaf can tell what the ground is. Where no
    // such leaf is resident, the cube is written as asked.
    let ground = world
        .previewing(Some(entry), None)
        .asset_of(&middle(entry))
        .exact();
    ground == Some(cube)
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
    /// What a tap does at `at` with this asset held, its cube at `zoom`.
    pub fn place(self, at: &Q3, zoom: Zoom) {
        match self {
            Asset::Cube(id) => place_cube(at, zoom, id),
            Asset::Decor(id) => place_decor(at, zoom, id),
            // A draft opens there, and the UI writes its text.
            Asset::Marker => draft_new(at),
            Asset::Unknown => {}
        }
    }
}

/// Placing a cube that only undoes a carve writes VOID, as `remove` does.
fn place_cube(at: &Q3, zoom: Zoom, cube: AssetId) {
    let val = if place_val_is_redundant(World::now(), at, zoom, cube) {
        VOID
    } else {
        cube
    };
    write_object_val(Octant::holding(zoom, at), val);
}

/// Where an erase emptied a cell of the decor the scatter stands there,
/// placing that decor back writes VOID, so the scattered instance returns as
/// it was.
fn place_decor(at: &Q3, zoom: Zoom, id: AssetId) {
    let val = if restores_scattered(at, id).is_some() {
        VOID
    } else {
        id
    };
    write_object_val(Octant::holding(zoom, at), val);
}

/// The decor the ground generates at `at` that placing `id` would bring
/// back (`place_decor`). The ghost previews it.
pub fn restores_scattered(at: &Q3, id: AssetId) -> Option<Instance> {
    is_carved(&World::now(), at).then(|| World::now().generated_instance(Decor::of(id), at))?
}

/// The marker draft the tool holds open. The turn hands it to the scene's
/// markers, and each change to it here asks for the turn that draws it.
static DRAFT: Global<Option<Draft>> = Global::new();

fn draft_mut() -> &'static mut Option<Draft> {
    DRAFT.get(|| None)
}

pub fn draft() -> Option<&'static Draft> {
    draft_mut().as_ref()
}

/// Opens a draft at `at` for a marker tool's tap, on a free place or on a
/// marker. The draft keeps the text of a draft already open there, or else
/// starts with the text of the marker that stands there.
pub fn draft_at(at: Octant) {
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
    request_world_tick();
}

/// Opens a new marker's draft for a marker tool's tap on the free cell whose
/// min corner is `corner`.
pub fn draft_new(corner: &Q3) {
    draft_at(Octant::holding(MARKER_ZOOM, corner));
}

pub fn cancel_draft() {
    if draft_mut().take().is_some() {
        request_world_tick();
    }
}

/// The open draft's text, for the UI.
pub fn marker_draft() -> Option<&'static str> {
    draft().map(|d| d.text.as_str())
}

/// Sets the open draft's text to `text`, the whole text as it is now. The
/// text is copied into the draft's own buffer, so a keystroke allocates only
/// when the text grows past the longest it has held. A text longer than
/// `MARKER_TEXT_CHARS` is refused whole, and the earlier text stays.
pub fn set_draft_text(text: &str) -> Result<(), Errno> {
    if text.chars().count() > MARKER_TEXT_CHARS as usize {
        return Err(Errno::EMSGSIZE);
    }
    if let Some(d) = draft_mut() {
        d.text.clear();
        d.text.push_str(text);
        request_world_tick();
    }
    Ok(())
}

/// Commits the open draft with its text, if its place is still free or
/// still holds a marker. A draft without text is not committed.
pub fn commit_draft() {
    let Some(Draft { at, text }) = draft_mut().take() else {
        return;
    };
    request_world_tick();
    let (zoom, cell) = (at.zoom(), at.at());
    let stands = match World::now().entry_at(at) {
        Some(e) => e.asset() == Asset::Marker,
        None => !is_placement_blocked(Asset::Marker, zoom, &base_of(zoom, cell)),
    };
    if stands && !text.is_empty() {
        submit(Write {
            at: key_of::<3>(zoom, cell),
            id: MARKER,
            ext: Some(Ext::MarkerText(text)),
        });
    }
}

/// Writes an AIR leaf, which means "nothing here, on purpose". Unlike
/// `remove`, it also hides baked terrain, which has no leaf of its own to
/// delete.
pub fn place_air(at: Octant) {
    write_object_val(at, cube::object::air as u32);
}

/// Erases what is placed at `at` and leaves the cell empty. Where the ground
/// would fill the cell once the asset is gone, AIR is written, so an asset
/// taken out of a hole leaves the hole. A generated decor that shared the
/// asset's box comes back, because its erase is undone too.
pub fn remove(at: Octant) {
    let world = World::now();
    let erased: Vec<[i32; 3]> = world
        .generated_over(&cube(at.zoom(), at.at()))
        .into_iter()
        .filter_map(|i| i.generated_in())
        .filter(|&c| is_carved(&World::now(), &base_of(GROUND.leaf_cell_zoom(), c)))
        .collect();
    // Only the resident finest leaf can tell what fills the cube without the
    // asset, as in `place_val_is_redundant`. Where no such leaf is resident,
    // the cube is emptied with AIR.
    let id = match world
        .previewing(Some(at), None)
        .asset_of(&middle(at))
        .exact()
    {
        Some(v) if v != VOID && !answer::is_placed(v) => AIR,
        None => AIR,
        Some(_) => VOID,
    };
    submit(Write {
        at: key_of::<3>(at.zoom(), at.at()),
        id,
        ext: None,
    });
    for c in erased {
        write_object_val(Octant::new(GROUND.leaf_cell_zoom(), c), VOID);
    }
}

/// The edit the pointer would make, decided once a turn. The ghost shows it,
/// and the turn hands what it removes to the drawing to leave out (`edits`).
#[derive(Clone, Copy, Default)]
pub struct Preview {
    pub object: Option<Entry>,
    /// The zoom of the cube the edit fills or takes.
    pub zoom: Zoom,
    pub digging: bool,
    /// The same check the commit makes (`is_placement_blocked`), so the
    /// preview and the placement cannot disagree.
    pub blocked: bool,
    /// What `blocked` was computed from: the asset, its zoom, the hovered
    /// corner and the store's revision. `blocked` is computed again only when
    /// one of these moved or a change touched the columns the check reads
    /// (`placement_reads`, `changes::within`). The step runs on every frame
    /// turn, so it sees each change once.
    blocked_key: Option<(Asset, Zoom, Q3, u32)>,
    pub edits: RenderEdits,
}

static PREVIEW: Global<Preview> = Global::new();

pub fn preview() -> Preview {
    *PREVIEW.get(Preview::default)
}

/// Decides the preview for this turn and returns whether what the edit
/// leaves out of the drawing changed.
pub fn preview_step() -> bool {
    let hover = hover();
    let ui = tool();
    let hovered = hover.coord;
    let world = World::now();
    let object = hover.object_at().and_then(|at| world.entry_at(at));
    let scattered = hover.scattered_decor().is_some();
    let digging = ui == Tool::Eraser && object.is_none() && !scattered;
    let asset = ui.asset();
    let zoom = match (asset, &object) {
        (Some((_, zoom)), _) => zoom,
        (None, Some(o)) => o.zoom,
        // A terrain carve takes a whole cell. The scan emits nothing for a
        // zero material, whatever its bytes below the cell, so the whole cell
        // goes anyway.
        (None, None) => GROUND.leaf_cell_zoom(),
    };
    let (blocked, blocked_key) = match (&hovered, asset) {
        (Some(h), Some((asset, _))) => {
            let blocked_key = Some((asset, zoom, *h, world.revision()));
            let was = *PREVIEW.get(Preview::default);
            let same = was.blocked_key == blocked_key
                && !changes::within(placement_reads(asset, zoom, h), &[]);
            (
                if same {
                    was.blocked
                } else {
                    is_placement_blocked(asset, zoom, h)
                },
                blocked_key,
            )
        }
        _ => (false, None),
    };
    // The real object is hidden a tick or more after the ghost appears.
    let hidden = object.filter(|_| ui == Tool::Eraser);
    let edits = RenderEdits {
        hide: hidden.map(|o| o.id()),
        // Where the eraser hovers bare terrain, an AIR entry that is never
        // written goes into the drawn build, so what lies one voxel below
        // shows through the ghost.
        carve: hovered
            .filter(|_| digging)
            .map(|h| Octant::holding(zoom, &h)),
        scattered_cell: hover
            .scattered_decor()
            .filter(|_| ui == Tool::Eraser)
            .and_then(|s| s.generated_in()),
    };
    let was = core::mem::replace(
        PREVIEW.get(Preview::default),
        Preview {
            object,
            zoom,
            digging,
            blocked,
            blocked_key,
            edits,
        },
    );
    was.edits != edits
}

#[cfg(test)]
#[path = "write.test.rs"]
mod test;
