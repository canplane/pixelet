// pxl-kernel/domain/control/interaction.rs

//! What the pointer picks and edits, apart from what it does to the camera:
//! the tool it holds, what is under it, and what a tap does with that.

pub mod collision;
pub mod edit;
pub mod ghost;
pub mod pick;

use pxl_spec::asset::cube::Cube;
use pxl_spec::asset::{cube, DecorId};
use pxl_spec::space::camera::grid::Tau;
use pxl_spec::space::coord::q3::Q3;
use pxl_spec::space::coord::v3;
use pxl_spec::space::layer::lattice::GROUND;

pub use crate::domain::control::cmd::ActiveTool;
use crate::domain::control::interaction::collision::{anchor_center, object_aabb};
use crate::domain::control::interaction::collision::{is_placement_blocked, PlacementTool};
use crate::domain::control::interaction::edit::{
    place, place_air, place_decor, remove, wait_to_put,
};
use crate::domain::control::interaction::pick::{
    ray_pick, resolve_hit_object, world_adjacent_corner, world_struck_corner,
};
use crate::domain::control::navigation::support::stand;
use crate::domain::layer::instance::Instance;
use crate::domain::layer::object::store::Entry;
use crate::domain::query::{Hit, World};
use crate::domain::view::frame::pose::ray_of;
use pxl_spec::bake::constant::EntryAt;
use pxl_spec::space::coord::zoom::{base_of, cube, index_of, of_side, units, Zoom};
use xpute_guest::global::Global;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Holding {
    pub active_tool: ActiveTool,
    /// A placed color or a ground look.
    pub object_cube: Cube,
    /// The object size by its tier (`object_zoom_of`).
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
        crate::domain::query::Struck::Decor(i) => match i.grown_in() {
            Some(cell) => Some(Struck::Grown(cell, i)),
            None => resolve_hit_object(hit).map(Struck::Object),
        },
        _ => resolve_hit_object(hit).map(Struck::Object),
    }
}

/// What the hover's pick met, and the cell zoom the eye's rays draw where
/// it struck, by the rule they are cast by (domain/query/ray.rs `by_distance`),
/// which near the eye is finer than the ground's cells, as what is placed
/// may be; none without an eye.
pub(crate) struct Picked {
    pub hit: Hit,
    pub drawn: Option<Zoom>,
}

impl Picked {
    /// The zoom of the box struck.
    pub fn held(&self) -> Zoom {
        of_side(self.hit.cube.extent()[0])
    }

    /// Whether the rays draw coarser there than the box struck: a summary,
    /// not what is held, so nothing in it is struck.
    pub fn folded(&self) -> bool {
        self.drawn.is_some_and(|z| z < self.held())
    }
}

/// What the hover picks at `ndc_x`, `ndc_y` on the world drawn at `drawn`,
/// from the pose as the camera holds it now, which a door event or a
/// controller may have moved since the turn composed the view.
pub(crate) fn picked(ndc_x: f64, ndc_y: f64, drawn: Tau) -> Option<Picked> {
    let pose = crate::domain::control::camera::controller::pose()?;
    let (mut origin, mut direction) = ([0.0; 3], [0.0; 3]);
    ray_of(pose, ndc_x, ndc_y, &mut origin, &mut direction);
    let hit = ray_pick(origin, direction, drawn)?;
    let drawn = crate::domain::view::observation::eye().map(|eye| {
        let v = v3::sub(&hit.cube.center(), &eye);
        crate::domain::query::by_distance(drawn).at(v3::dot(&v, &v) as f32)
    });
    Some(Picked { hit, drawn })
}

/// What is under the pointer at `ndc_x`, `ndc_y`, for the tool held.
pub(crate) fn hover_at(ndc_x: f64, ndc_y: f64, drawn: Tau) {
    let ui = interaction().holding;
    interaction().pointed = Some([ndc_x, ndc_y]);
    let hover = &mut interaction().hover;
    let Some(Picked { hit, .. }) = picked(ndc_x, ndc_y, drawn).filter(|p| !p.folded()) else {
        *hover = Hover::default();
        return;
    };

    if let Some(tool) = PlacementTool::of(&ui) {
        hover.object_at = None;
        hover.scattered = None;
        hover.approach = None;
        // A decor met in front of the pick is what the new cube goes flush
        // against; a scattered one is no entry, so it lands as on the ground.
        let precise_box = resolve_hit_object(&hit).map(|o| object_aabb(&o));
        hover.coord = Some(world_adjacent_corner(
            &hit,
            tool.size(),
            precise_box.as_ref(),
        ));
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
        Some(Struck::Object(o)) => {
            world_adjacent_corner(&hit, units(GROUND.cell()), Some(&object_aabb(o)))
        }
        Some(Struck::Grown(cell, _)) => base_of(GROUND.cell(), *cell),
        None => world_adjacent_corner(&hit, units(GROUND.cell()), None),
    });
    let (corner, zoom) = match struck {
        Some(Struck::Object(o)) => (o.corner(), o.zoom),
        Some(Struck::Grown(cell, _)) => (base_of(GROUND.cell(), cell), GROUND.cell()),
        // On the cell's grid, not the tier's: that is the size a carve is written at.
        None => (
            world_struck_corner(&hit, units(GROUND.cell())),
            GROUND.cell(),
        ),
    };
    // Past where the world takes an edit, nothing is pointed at.
    if !World::now().editable(&cube(zoom, index_of(zoom, &corner))) {
        *hover = Hover::default();
        return;
    }
    hover.coord = Some(corner);
}

/// A tap at `ndc_x`, `ndc_y` with the tool held, on the world drawn at `drawn`.
pub fn tap(ndc_x: f64, ndc_y: f64, drawn: Tau) {
    hover_at(ndc_x, ndc_y, drawn);
    let Some(hovered) = interaction().hover.coord else {
        return;
    };
    let ui = interaction().holding;
    let hover = interaction().hover;

    match PlacementTool::of(&ui) {
        // The slot may have filled since the last hover update.
        Some(tool) if is_placement_blocked(tool, &hovered) => {}
        Some(PlacementTool::Object(zoom)) => place(&hovered, zoom, ui.object_cube),
        Some(PlacementTool::Decor(id)) => place_decor(&hovered, id),
        // It waits there for its words, which the shell asks for.
        Some(PlacementTool::Marker) => wait_to_put(&hovered),
        None if ui.active_tool == ActiveTool::Pointer => {
            // Recenter across the struck face, standing as a pan lands.
            let Some(approach) = hover.approach else {
                return;
            };
            let target = stand(&anchor_center(&approach));
            crate::domain::control::navigation::destination::go_to(
                target,
                crate::domain::control::camera::controller::Motion::Smooth,
            );
        }
        // The eraser.
        None => {
            match (hover.object_at, hover.scattered) {
                (Some(at), _) => remove(at),
                (None, Some(s)) => {
                    if let Some(cell) = s.grown_in() {
                        place_air(EntryAt::new(GROUND.cell(), cell));
                    }
                }
                // AIR, not VOID: terrain has no leaf to delete. A whole cell, never the
                // size tier: the scan emits nothing for a zero material whatever its
                // sub-cell bytes, so a smaller leaf would save a bite nobody took.
                (None, None) => place_air(EntryAt::holding(GROUND.cell(), &hovered)),
            }
        }
    }
}
