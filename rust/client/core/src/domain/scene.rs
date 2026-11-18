// pxl-core/domain/scene.rs

//! The scene is what a frame draws besides the world the screen's rays cast.
//! Each of its parts keeps its own state and adds its draws to the one draw
//! list (domain/scene/model.rs `draw_list`), in the order `draw` calls them.

pub(crate) mod animation;
pub(crate) mod cast;
pub(crate) mod character;
pub(crate) mod decor;
pub(crate) mod environment;
pub(crate) mod frame;
pub(crate) mod gizmo;
pub(crate) mod indicator;
pub(crate) mod lane;
pub(crate) mod marker;
pub(crate) mod model;
pub(crate) mod param;
pub(crate) mod rate;
pub(crate) mod resolution;
pub(crate) mod stage;
pub(crate) mod transform;

use pxl_render::gpu::draw::DrawList;

use crate::domain::scene::character::companion::{companion_animate, companion_draw};
use crate::domain::scene::character::surface::character_surface_shows;
use crate::domain::scene::environment::environment_animate;
use crate::domain::scene::frame::Frame;
use crate::domain::scene::gizmo::coverage::coverage_gizmo_draw;
use crate::domain::scene::gizmo::frame::frame_gizmo_draw;
use crate::domain::scene::indicator::ghost::ghost_draw;
use crate::domain::scene::indicator::location::{location_animate, location_draw};
use crate::domain::scene::model::registry::ModelTable;

/// Returns whether anything is still moving, in which case the stage asks for
/// another frame.
pub(crate) fn animate(list: &mut DrawList, models: &ModelTable, frame: &Frame) -> bool {
    companion_animate(list, models, frame, character_surface_shows())
        | location_animate(list, frame)
        | environment_animate(frame)
}

pub(crate) fn draw(list: &mut DrawList, models: &ModelTable) {
    ghost_draw(list);
    companion_draw(list, models);
    location_draw(list);
    frame_gizmo_draw(list);
    coverage_gizmo_draw(list);
}

#[cfg(test)]
#[path = "scene.test.rs"]
mod test;
