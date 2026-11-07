// pxl-kernel/domain/control/edit.rs

//! What the pointer picks and edits, apart from what it does to the camera:
//! the tool it holds, what is under it, and what a tap does with that.

pub mod collision;
pub mod ghost;
pub mod pick;
pub mod write;

use pxl_spec::space::coord::aabb::Aabb;
use pxl_spec::space::coord::q3::Q3;
use pxl_spec::space::coord::v3;
use pxl_spec::space::layer::lattice::GROUND;

use crate::domain::control::camera::controller;
use crate::domain::control::camera::controller::Motion;
use crate::domain::control::edit::collision::is_placement_blocked;
use crate::domain::control::edit::collision::{anchor_center, object_aabb};
use crate::domain::control::edit::pick::{
    ray_pick, resolve_hit_object, world_adjacent_corner, world_struck_corner,
};
use crate::domain::control::edit::write::{place_air, remove};
use crate::domain::control::navigation::destination;
use crate::domain::control::support::stand;
use crate::domain::layer::instance::Instance;
use crate::domain::layer::object::asset::Asset;
use crate::domain::query;
use crate::domain::query::HitKind::Decor;
use crate::domain::query::HitKind::Object;
use crate::domain::query::{Hit, World};
use crate::domain::view::frame::pose::{eye, ray_of};
use pxl_spec::space::coord::zoom::{base_of, cube, idx_of, of_side, Zoom};
use pxl_spec::space::layer::octant::Octant;
use xpute_core::global::Global;

/// What the pointer holds, as the shell chose it (`EDIT_TOOL`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Tool {
    #[default]
    Pointer,
    Eraser,
    /// An item to place, at a scale chosen for a cube.
    Item {
        asset: Asset,
        scale_steps: i32,
    },
}

impl Tool {
    /// What it would place, and the zoom that comes to.
    pub fn asset(self) -> Option<(Asset, Zoom)> {
        match self {
            Tool::Item { asset, scale_steps } => Some((asset, asset.zoom(scale_steps))),
            Tool::Pointer | Tool::Eraser => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Hover {
    pub coord: Option<Q3>,
    pub target: Option<Target>,
    /// Where the pivot sent there starts from.
    pub approach: Option<Q3>,
}

struct State {
    tool: Tool,
    hover: Hover,
    /// Where the pointer stands on the screen, in NDC, whatever it can strike:
    /// the gauge casts its own ray there.
    pointer_ndc: Option<[f64; 2]>,
}

static STATE: Global<State> = Global::new();

fn state() -> &'static mut State {
    STATE.get(|| State {
        tool: Tool::default(),
        hover: Hover::default(),
        pointer_ndc: None,
    })
}

pub fn tool() -> Tool {
    state().tool
}

pub(crate) fn set_tool(tool: Tool) {
    state().tool = tool;
}

pub fn hover() -> Hover {
    state().hover
}

pub(crate) fn clear_hover() {
    state().hover = Hover::default();
    state().pointer_ndc = None;
}

/// Where the pointer stands on the screen, in NDC, while it is over it.
pub fn pointer_ndc() -> Option<[f64; 2]> {
    state().pointer_ndc
}

/// The pointer stands at `ndc_x`, `ndc_y` over something the world's pick
/// does not reach (pxl-kernel pointer.rs): nothing in the world is picked.
pub(crate) fn point_at(ndc_x: f64, ndc_y: f64) {
    state().pointer_ndc = Some([ndc_x, ndc_y]);
    state().hover = Hover::default();
}

/// A gesture under way points at nothing to place.
pub(crate) fn clear_hover_coord() {
    state().hover.coord = None;
}

/// What the pick struck, as a tool reads it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Target {
    /// A placed object.
    Object(Octant),
    /// A decor the ground generated, which has no entry, in its leaf cell.
    Generated([i32; 3], Instance),
}

impl Hover {
    pub fn object_at(&self) -> Option<Octant> {
        match self.target {
            Some(Target::Object(at)) => Some(at),
            _ => None,
        }
    }

    pub fn scattered_decor(&self) -> Option<Instance> {
        match self.target {
            Some(Target::Generated(_, i)) => Some(i),
            _ => None,
        }
    }
}

/// The box what is put across the face `hit` struck goes flush against, past
/// the hit's own: a placed cube's entry, as it is stored. A decor's is the
/// box it claims, which the hit carries.
fn flush_against(hit: &Hit) -> Option<Aabb> {
    match hit.kind {
        Object(_) => resolve_hit_object(hit).map(|o| object_aabb(&o)),
        _ => None,
    }
}

fn resolve_target(hit: &Hit) -> Option<Target> {
    match hit.kind {
        Decor(i) => match i.generated_in() {
            Some(cell) => Some(Target::Generated(cell, i)),
            None => resolve_hit_object(hit).map(|o| Target::Object(o.id())),
        },
        _ => resolve_hit_object(hit).map(|o| Target::Object(o.id())),
    }
}

/// What the hover's pick met, and the cell zoom the eye's rays draw where
/// it struck, by the rule they are cast by (domain/query/ray.rs `by_dist`),
/// which near the eye is finer than the ground's cells, as what is placed
/// may be; none without an eye.
pub(crate) struct Pick {
    pub hit: Hit,
    pub ray_zoom: Option<Zoom>,
}

/// The cell what `hit` struck stands in: a decor's, whatever its box, else
/// the box struck.
fn cell_struck(hit: &Hit) -> Aabb {
    match hit.kind {
        Decor(i) => i.cell(),
        _ => hit.cube,
    }
}

impl Pick {
    /// The zoom of the cell struck.
    pub(crate) fn zoom(&self) -> Zoom {
        of_side(cell_struck(&self.hit).extent()[0])
    }

    /// Whether the rays draw coarser there than the cell struck: a summary,
    /// not what is held, so nothing in it is struck.
    pub(crate) fn is_folded(&self) -> bool {
        self.ray_zoom.is_some_and(|z| z < self.zoom())
    }
}

/// What the hover picks at `ndc_x`, `ndc_y`,
/// from the pose as the camera holds it now, which a door event or a
/// controller may have moved since the turn composed the view.
pub(crate) fn pick_at(ndc_x: f64, ndc_y: f64) -> Option<Pick> {
    let pose = controller::pose()?;
    let (mut origin, mut dir) = ([0.0; 3], [0.0; 3]);
    ray_of(pose, ndc_x, ndc_y, &mut origin, &mut dir);
    let hit = ray_pick(origin, dir)?;
    let ray_zoom = eye().map(|eye| {
        let v = v3::sub(&cell_struck(&hit).center(), &eye);
        query::by_dist().at(v3::dot(&v, &v) as f32)
    });
    Some(Pick { hit, ray_zoom })
}

/// What is under the pointer at `ndc_x`, `ndc_y`, for the tool held.
pub(crate) fn hover_at(ndc_x: f64, ndc_y: f64) {
    let ui = state().tool;
    state().pointer_ndc = Some([ndc_x, ndc_y]);
    let hover = &mut state().hover;
    let Some(Pick { hit, .. }) = pick_at(ndc_x, ndc_y).filter(|p| !p.is_folded()) else {
        *hover = Hover::default();
        return;
    };

    if let Some((_, zoom)) = ui.asset() {
        hover.target = None;
        hover.approach = None;
        hover.coord = Some(world_adjacent_corner(
            &hit,
            zoom,
            flush_against(&hit).as_ref(),
        ));
        return;
    }

    let target = resolve_target(&hit);
    hover.target = target;
    hover.approach = Some(world_adjacent_corner(
        &hit,
        GROUND.cell(),
        flush_against(&hit).as_ref(),
    ));
    let (corner, zoom) = match target {
        Some(Target::Object(at)) => (base_of(at.zoom(), at.at()), at.zoom()),
        Some(Target::Generated(cell, _)) => (base_of(GROUND.cell(), cell), GROUND.cell()),
        // On the cell's grid, not the tier's: that is the size a carve is written at.
        None => (world_struck_corner(&hit, GROUND.cell()), GROUND.cell()),
    };
    // Past where the world takes an edit, nothing is pointed at.
    if !World::now().editable(&cube(zoom, idx_of(zoom, &corner))) {
        *hover = Hover::default();
        return;
    }
    hover.coord = Some(corner);
}

/// A tap at `ndc_x`, `ndc_y` with the tool held.
pub fn tap(ndc_x: f64, ndc_y: f64) {
    hover_at(ndc_x, ndc_y);
    let Some(hovered) = state().hover.coord else {
        return;
    };
    let ui = state().tool;
    let hover = state().hover;

    match ui.asset() {
        Some((asset, zoom)) => {
            // The slot may have filled since the last hover update.
            if !is_placement_blocked(asset, zoom, &hovered) {
                asset.place(&hovered, zoom);
            }
        }
        None if ui == Tool::Pointer => {
            // Recenter across the struck face, standing as a pan lands.
            let Some(approach) = hover.approach else {
                return;
            };
            let target = stand(&anchor_center(&approach));
            destination::go_to(target, Motion::Smooth);
        }
        // The eraser.
        None => {
            match hover.target {
                Some(Target::Object(at)) => remove(at),
                Some(Target::Generated(cell, _)) => place_air(Octant::new(GROUND.cell(), cell)),
                // AIR, not VOID: terrain has no leaf to delete. A whole cell, never the
                // size tier.
                None => place_air(Octant::holding(GROUND.cell(), &hovered)),
            }
        }
    }
    // What the tap wrote is under the pointer, which may not move again: the
    // world is picked again there.
    hover_at(ndc_x, ndc_y);
}
