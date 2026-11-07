// pxl-core/domain/scene.rs

//! What a frame draws beside the screen's rays: each drawn layer keeps its
//! own state and lists itself into the draw list (domain/scene/model.rs
//! `draw_list`), in the order here.

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
pub mod resolution;
pub mod stage;
pub mod transform;

use pxl_render::gpu::draw::DrawList;

use crate::domain::scene::character::companion::{companion_animate, companion_draw};
use crate::domain::scene::character::surface::character_surface_shows;
use crate::domain::scene::environment::environment_animate;
use crate::domain::scene::frame::Frame;
use crate::domain::scene::gizmo::coverage_gizmo::coverage_gizmo_draw;
use crate::domain::scene::gizmo::frame_gizmo::frame_gizmo_draw;
use crate::domain::scene::indicator::ghost::ghost_draw;
use crate::domain::scene::indicator::location::{location_animate, location_draw};
use crate::domain::scene::model::registry::Models;

/// Advances what animates; whether anything did.
pub fn animate(list: &mut DrawList, models: &Models, frame: &Frame) -> bool {
    companion_animate(list, models, frame, character_surface_shows())
        | location_animate(frame)
        | environment_animate(frame)
}

/// Sets the scene's own layers' draws into the turn's list.
pub fn draw(list: &mut DrawList, models: &Models) {
    ghost_draw(list);
    companion_draw(list, models);
    location_draw(list);
    frame_gizmo_draw(list);
    coverage_gizmo_draw(list);
}

#[cfg(test)]
#[path = "scene.test.rs"]
mod test;
