// pxl-render/ray.rs

//! The eye's rays on the GPU. The cast (ray/cast.rs) leaves one ray at each
//! drawn pixel of the world surface's `rays`, over the terrain bricks the
//! core hands (ray/ground.rs). The screen pass here draws each ray at its
//! own pixel, painted from the face it entered by paint.wgsl's `paint`. A
//! ray's words are laid out in pxl-engine ray.rs.

mod cast;
mod count;
mod ground;
mod raster;
mod schedule;

use crate::frame::shader::record::PIPELINE_SCREEN_RAY;
use crate::gpu::api::{
    buf_entry, buffer_usage, bytes, release, view_entry, word_bytes, Entry, Gpu, TrackedRenderPass,
};
use crate::gpu::pipeline::{Pipeline, PipelineCache};
use crate::gpu::span_spec::FrameKind;
use crate::paint::ground::look_words;
use cast::CastBuffer;
pub use cast::{Cast, LightRead, ObjectTable};
pub use count::{cnt_at, RayCount, CAST, LANES_AT, RAYS, RAY_CNTS};
use ground::GroundBuffer;
pub use ground::{BrickWrite, GroundInput, TerrainUpload, TerrainWrite};

/// Whether a frame of `kind` writes the pixels' rays again. A `cast` frame
/// casts them whole; a `px` frame casts them again over the samples the
/// last cast left.
fn casts_px(kind: FrameKind) -> bool {
    kind == FrameKind::cast || kind == FrameKind::px
}

pub struct RayPass {
    pipeline: Pipeline,
    /// The screen pass's bind group 0, over the lit entries and the ids in
    /// `bindings`: the world surface's rays, the cast's eye buffer, and the
    /// damaged tiles' words and uniform.
    group: u32,
    bindings: [u32; 4],
    /// The damaged tiles' words and uniform (`Resolve::tiles`), and the
    /// number of tiles on the screen. The pixels' cast casts the damaged
    /// tiles alone, and the screen pass draws a quad for every tile, an
    /// undamaged one collapsed (screen_tile.wgsl).
    tiles: ((u32, u32), u32),
    /// The entries a cell's face is painted by (`Renderer::lit_entries`).
    /// It is empty until the renderer first hands them.
    lit: Vec<Entry>,
    looks: u32,
    /// The terrain's bricks on the GPU, made the first time `hand` runs. No
    /// ray is cast before then.
    ground: Option<GroundBuffer>,
    cast: Option<CastBuffer>,
}

impl RayPass {
    pub fn new(g: &Gpu, pipelines: &mut PipelineCache) -> RayPass {
        let pipeline = pipelines.build_world(g, PIPELINE_SCREEN_RAY);
        let words = look_words();
        let looks = g.buf(
            word_bytes(4 * words.len()),
            buffer_usage::STORAGE | buffer_usage::COPY_DST,
        );
        g.write_buf(looks, 0, bytes(&words));
        RayPass {
            pipeline,
            group: 0,
            bindings: [0; 4],
            tiles: ((0, 0), 0),
            lit: Vec::new(),
            looks,
            ground: None,
            cast: None,
        }
    }

    pub fn casts_px(&self) -> bool {
        casts_px(self.kind())
    }

    /// The renderer hands the lit entries again whenever an object they name
    /// is made again or released. The group over the old entries names that
    /// object, so it is released at once; `prepare` makes it again.
    pub fn set_lit(&mut self, entries: Vec<Entry>) {
        self.lit = entries;
        release(self.group);
        (self.group, self.bindings) = (0, [0; 4]);
    }

    /// The renderer hands the frame's tiles before `prepare`, which binds
    /// them.
    pub fn set_tiles(&mut self, tiles: (u32, u32), cnt: u32) {
        self.tiles = (tiles, cnt);
    }

    /// Makes the screen pass's group again when the rays, the eye buffer or
    /// the tiles changed, or when `set_lit` released it. No group is made
    /// until the lit entries and every buffer exist.
    fn regroup(&mut self, g: &Gpu, [rays, eye]: [u32; 2]) {
        let ((changed_at, tiles), _) = self.tiles;
        let bindings = [rays, eye, changed_at, tiles];
        if bindings == self.bindings || self.lit.is_empty() || bindings.contains(&0) {
            return;
        }
        release(self.group);
        let mut e = self.lit.clone();
        e.push(view_entry(3, rays));
        e.push(buf_entry(20, self.looks));
        e.push(buf_entry(21, eye));
        e.push(buf_entry(30, changed_at));
        e.push(buf_entry(31, tiles));
        self.group = g.bind_group(self.pipeline.layouts[0], &e);
        self.bindings = bindings;
    }

    pub fn draw(&self, pass: &mut TrackedRenderPass) {
        if self.kind() == FrameKind::plain || self.group == 0 {
            return;
        }
        pass.set_pipeline(self.pipeline.id);
        pass.set_bind_group(0, self.group, &[]);
        pass.draw(6, self.tiles.1, 0, 0);
    }
}
