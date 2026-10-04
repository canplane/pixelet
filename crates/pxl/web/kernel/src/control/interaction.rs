// pxl-kernel/control/interaction.rs

//! What the pointer picks and edits, apart from what it does to the camera:
//! the tool it holds, what is under it, and what a tap does with that.

pub mod pick;

use pxl_spec::bake::constant::{CELL, U_PER_CELL};
use pxl_spec::cell::object::PlaceableCube;
use pxl_spec::cell::{cube_types, DecorId};
use pxl_spec::space::pvf3::ivec::IVec;
use pxl_spec::space::pvf3::pvf3::Pvf3;
use pxl_spec::space::v3::V3;

pub use crate::abi::cmd::ActiveTool;
use crate::control::interaction::pick::{ray_pick, resolve_hit_object, world_adjacent_corner, world_struck_corner};
use crate::control::navigation::support::stand;
use crate::global::Global;
use crate::layer::decor::hit::{raycast_decor, DecorStruck, Scattered};
use crate::layer::object::collision::{is_placement_blocked, PlacementTool};
use crate::layer::object::geometry::{anchor_center, object_aabb};
use crate::layer::object::size::{DECOR_SIZE, DEFAULT_OBJECT_SIZE};
use crate::layer::object::store::{get_placed_object, place, place_air, place_decor, remove, EntryAt, OccupiedEntry};
use crate::world::access::Hit;
use crate::world::frame::view::{view, view_ray_local};
use xpute_runtime::global::Global;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Holding {
    pub active_tool: ActiveTool,
    /// The hotbar only ever sets this to a flat color, but the drawer picks
    /// terrain-styled cubes too.
    pub object_cube: PlaceableCube,
    pub object_size: f64,
    pub decor_id: DecorId,
}

impl Default for Holding {
    fn default() -> Holding {
        Holding {
            active_tool: ActiveTool::Pointer,
            object_cube: PlaceableCube::Block(cube_types::Block::red),
            object_size: DEFAULT_OBJECT_SIZE,
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
fn resolve_struck(hit: &Hit, origin: V3, direction: V3, map_origin: IVec) -> Option<Struck> {
    match raycast_decor(origin, direction, map_origin, hit.t) {
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
fn resolve_hit_target(hit: &Hit, origin: V3, direction: V3, map_origin: IVec) -> Option<Entry> {
    match resolve_struck(hit, origin, direction, map_origin) {
        Some(Struck::Object(o)) => Some(o),
        _ => resolve_hit_object(hit),
    }
}

/// What is under the pointer at `ndc_x`, `ndc_y`, for the tool held.
pub(crate) fn hover_at(ndc_x: f64, ndc_y: f64) {
    let mut origin = [0.0; 3];
    let mut direction = [0.0; 3];
    view_ray_local(ndc_x, ndc_y, &mut origin, &mut direction);

    let ui = interaction().holding;
    let map_origin = view().origin;
    let Some(pick) = ray_pick(origin, direction) else {
        return clear_hover();
    };
    let hit = pick.hit;
    let hover = &mut interaction().hover;

    if matches!(ui.active_tool, ActiveTool::Object | ActiveTool::Decor) {
        hover.object_at = None;
        hover.scattered = None;
        hover.approach = None;
        let placement_size = if ui.active_tool == ActiveTool::Decor { DECOR_SIZE } else { ui.object_size };
        // A decor met in front of the pick is what the new cube goes flush against.
        let precise_box = resolve_hit_target(&hit, origin, direction, map_origin).map(|o| object_aabb(&o, map_origin));
        hover.coord = Some(world_adjacent_corner(&hit, placement_size, map_origin, precise_box.as_ref()));
        return;
    }

    let struck = resolve_struck(&hit, origin, direction, map_origin);
    hover.object_at = match &struck {
        Some(Struck::Object(o)) => Some(o.at()),
        _ => None,
    };
    hover.scattered = match struck {
        Some(Struck::Scattered(s)) => Some(s),
        _ => None,
    };
    // A scattered decor is passed through.
    hover.approach = Some(match &struck {
        Some(Struck::Object(o)) => world_adjacent_corner(&hit, U_PER_CELL, map_origin, Some(&object_aabb(o, map_origin))),
        Some(Struck::Scattered(s)) => CELL.base_of(s.cell),
        None => world_adjacent_corner(&hit, U_PER_CELL, map_origin, None),
    });
    hover.coord = Some(match struck {
        Some(Struck::Object(o)) => o.corner(),
        Some(Struck::Scattered(s)) => CELL.base_of(s.cell),
        // On the cell's grid, not the tier's: that is the size a carve is written at.
        None => world_struck_corner(&hit, U_PER_CELL, map_origin),
    });
}

/// A tap at `ndc_x`, `ndc_y` with the tool held.
pub(crate) fn tap(ndc_x: f64, ndc_y: f64) {
    // A badge takes the tap before the ground under it, with the pointer tool only.
    if interaction().holding.active_tool == ActiveTool::Pointer && crate::layer::poi::placement::tap(ndc_x, ndc_y) {
        return;
    }
    hover_at(ndc_x, ndc_y);
    let Some(hovered) = interaction().hover.coord else { return };
    let ui = interaction().holding;
    let hover = interaction().hover;
    let object_size = ui.object_size;

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
                (None, Some(s)) => place_air(&CELL.base_of(s.cell), U_PER_CELL),
                // AIR, not VOID: terrain has no leaf to delete. A whole cell, never the
                // size tier: the scan emits nothing for a zero material whatever its
                // sub-cell bytes, so a smaller leaf would save a bite nobody took.
                (None, None) => place_air(&hovered, U_PER_CELL),
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
