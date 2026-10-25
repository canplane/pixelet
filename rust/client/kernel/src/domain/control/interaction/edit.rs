// pxl-kernel/domain/control/interaction/edit.rs

//! What an edit writes and what it reads to decide: placing, removing and
//! carving objects, read through the world's query and written to the store.

use crate::domain::control::interaction::collision::{is_placement_blocked, placement_reads, PlacementTool};
use crate::domain::control::interaction::{holding, hover, ActiveTool};
use crate::domain::layer::object::ext::Ext;
use crate::domain::layer::object::preview::{draft, redraft, words_at, Draft, RenderEdits};
use crate::domain::layer::object::size::MARKER_ZOOM;
use pxl_spec::asset::cube::{cube_id, Cube};
use pxl_spec::asset::decor_num_id;
use pxl_spec::asset::marker::MARKER;
use pxl_spec::asset::marker::MARKER_TEXT_CHARS;
use pxl_spec::asset::{cube, DecorId};
use pxl_spec::block::object::object_value_is_marker;
use pxl_spec::space::coord::q3::{self, Q3};
use pxl_spec::space::coord::world;
use pxl_spec::space::coord::zoom::{base_of, cube, units, Zoom};
use pxl_spec::space::hierarchy::morton::key_of;
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec_core::answer::PLACED_BLOCK;
use xpute_guest::global::Global;
use xpute_kit::status::errno::Errno;

use crate::domain::layer::instance::Instance;
use crate::domain::layer::object::repository;
use crate::domain::layer::object::store::{apply, Entry, Write, AIR, VOID};
use crate::domain::query::World;
use pxl_spec::asset::decor::Decor;
use pxl_spec::bake::constant::EntryAt;

/// Whether air is written where `at` is: a carve of the ground, or what was
/// placed there taken back.
fn carved(at: &Q3) -> bool {
    World::now().block_of(at).value() == Some(PLACED_BLOCK | AIR)
}

// ---- Mutations ----

/// A write of VOID collapses its path back, so a redundant record never sits in a save.
fn write_object_value(at: EntryAt, val: u32) {
    submit(Write::at(at, val));
}

/// A write is made here at once and sent to be kept, where the world takes
/// an edit (`World::editable`); elsewhere it is refused.
fn submit(write: Write) {
    let (zoom, at) = pxl_spec::space::hierarchy::morton::node_of::<3>(write.key);
    if !World::now().editable(&cube(zoom, at)) {
        return;
    }
    repository::send(&write);
    apply([write]);
}

/// Whether a whole-cell pick of a look only undoes a carve: the cell holds
/// an AIR leaf, and the ground there without it is that look, its stratum's
/// at that depth (without the carve, every cube stacked on matching ground
/// was dropped).
fn place_value_is_redundant(at: &Q3, zoom: Zoom, cube: Cube) -> bool {
    if zoom > GROUND.cell() || !carved(at) {
        return false;
    }
    let Cube::Ground(look) = cube else { return false };
    let entry = EntryAt::holding(zoom, at);
    // Only the resident finest leaf can say; elsewhere the cube is written as asked.
    let ground = World::now().previewing(Some(entry), None).block_of(&middle(entry)).exact();
    ground == Some(look as u32)
}

/// The middle of the cube `at` names.
fn middle(at: EntryAt) -> Q3 {
    let (base, half) = (base_of(at.zoom(), at.at()), q3::from_real(units(at.zoom()) / 2.0));
    world::at(i64::from(base[0]) + half, i64::from(base[1]) + half, i64::from(base[2]) + half)
}

/// A redundant re-paint of a carve writes VOID, as remove() does.
pub fn place(at: &Q3, zoom: Zoom, cube: Cube) {
    let val = if place_value_is_redundant(at, zoom, cube) { VOID } else { cube_id(cube) };
    write_object_value(EntryAt::holding(zoom, at), val);
}

/// Placing the decor a scatter stands in a cell an erase emptied writes
/// VOID, so the scattered instance returns as it was.
pub fn place_decor(at: &Q3, decor_id: DecorId) {
    let value = if restores_scattered(at, decor_id).is_some() { VOID } else { decor_num_id(decor_id) };
    write_object_value(EntryAt::holding(PlacementTool::Decor(decor_id).zoom(), at), value);
}

/// The decor the ground grows that `place_decor` would bring back, for its preview.
pub fn restores_scattered(at: &Q3, decor_id: DecorId) -> Option<Instance> {
    (PlacementTool::Decor(decor_id).zoom() <= GROUND.cell() && carved(at)).then(|| World::now().grown(Decor::of(decor_id), at))?
}

/// A marker tool's tap at `at`, a place free or holding a marker: the marker
/// waits there, with the marker's own words if one stands there.
pub fn wait_at(at: EntryAt) {
    *redraft() = Some(Draft {
        at,
        words: words_at(at).unwrap_or_default().to_string(),
    });
}

/// A marker tool's tap on the free cell whose min corner is `corner`.
pub fn wait_to_put(corner: &Q3) {
    wait_at(EntryAt::holding(MARKER_ZOOM, corner));
}

/// Nothing waits any more.
pub fn cancel_marker() {
    *redraft() = None;
}

/// Whether a marker waits, and its words, for the shell.
pub fn marker_waiting() -> (bool, &'static str) {
    draft().map_or((false, ""), |d| (true, d.words.as_str()))
}

/// The waiting marker's words as they are now, written into the draft's own
/// buffer, so a keystroke allocates only past the longest words it has held. Words past
/// `MARKER_TEXT_CHARS` are refused whole, and the words before stay.
pub fn draft_words(text: &str) -> Result<(), Errno> {
    if text.chars().count() > MARKER_TEXT_CHARS as usize {
        return Err(Errno::EMSGSIZE);
    }
    if let Some(d) = redraft() {
        d.words.clear();
        d.words.push_str(text);
    }
    Ok(())
}

/// The waiting marker put up with its words, where its place is still free
/// or still a marker; none is put up without words.
pub fn put_marker() {
    let Some(Draft { at, words }) = redraft().take() else { return };
    let (zoom, cell) = (at.zoom(), at.at());
    let stands = match World::now().placed(at) {
        Some(e) => object_value_is_marker(e.state),
        None => !is_placement_blocked(PlacementTool::Marker, &base_of(zoom, cell)),
    };
    if stands && !words.is_empty() {
        submit(Write {
            key: key_of::<3>(zoom, cell),
            id: MARKER,
            ext: Some(Ext::MarkerText(words)),
        });
    }
}

/// An AIR leaf: "nothing here, on purpose". Unlike remove(), it also hides
/// baked terrain, which has no leaf of its own to delete.
pub fn place_air(at: EntryAt) {
    write_object_value(at, cube::object::air as u32);
}

/// The eraser leaves its cell empty: where the ground fills it once the
/// block is gone, air is written, so a block taken out of a hole leaves the
/// hole. A decor the ground grew whose box it shared comes back, its erase
/// given back too.
pub fn remove(at: EntryAt) {
    let world = World::now();
    let erased: Vec<[i32; 3]> = world
        .grown_over(&cube(at.zoom(), at.at()))
        .into_iter()
        .filter_map(|i| i.grown_in())
        .filter(|&c| carved(&base_of(GROUND.cell(), c)))
        .collect();
    // Only the resident finest leaf can say what fills the cube without it, as
    // `place_value_is_redundant` asks; elsewhere it is emptied as asked.
    let id = match world.previewing(Some(at), None).block_of(&middle(at)).exact() {
        Some(v) if v != VOID && !pxl_spec_core::answer::placed(v) => AIR,
        None => AIR,
        Some(_) => VOID,
    };
    submit(Write {
        key: key_of::<3>(at.zoom(), at.at()),
        id,
        ext: None,
    });
    for c in erased {
        write_object_value(EntryAt::new(GROUND.cell(), c), VOID);
    }
}

/// The edit the pointer would make, decided once a turn: the cursor's ghost
/// shows it, and what it removes is handed to what is drawn to leave out.
#[derive(Clone, Copy, Default)]
pub struct Pending {
    pub object: Option<Entry>,
    /// The cube the edit fills or takes.
    pub zoom: Zoom,
    pub digging: bool,
    /// The same check as the commit, so preview and placement cannot disagree.
    pub blocked: bool,
    /// What `blocked` was asked of: the tool, its zoom with it, the hovered
    /// corner and the store's revision. Asked again only where one moved or
    /// a change touched the columns it reads (`placement_reads`,
    /// `changed::within`), as a step each frame turn sees each change once.
    asked: Option<(PlacementTool, Q3, u32)>,
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
    let world = World::now();
    let object = hover.object_at.and_then(|at| world.placed(at));
    let scattered = hover.scattered.is_some();
    let digging = active_tool == ActiveTool::Eraser && object.is_none() && !scattered;
    let tool = PlacementTool::of(&ui);
    let zoom = match (tool, &object) {
        (Some(tool), _) => tool.zoom(),
        (None, Some(o)) => o.zoom,
        // A terrain carve is a whole cell: the scan emits nothing for a zero
        // material whatever its sub-cell bytes, so the cell goes whole anyway.
        (None, None) => GROUND.cell(),
    };
    let (blocked, asked) = match (&hovered, tool) {
        (Some(h), Some(tool)) => {
            let asked = Some((tool, *h, world.revision()));
            let was = *PENDING.get(Pending::default);
            let same = was.asked == asked && !crate::domain::query::changed::within(placement_reads(tool, h), &[]);
            (if same { was.blocked } else { is_placement_blocked(tool, h) }, asked)
        }
        _ => (false, None),
    };
    // The real object's hide lands a tick or more after the ghost, accepted.
    let hidden = object.filter(|_| active_tool == ActiveTool::Eraser);
    let edits = RenderEdits {
        hide: hidden.map(|o| o.id()),
        // The never-written AIR entry injected into the drawn build where
        // the eraser hovers bare terrain: what is one voxel below shows
        // through the ghost.
        carve: hovered.filter(|_| digging).map(|h| EntryAt::holding(zoom, &h)),
        scattered: hover.scattered.filter(|_| active_tool == ActiveTool::Eraser).and_then(|s| s.grown_in()),
    };
    *PENDING.get(Pending::default) = Pending {
        object,
        zoom,
        digging,
        blocked,
        asked,
    };
    edits
}
