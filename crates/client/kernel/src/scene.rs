// pxl-kernel/scene.rs

//! What a frame draws beside the screen's rays: each drawn layer's own
//! state, listed into the kernel's draw list.

pub mod actor;
pub mod cast;
pub mod character;
pub mod decor;
pub mod environment;
pub mod frame;
pub mod indicator;
pub mod place;
pub mod stage;

use pxl_render::frame::draw::DrawList;
use pxl_spec::space::coord::v3::V3;

use crate::model::registry::Models;
use crate::scene::actor::Actors;
use crate::scene::environment::EnvironmentDraw;
use crate::scene::frame::Frame;
use crate::scene::indicator::debug_lines::DebugLines;
use crate::scene::indicator::hover_cursor::GhostDraw;
use crate::scene::indicator::location_puck::LocationPuck;

#[derive(Default)]
pub struct Scene {
    pub ghost: GhostDraw,
    pub actors: Actors,
    pub puck: LocationPuck,
    pub environment: EnvironmentDraw,
    pub debug: DebugLines,
}

impl Scene {
    /// Advances what animates; whether anything did.
    pub fn tick(&mut self, draw: &mut DrawList, models: &Models, frame: &Frame) -> bool {
        self.actors.tick(draw, models, frame.delta_s as f32) | self.puck.tick(frame) | self.environment.tick(frame)
    }

    /// Begins the frame's list with the scene's own layers.
    pub fn begin(&mut self, draw: &mut DrawList, models: &Models, eye: V3) {
        draw.begin_frame();
        self.ghost.draw(draw, eye);
        self.actors.draw(draw, models, eye);
        self.puck.draw(draw, eye);
        self.debug.draw(draw, eye);
    }
}
