// pxl-kernel/domain/scene.rs

//! What a frame draws beside the screen's rays: each drawn layer's own
//! state, listed into the draw list (domain/query/model.rs `draw_list`).

pub mod actor;
pub mod cast;
pub mod character;
pub mod decor;
pub mod environment;
pub mod frame;
pub mod indicator;
pub mod marker;
pub mod param;
pub mod stage;

use pxl_render::frame::draw::DrawList;
use pxl_spec::space::coord::v3::V3;
use xpute_guest::global::Global;

use crate::domain::query::model::registry::Models;
use crate::domain::scene::actor::Actors;
use crate::domain::scene::environment::EnvironmentDraw;
use crate::domain::scene::frame::Frame;
use crate::domain::scene::indicator::debug_line::DebugLines;
use crate::domain::scene::indicator::ghost::GhostDraw;
use crate::domain::scene::indicator::location_puck::LocationPuck;

/// The scene's state, one for the module's life.
static SCENE: Global<Scene> = Global::new();

pub fn scene() -> &'static mut Scene {
    SCENE.get(Scene::default)
}

#[derive(Default)]
pub struct Scene {
    pub ghost: GhostDraw,
    pub actors: Actors,
    pub puck: LocationPuck,
    pub environment: EnvironmentDraw,
    pub debug: DebugLines,
    /// The coverage gizmo's (domain/scene/indicator/coverage_gizmo.rs).
    pub coverage: DebugLines,
}

impl Scene {
    /// Advances what animates; whether anything did.
    pub fn tick(&mut self, draw: &mut DrawList, models: &Models, frame: &Frame) -> bool {
        self.actors.tick(draw, models, frame) | self.puck.tick(frame) | self.environment.tick(frame)
    }

    /// Begins the frame's list with the scene's own layers.
    pub fn begin(&mut self, draw: &mut DrawList, models: &Models, eye: V3) {
        draw.begin_frame();
        self.ghost.draw(draw, eye);
        self.actors.draw(draw, models, eye);
        self.puck.draw(draw, eye);
        self.debug.draw(draw, eye);
        self.coverage.draw(draw, eye);
    }
}
