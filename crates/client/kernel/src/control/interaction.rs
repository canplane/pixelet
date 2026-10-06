// pxl-kernel/control/interaction.rs

//! What the pointer picks and edits, apart from what it does to the camera:
//! the tool it holds, what is under it, and what a tap does with that.

pub mod pick;

use pxl_spec::cell::object::PlaceableCube;
use pxl_spec::cell::{cube_types, DecorId};
use pxl_spec::space::coord::pvf3::pvf3::Pvf3;
use pxl_spec::space::coord::v3::V3;
use pxl_spec::space::layer::lattice::GROUND;

pub use crate::abi::cmd::ActiveTool;
use crate::control::interaction::pick::{ray_pick, resolve_hit_object, world_adjacent_corner, world_struck_corner};
use crate::control::navigation::support::stand;
use crate::layer::object::collision::{anchor_center, object_aabb};
use crate::layer::object::collision::{is_placement_blocked, PlacementTool};
use crate::layer::object::size::{object_size_of, DECOR_SIZE};
use crate::layer::object::store::{get_placed_object, place, place_air, place_decor, remove, Entry, EntryAt};
use crate::world::chunk::decor::hit::{raycast_decor, DecorStruck, Scattered};
use crate::world::query::Hit;
use crate::world::view::frame::pose::view_ray;
use pxl_spec::space::coord::zoom::{base_of, of_side, units};
use xpute_runtime::global::Global;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Holding {
    pub active_tool: ActiveTool,
    /// A placed color or a ground look.
    pub object_cube: PlaceableCube,
    /// The object size by its tier (`object_size_of`).
    pub object_tier: u32,
    pub decor_id: DecorId,
}

impl Default for Holding {
    fn default() -> Holding {
        Holding {
            active_tool: ActiveTool::Pointer,
            object_cube: PlaceableCube::Block(cube_types::Block::red),
            object_tier: 0,
            decor_id: DecorId::tree,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Hover {
    pub coord: Option<Pvf3>,
    pub object_at: Option<EntryAt>,
    /// A scattered decor has no entry.
    pub scattered: Option<Scattered>,
    /// Where the map center sent there starts from.
    pub approach: Option<Pvf3>,
}

struct Interaction {
    holding: Holding,
    hover: Hover,
}

static INTERACTION: Global<Interaction> = Global::new();

fn interaction() -> &'static mut Interaction {
    INTERACTION.get(|| Interaction {
        holding: Holding::default(),
        hover: Hover::default(),
    })
}

pub fn holding() -> Holding {
    interaction().holding
}

pub(crate) fn hold(holding: Holding) {
    interaction().holding = holding;
}

pub fn hover() -> Hover {
    interaction().hover
}

pub(crate) fn clear_hover() {
    interaction().hover = Hover::default();
}

/// A gesture under way points at nothing to place.
pub(crate) fn clear_hover_coord() {
    interaction().hover.coord = None;
}

enum Struck {
    Object(Entry),
    Scattered(Scattered),
}

/// The decor mesh hit wins only if closer than `hit.t`.
fn resolve_struck(hit: &Hit, origin: V3, direction: V3) -> Option<Struck> {
    match raycast_decor(origin, direction, hit.t) {
        Some(DecorStruck::Placed(at)) => {
            if let Some(o) = get_placed_object(at) {
                return Some(Struck::Object(o));
            }
        }
        Some(DecorStruck::Scattered(s)) => return Some(Struck::Scattered(s)),
        None => {}
    }
    resolve_hit_object(hit).map(Struck::Object)
}

/// A scattered instance is not an entry, so a placement lands as on the ground.
fn resolve_hit_target(hit: &Hit, origin: V3, direction: V3) -> Option<Entry> {
    match resolve_struck(hit, origin, direction) {
        Some(Struck::Object(o)) => Some(o),
        _ => resolve_hit_object(hit),
    }
}

/// What is under the pointer at `ndc_x`, `ndc_y`, for the tool held.
pub(crate) fn hover_at(ndc_x: f64, ndc_y: f64) {
    let mut origin = [0.0; 3];
    let mut direction = [0.0; 3];
    view_ray(ndc_x, ndc_y, &mut origin, &mut direction);

    let ui = interaction().holding;
    let Some(pick) = ray_pick(origin, direction) else {
        return clear_hover();
    };
    let hit = pick.hit;
    let hover = &mut interaction().hover;
    // A folded cell is a summary, not what is held: nothing in it is struck.
    if folded_at(&hit) {
        return clear_hover();
    }

    if matches!(ui.active_tool, ActiveTool::Object | ActiveTool::Decor) {
        hover.object_at = None;
        hover.scattered = None;
        hover.approach = None;
        let placement_size = if ui.active_tool == ActiveTool::Decor { DECOR_SIZE } else { object_size_of(ui.object_tier) };
        // A decor met in front of the pick is what the new cube goes flush against.
        let precise_box = resolve_hit_target(&hit, origin, direction).map(|o| object_aabb(&o));
        hover.coord = Some(world_adjacent_corner(&hit, placement_size, precise_box.as_ref()));
        return;
    }

    let struck = resolve_struck(&hit, origin, direction);
    hover.object_at = match &struck {
        Some(Struck::Object(o)) => Some(o.id()),
        _ => None,
    };
    hover.scattered = match struck {
        Some(Struck::Scattered(s)) => Some(s),
        _ => None,
    };
    // A scattered decor is passed through.
    hover.approach = Some(match &struck {
        Some(Struck::Object(o)) => world_adjacent_corner(&hit, units(GROUND.cell()), Some(&object_aabb(o))),
        Some(Struck::Scattered(s)) => base_of(GROUND.cell(), s.cell),
        None => world_adjacent_corner(&hit, units(GROUND.cell()), None),
    });
    hover.coord = Some(match struck {
        Some(Struck::Object(o)) => o.corner(),
        Some(Struck::Scattered(s)) => base_of(GROUND.cell(), s.cell),
        // On the cell's grid, not the tier's: that is the size a carve is written at.
        None => world_struck_corner(&hit, units(GROUND.cell())),
    });
}

/// Whether what `hit` struck is drawn folded: the finest drawn chunk there
/// has cells coarser than the box struck.
fn folded_at(hit: &Hit) -> bool {
    let [x, y, z] = hit.cube.center();
    let at = crate::world::view::frame::coord::world_to_canonical(&[x, y, z]);
    let held = of_side(hit.cube.extent()[0]);
    crate::with_world(|_, sched, _| sched.cells_at(&at)).flatten().is_some_and(|cells| cells < held)
}

/// A tap at `ndc_x`, `ndc_y` with the tool held.
pub(crate) fn tap(ndc_x: f64, ndc_y: f64) {
    // A badge takes the tap before the ground under it, with the pointer tool only.
    if interaction().holding.active_tool == ActiveTool::Pointer && crate::world::chunk::place::tap(ndc_x, ndc_y) {
        return;
    }
    hover_at(ndc_x, ndc_y);
    let Some(hovered) = interaction().hover.coord else { return };
    let ui = interaction().holding;
    let hover = interaction().hover;
    let object_size = object_size_of(ui.object_tier);

    match ui.active_tool {
        ActiveTool::Pointer => {
            // Recenter across the struck face, standing as a pan lands.
            let Some(approach) = hover.approach else { return };
            let target = stand(&anchor_center(&approach));
            crate::control::navigation::destination::go_to(target, crate::control::camera::controller::Motion::Smooth);
        }
        ActiveTool::Eraser => {
            match (hover.object_at, hover.scattered) {
                (Some(at), _) => remove(at),
                (None, Some(s)) => place_air(&base_of(GROUND.cell(), s.cell), units(GROUND.cell())),
                // AIR, not VOID: terrain has no leaf to delete. A whole cell, never the
                // size tier: the scan emits nothing for a zero material whatever its
                // sub-cell bytes, so a smaller leaf would save a bite nobody took.
                (None, None) => place_air(&hovered, units(GROUND.cell())),
            }
        }
        ActiveTool::Decor | ActiveTool::Object => {
            // The slot may have filled since the last hover update.
            if ui.active_tool == ActiveTool::Decor {
                if !is_placement_blocked(PlacementTool::Decor, &hovered, object_size) {
                    place_decor(&hovered, DECOR_SIZE, ui.decor_id);
                }
            } else if !is_placement_blocked(PlacementTool::Object, &hovered, object_size) {
                place(&hovered, object_size, ui.object_cube);
            }
        }
    }
}
