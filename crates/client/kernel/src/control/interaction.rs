// pxl-kernel/control/interaction.rs

//! What the pointer picks and edits, apart from what it does to the camera:
//! the tool it holds, what is under it, and what a tap does with that.

pub mod collision;
pub mod edit;
pub mod ghost;
pub mod pick;

use pxl_spec::block::decor::Decor;
use pxl_spec::block::object::Cube;
use pxl_spec::block::{cube, DecorId};
use pxl_spec::space::camera::grid::Tau;
use pxl_spec::space::coord::q3::Q3;
use pxl_spec::space::layer::lattice::GROUND;

pub use crate::abi::cmd::ActiveTool;
use crate::control::interaction::collision::{anchor_center, object_aabb};
use crate::control::interaction::collision::{is_placement_blocked, PlacementTool};
use crate::control::interaction::edit::{place, place_air, place_decor, remove, wait_to_put};
use crate::control::interaction::pick::{ray_pick, resolve_hit_object, world_adjacent_corner, world_struck_corner};
use crate::control::navigation::support::stand;
use crate::layer::instance::Instance;
use crate::layer::object::size::{object_size_of, MARKER_SIZE};
use crate::layer::object::store::Entry;
use crate::query::{Hit, World};
use crate::view::frame::pose::view_ray;
use pxl_spec::bake::constant::EntryAt;
use pxl_spec::space::coord::zoom::{base_of, cube, index_of, of_side, units};
use xpute_guest::global::Global;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Holding {
    pub active_tool: ActiveTool,
    /// A placed color or a ground look.
    pub object_cube: Cube,
    /// The object size by its tier (`object_size_of`).
    pub object_tier: u32,
    pub decor_id: DecorId,
}

impl Default for Holding {
    fn default() -> Holding {
        Holding {
            active_tool: ActiveTool::Pointer,
            object_cube: Cube::Object(cube::Object::red),
            object_tier: 0,
            decor_id: DecorId::tree,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Hover {
    pub coord: Option<Q3>,
    pub object_at: Option<EntryAt>,
    /// A decor the ground grew, which has no entry.
    pub scattered: Option<Instance>,
    /// Where the focus sent there starts from.
    pub approach: Option<Q3>,
}

struct Interaction {
    holding: Holding,
    hover: Hover,
    /// Where the pointer stands on the screen, in NDC, whatever it can strike:
    /// the gauge casts its own ray there.
    pointed: Option<[f64; 2]>,
}

static INTERACTION: Global<Interaction> = Global::new();

fn interaction() -> &'static mut Interaction {
    INTERACTION.get(|| Interaction {
        holding: Holding::default(),
        hover: Hover::default(),
        pointed: None,
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
    interaction().pointed = None;
}

/// Where the pointer stands on the screen, in NDC, while it is over it.
pub fn pointed() -> Option<[f64; 2]> {
    interaction().pointed
}

/// A gesture under way points at nothing to place.
pub(crate) fn clear_hover_coord() {
    interaction().hover.coord = None;
}

enum Struck {
    Object(Entry),
    /// Grown by the ground, in its leaf cell.
    Grown([i32; 3], Instance),
}

/// What the pick struck, as a tool reads it: a placed object, or a decor the ground grew.
fn resolve_struck(hit: &Hit) -> Option<Struck> {
    match hit.struck {
        crate::query::Struck::Decor(i) => match i.grown_in() {
            Some(cell) => Some(Struck::Grown(cell, i)),
            None => resolve_hit_object(hit).map(Struck::Object),
        },
        _ => resolve_hit_object(hit).map(Struck::Object),
    }
}

/// What is under the pointer at `ndc_x`, `ndc_y`, for the tool held.
pub(crate) fn hover_at(ndc_x: f64, ndc_y: f64, drawn: Tau) {
    let mut origin = [0.0; 3];
    let mut direction = [0.0; 3];
    view_ray(ndc_x, ndc_y, &mut origin, &mut direction);

    let ui = interaction().holding;
    interaction().pointed = Some([ndc_x, ndc_y]);
    let Some(hit) = ray_pick(origin, direction, drawn) else {
        interaction().hover = Hover::default();
        return;
    };
    let hover = &mut interaction().hover;
    // A folded cell is a summary, not what is held: nothing in it is struck.
    if folded_at(&hit, drawn) {
        *hover = Hover::default();
        return;
    }

    if matches!(ui.active_tool, ActiveTool::Object | ActiveTool::Decor | ActiveTool::Marker) {
        hover.object_at = None;
        hover.scattered = None;
        hover.approach = None;
        let placement_size = match ui.active_tool {
            ActiveTool::Object => object_size_of(ui.object_tier),
            ActiveTool::Decor => units(Decor::of(ui.decor_id).row.cell_zoom()),
            _ => MARKER_SIZE,
        };
        // A decor met in front of the pick is what the new cube goes flush
        // against; a scattered one is no entry, so it lands as on the ground.
        let precise_box = resolve_hit_object(&hit).map(|o| object_aabb(&o));
        hover.coord = Some(world_adjacent_corner(&hit, placement_size, precise_box.as_ref()));
        return;
    }

    let struck = resolve_struck(&hit);
    hover.object_at = match &struck {
        Some(Struck::Object(o)) => Some(o.id()),
        _ => None,
    };
    hover.scattered = match struck {
        Some(Struck::Grown(_, s)) => Some(s),
        _ => None,
    };
    // A scattered decor is passed through.
    hover.approach = Some(match &struck {
        Some(Struck::Object(o)) => world_adjacent_corner(&hit, units(GROUND.cell()), Some(&object_aabb(o))),
        Some(Struck::Grown(cell, _)) => base_of(GROUND.cell(), *cell),
        None => world_adjacent_corner(&hit, units(GROUND.cell()), None),
    });
    let (corner, size) = match struck {
        Some(Struck::Object(o)) => (o.corner(), o.size()),
        Some(Struck::Grown(cell, _)) => (base_of(GROUND.cell(), cell), units(GROUND.cell())),
        // On the cell's grid, not the tier's: that is the size a carve is written at.
        None => (world_struck_corner(&hit, units(GROUND.cell())), units(GROUND.cell())),
    };
    // Past where the world takes an edit, nothing is pointed at.
    if !editable(&corner, size) {
        *hover = Hover::default();
        return;
    }
    hover.coord = Some(corner);
}

/// Whether the cube of `size` cornered at `corner` may hold an edit.
fn editable(corner: &Q3, size: f64) -> bool {
    let zoom = of_side(size);
    World::now().editable(&cube(zoom, index_of(zoom, corner)))
}

/// Whether what `hit` struck is drawn folded: the eye's rays draw cells
/// coarser than the box struck where it is (view/observation.rs `cell_at`).
fn folded_at(hit: &Hit, drawn: Tau) -> bool {
    let held = of_side(hit.cube.extent()[0]);
    crate::view::observation::eye()
        .and_then(|eye| crate::view::observation::cell_at(eye, drawn, hit.cube.center()))
        .is_some_and(|cells| cells < held)
}

/// A tap at `ndc_x`, `ndc_y` with the tool held, on the world drawn at `drawn`.
pub fn tap(ndc_x: f64, ndc_y: f64, drawn: Tau) {
    hover_at(ndc_x, ndc_y, drawn);
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
                (None, Some(s)) => {
                    if let Some(cell) = s.grown_in() {
                        place_air(&base_of(GROUND.cell(), cell), units(GROUND.cell()));
                    }
                }
                // AIR, not VOID: terrain has no leaf to delete. A whole cell, never the
                // size tier: the scan emits nothing for a zero material whatever its
                // sub-cell bytes, so a smaller leaf would save a bite nobody took.
                (None, None) => place_air(&hovered, units(GROUND.cell())),
            }
        }
        // The slot may have filled since the last hover update.
        ActiveTool::Decor => {
            if !is_placement_blocked(PlacementTool::Decor(ui.decor_id), &hovered, object_size) {
                place_decor(&hovered, units(Decor::of(ui.decor_id).row.cell_zoom()), ui.decor_id);
            }
        }
        // It waits there for its words, which the shell asks for.
        ActiveTool::Marker => {
            if !is_placement_blocked(PlacementTool::Marker, &hovered, object_size) {
                wait_to_put(&hovered);
            }
        }
        ActiveTool::Object => {
            if !is_placement_blocked(PlacementTool::Object, &hovered, object_size) {
                place(&hovered, object_size, ui.object_cube);
            }
        }
    }
}
