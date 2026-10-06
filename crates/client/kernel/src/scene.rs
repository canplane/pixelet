// pxl-kernel/scene.rs

//! What a frame draws beside the chunks: the draw list every layer writes
//! into, and each drawn layer's own state. The chunks' table draws into the
//! same list (`world/chunk/table.rs`).

pub mod billboard;
pub mod character;
pub mod environment;
pub mod marker;
pub mod model;
pub mod stage;

use pxl_render::frame::draw::DrawList;
use pxl_spec::space::coord::v3::V3;

use crate::scene::billboard::field::Billboards;
use crate::scene::environment::state::EnvironmentDraw;
use crate::scene::marker::coverage_gizmo::CoverageGizmo;
use crate::scene::marker::debug_lines::DebugLines;
use crate::scene::marker::hover_cursor::GhostDraw;
use crate::scene::marker::location_puck::Markers;
use crate::scene::model::store::Models;
use crate::world::chunk::table::Frame;

#[derive(Default)]
pub struct Scene {
    pub draw: DrawList,
    pub ghost: GhostDraw,
    pub models: Models,
    pub markers: Markers,
    pub billboards: Billboards,
    pub environment: EnvironmentDraw,
    pub debug: DebugLines,
    pub coverage: CoverageGizmo,
}

impl Scene {
    /// Advances what animates; whether anything did.
    pub fn tick(&mut self, frame: &Frame) -> bool {
        self.models.tick(&mut self.draw, frame.delta_s as f32) | self.markers.tick(frame) | self.environment.tick(frame)
    }

    /// Begins the frame's list with every layer but the chunks.
    pub fn begin(&mut self, eye: V3) {
        self.draw.begin_frame();
        self.ghost.draw(&mut self.draw, eye);
        self.models.draw(&mut self.draw, eye);
        self.markers.draw(&mut self.draw, eye);
        self.debug.draw(&mut self.draw, eye);
        self.coverage.draw(&mut self.draw, eye);
    }
}
