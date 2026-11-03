// pxl-kernel/domain/control/edit/write.rs

//! What an edit writes and what it reads to decide: placing, removing and
//! carving objects, read through the world's query and written to the store.

use crate::domain::control::edit::collision::{is_placement_blocked, placement_reads, Placeable};
use crate::domain::control::edit::{hover, tool, ActiveTool};
use crate::domain::layer::object::ext::Ext;
use crate::domain::layer::object::preview::{draft, draft_mut, Draft, RenderEdits};
use crate::domain::layer::object::size::MARKER_ZOOM;
use pxl_engine::answer::PLACED_ASSET;
use pxl_spec::asset::cube::{cube_id, Cube};
use pxl_spec::asset::decor_num_id;
use pxl_spec::asset::marker::MARKER;
use pxl_spec::asset::marker::MARKER_TEXT_CHARS;
use pxl_spec::asset::{cube, DecorId};
use pxl_spec::object::is_marker;
use pxl_spec::space::coord::q3::{self, Q3};
use pxl_spec::space::coord::world;
use pxl_spec::space::coord::zoom::{base_of, cube, units, Zoom};
use pxl_spec::space::hierarchy::morton::key_of;
use pxl_spec::space::layer::lattice::GROUND;
use xpute_guest::global::Global;
use xpute_kit::status::errno::Errno;

use crate::domain::layer::instance::Instance;
use crate::domain::layer::object::repository;
use crate::domain::layer::object::store::{apply, Entry, Write, AIR, VOID};
use crate::domain::query::changes;
use crate::domain::query::World;
use pxl_engine::answer;
use pxl_spec::asset::decor::Decor;
use pxl_spec::bake::constant::EntryAt;
use pxl_spec::space::hierarchy::morton;

/// Whether air is written where `at` is: a carve of the ground, or what was
/// placed there taken back.
fn is_carved(world: &World, at: &Q3) -> bool {
    world.asset_of(at).val() == Some(PLACED_ASSET | AIR)
}

// ---- Mutations ----

/// A write of VOID collapses its path back, so a redundant record never sits in a save.
fn write_object_val(at: EntryAt, val: u32) {
    submit(Write::at(at, val));
}

/// A write is made here at once and, where it changed what is placed, sent
/// to be kept, where the world takes an edit (`World::editable`); elsewhere
/// it is refused.
fn submit(write: Write) {
    let (zoom, at) = morton::node_of::<3>(write.key);
    if !World::now().editable(&cube(zoom, at)) {
        return;
    }
    if apply([write.clone()]) {
        repository::send(write);
    }
}

/// Whether a whole-cell pick only undoes a carve: the cell holds an AIR leaf, and
/// the ground there without it is that cube, its stratum's at that depth.
fn place_val_is_redundant(world: World, at: &Q3, zoom: Zoom, cube: Cube) -> bool {
    if zoom > GROUND.cell() || !is_carved(&world, at) {
        return false;
    }
    let entry = EntryAt::holding(zoom, at);
    // Only the resident finest leaf can say; elsewhere the cube is written as asked.
    let ground = world
        .previewing(Some(entry), None)
        .asset_of(&middle(entry))
        .exact();
    ground == Some(cube_id(cube))
}

/// The middle of the cube `at` names.
pub(super) fn middle(at: EntryAt) -> Q3 {
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

/// A redundant re-paint of a carve writes VOID, as remove() does.
pub fn place(at: &Q3, zoom: Zoom, cube: Cube) {
    let val = if place_val_is_redundant(World::now(), at, zoom, cube) {
        VOID
    } else {
        cube_id(cube)
    };
    write_object_val(EntryAt::holding(zoom, at), val);
}

/// Placing the decor a scatter stands in a cell an erase emptied writes
/// VOID, so the scattered instance returns as it was.
pub fn place_decor(at: &Q3, decor_id: DecorId) {
    let val = if restores_scattered(at, decor_id).is_some() {
        VOID
    } else {
        decor_num_id(decor_id)
    };
    write_object_val(EntryAt::holding(Placeable::Decor(decor_id).zoom(), at), val);
}

/// The decor the ground generates that `place_decor` would bring back, for its preview.
pub fn restores_scattered(at: &Q3, decor_id: DecorId) -> Option<Instance> {
    (Placeable::Decor(decor_id).zoom() <= GROUND.cell() && is_carved(&World::now(), at))
        .then(|| World::now().generated_instance(Decor::of(decor_id), at))?
}

/// A marker tool's tap at `at`, a place free or holding a marker: a draft
/// opened there, with the marker's own text if one stands there.
pub fn draft_at(at: EntryAt) {
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
}

/// A marker tool's tap on the free cell whose min corner is `corner`: a new
/// marker's draft.
pub fn draft_new(corner: &Q3) {
    draft_at(EntryAt::holding(MARKER_ZOOM, corner));
}

/// The draft dropped.
pub fn cancel_draft() {
    *draft_mut() = None;
}

/// The draft's text, for the shell; none where no draft is open.
pub fn marker_draft() -> Option<&'static str> {
    draft().map(|d| d.text.as_str())
}

/// The draft's text as it is now, written into its own
/// buffer, so a keystroke allocates only past the longest text it has held. Text past
/// `MARKER_TEXT_CHARS` is refused whole, and the text before stays.
pub fn set_draft_text(text: &str) -> Result<(), Errno> {
    if text.chars().count() > MARKER_TEXT_CHARS as usize {
        return Err(Errno::EMSGSIZE);
    }
    if let Some(d) = draft_mut() {
        d.text.clear();
        d.text.push_str(text);
    }
    Ok(())
}

/// The draft committed with its text, where its place is still free or
/// still a marker; none is committed without text.
pub fn commit_draft() {
    let Some(Draft { at, text }) = draft_mut().take() else {
        return;
    };
    let (zoom, cell) = (at.zoom(), at.at());
    let stands = match World::now().entry_at(at) {
        Some(e) => is_marker(e.state),
        None => !is_placement_blocked(Placeable::Marker, &base_of(zoom, cell)),
    };
    if stands && !text.is_empty() {
        submit(Write {
            key: key_of::<3>(zoom, cell),
            id: MARKER,
            ext: Some(Ext::MarkerText(text)),
        });
    }
}

/// An AIR leaf: "nothing here, on purpose". Unlike remove(), it also hides
/// baked terrain, which has no leaf of its own to delete.
pub fn place_air(at: EntryAt) {
    write_object_val(at, cube::object::air as u32);
}

/// The eraser leaves its cell empty: where the ground fills it once the
/// asset is gone, air is written, so an asset taken out of a hole leaves the
/// hole. A decor the ground generated whose box it shared comes back, its erase
/// given back too.
pub fn remove(at: EntryAt) {
    let world = World::now();
    let erased: Vec<[i32; 3]> = world
        .generated_over(&cube(at.zoom(), at.at()))
        .into_iter()
        .filter_map(|i| i.generated_in())
        .filter(|&c| is_carved(&World::now(), &base_of(GROUND.cell(), c)))
        .collect();
    // Only the resident finest leaf can say what fills the cube without it, as
    // `place_val_is_redundant` asks; elsewhere it is emptied as asked.
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
        key: key_of::<3>(at.zoom(), at.at()),
        id,
        ext: None,
    });
    for c in erased {
        write_object_val(EntryAt::new(GROUND.cell(), c), VOID);
    }
}

/// The edit the pointer would make, decided once a turn: its ghost
/// shows it, and what it removes is handed to what is drawn to leave out.
#[derive(Clone, Copy, Default)]
pub struct Pending {
    pub object: Option<Entry>,
    /// The cube the edit fills or takes.
    pub zoom: Zoom,
    pub digging: bool,
    /// The same check as the commit, so preview and placement cannot disagree.
    pub blocked: bool,
    /// What `blocked` was asked of: what it places, its zoom with it, the hovered
    /// corner and the store's revision. Asked again only where one moved or
    /// a change touched the columns it reads (`placement_reads`,
    /// `changes::within`), as a step each frame turn sees each change once.
    blocked_key: Option<(Placeable, Q3, u32)>,
}

static PENDING: Global<Pending> = Global::new();

pub fn pending() -> Pending {
    *PENDING.get(Pending::default)
}

/// Returns what the edit leaves out of what is drawn.
pub fn pending_step() -> RenderEdits {
    let hover = hover();
    let ui = tool();
    let hovered = hover.coord;
    let active_tool = ui.active_tool;
    let world = World::now();
    let object = hover.object_at.and_then(|at| world.entry_at(at));
    let scattered = hover.scattered_decor.is_some();
    let digging = active_tool == ActiveTool::Eraser && object.is_none() && !scattered;
    let placeable = Placeable::of(&ui);
    let zoom = match (placeable, &object) {
        (Some(placeable), _) => placeable.zoom(),
        (None, Some(o)) => o.zoom,
        // A terrain carve is a whole cell: the scan emits nothing for a zero
        // material whatever its sub-cell bytes, so the cell goes whole anyway.
        (None, None) => GROUND.cell(),
    };
    let (blocked, blocked_key) = match (&hovered, placeable) {
        (Some(h), Some(placeable)) => {
            let blocked_key = Some((placeable, *h, world.revision()));
            let was = *PENDING.get(Pending::default);
            let same = was.blocked_key == blocked_key
                && !changes::within(placement_reads(placeable, h), &[]);
            (
                if same {
                    was.blocked
                } else {
                    is_placement_blocked(placeable, h)
                },
                blocked_key,
            )
        }
        _ => (false, None),
    };
    // The real object's hide lands a tick or more after the ghost.
    let hidden = object.filter(|_| active_tool == ActiveTool::Eraser);
    let edits = RenderEdits {
        hide: hidden.map(|o| o.id()),
        // The never-written AIR entry injected into the drawn build where
        // the eraser hovers bare terrain: what is one voxel below shows
        // through the ghost.
        carve: hovered
            .filter(|_| digging)
            .map(|h| EntryAt::holding(zoom, &h)),
        scattered_cell: hover
            .scattered_decor
            .filter(|_| active_tool == ActiveTool::Eraser)
            .and_then(|s| s.generated_in()),
    };
    *PENDING.get(Pending::default) = Pending {
        object,
        zoom,
        digging,
        blocked,
        blocked_key,
    };
    edits
}

#[cfg(test)]
#[path = "write.test.rs"]
mod test;
