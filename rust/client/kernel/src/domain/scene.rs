// pxl-kernel/domain/scene.rs

//! What a frame draws beside the screen's rays: each drawn layer's own
//! state, listed into the draw list (domain/scene/model.rs `draw_list`).

pub mod actor;
pub mod cast;
pub mod character;
pub mod decor;
pub mod environment;
pub mod frame;
pub mod gizmo;
pub mod indicator;
pub mod lane;
pub mod marker;
pub mod model;
pub mod param;
pub mod region;
pub mod stage;

use pxl_render::frame::draw::DrawList;
use pxl_spec::space::coord::v3::V3;
use xpute_guest::global::Global;

use crate::domain::scene::character::companion::{companion_animate, companion_draw};
use crate::domain::scene::character::preview::{preview_animate, preview_draw};
use crate::domain::scene::environment::EnvironmentDraw;
use crate::domain::scene::frame::Frame;
use crate::domain::scene::gizmo::debug_line::DebugLines;
use crate::domain::scene::indicator::ghost::GhostDraw;
use crate::domain::scene::indicator::location_puck::LocationPuck;
use crate::domain::scene::model::registry::Models;

/// The scene's state, one for the module's life.
static SCENE: Global<Scene> = Global::new();

pub fn scene() -> &'static mut Scene {
    SCENE.get(Scene::default)
}

#[derive(Default)]
pub struct Scene {
    pub ghost: GhostDraw,
    pub puck: LocationPuck,
    pub environment: EnvironmentDraw,
    pub debug: DebugLines,
    /// The coverage gizmo's (domain/scene/gizmo/coverage_gizmo.rs).
    pub coverage: DebugLines,
}

impl Scene {
    /// Advances what animates; whether anything did.
    pub fn tick(&mut self, draw: &mut DrawList, models: &Models, frame: &Frame) -> bool {
        companion_animate(draw, models, frame)
            | preview_animate(draw, models, frame)
            | self.puck.tick(frame)
            | self.environment.tick(frame)
    }

    /// Begins the frame's list with the scene's own layers.
    pub fn begin(&mut self, draw: &mut DrawList, models: &Models, eye: V3) {
        draw.begin_frame();
        self.ghost.draw(draw, eye);
        companion_draw(draw, models, eye);
        preview_draw(draw, models, eye);
        self.puck.draw(draw, eye);
        self.debug.draw(draw, eye);
        self.coverage.draw(draw, eye);
    }
}
