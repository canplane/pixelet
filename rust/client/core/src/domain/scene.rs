// pxl-core/domain/scene.rs

//! The scene is what a frame draws besides the world the screen's rays cast.
//! Each of its parts keeps its own state and adds its draws to the one draw
//! list (domain/scene/model.rs `draw_list`), in the order `draw` calls them.

pub mod animation;
pub mod cast;
pub mod character;
pub mod column;
pub mod decor;
pub mod environment;
pub mod frame;
pub mod gizmo;
pub mod indicator;
pub mod marker;
pub mod model;
pub mod param;
pub mod rate;
pub mod resolution;
pub mod stage;
pub mod transform;

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
pub fn animate(list: &mut DrawList, models: &ModelTable, frame: &Frame) -> bool {
    companion_animate(list, models, frame, character_surface_shows())
        | location_animate(frame)
        | environment_animate(frame)
}

pub fn draw(list: &mut DrawList, models: &ModelTable) {
    ghost_draw(list);
    companion_draw(list, models);
    location_draw(list);
    frame_gizmo_draw(list);
    coverage_gizmo_draw(list);
}

#[cfg(test)]
#[path = "scene.test.rs"]
mod test;
