// pxl-render/gpu/ray.rs

//! The eye's rays, each drawn at its own pixel where it met a face, painted
//! by paint.wgsl's `paint` (pxl-spec-core ray.rs), from the world surface's
//! `rays` the GPU's cast leaves (gpu/renderer.rs `Cast`): the ground it
//! casts over (ray/ground.rs) and the cast (ray/cast.rs), its samples'
//! stops handed to the rasterizer (ray/raster.rs).

mod cast;
mod ground;
mod raster;
mod tally;

use crate::frame::shader::record::PIPELINE_SCREEN_RAY;
use crate::gpu::api::{
    buf_entry, buffer_usage, bytes, release, view_entry, word_bytes, Entry, Gpu, TrackedRenderPass,
};
use crate::gpu::pipeline::{Built, Pipelines};
use crate::gpu::span_spec::FrameKind;
use crate::mesh::paint::ground::look_words;
use cast::CastBuffers;
use ground::GroundBuffers;
pub use tally::{sum_at, CASTS, FRAMES, LANES_AT, RAYS, RAY_SUMS};

/// Whether a frame of `kind` casts the pixels' rays, whole or off the
/// samples that stand: the rays, their depth and where they met the world
/// are laid again.
fn casts_px(kind: FrameKind) -> bool {
    kind == FrameKind::cast || kind == FrameKind::px
}

pub struct RaysPass {
    built: Built,
    /// Its group 0 over the lit pass's entries and what it binds besides
    /// (`bound`, the surface's rays, the cast's eye, and the tiles' words and
    /// uniform): made again exactly when one of them is another.
    group: u32,
    bound: [u32; 4],
    /// The surface's rays and the cast's eye as last handed; the tiles the
    /// pixels' cast casts, the claimed (`Resolve::tiles`); and those its
    /// screen pass draws, a quad each (screen_tiles.wgsl), the claimed or,
    /// where the world's pass has no gate, every one (`Resolve::every`):
    /// their words and uniform, and how many tiles there are.
    handed: [u32; 2],
    claimed: (u32, u32),
    tiles: ((u32, u32), u32),
    /// The lit pass's entries it paints by (`Renderer::lit_entries`); none
    /// until the renderer hands them.
    lit: Vec<Entry>,
    looks: u32,
    ground: Option<GroundBuffers>,
    cast: Option<CastBuffers>,
    /// What this frame does with the cast, as `prepare` decided it: nothing
    /// cast (`plain`), the rays standing (`standing`), the pixels cast
    /// again over the samples that stand (`px`), or a cast run
    /// (`cast`).
    pub kind: FrameKind,
}

impl RaysPass {
    pub fn new(g: &Gpu, pipelines: &mut Pipelines) -> RaysPass {
        let built = pipelines.build_world(g, PIPELINE_SCREEN_RAY);
        let words = look_words();
        let looks = g.buf(
            word_bytes(4 * words.len()),
            buffer_usage::STORAGE | buffer_usage::COPY_DST,
        );
        g.write_buf(looks, 0, bytes(&words));
        RaysPass {
            built,
            group: 0,
            bound: [0; 4],
            handed: [0; 2],
            claimed: (0, 0),
            tiles: ((0, 0), 0),
            lit: Vec::new(),
            looks,
            ground: None,
            cast: None,
            kind: FrameKind::plain,
        }
    }

    /// Whether this frame casts the pixels' rays (`casts_px`).
    pub fn casts_px(&self) -> bool {
        casts_px(self.kind)
    }

    /// The lit pass's entries, handed again whenever one of its objects is
    /// made again: the group is made again over them when next drawn.
    pub fn set_lit(&mut self, entries: Vec<Entry>) {
        self.lit = entries;
        self.bound = [0; 4];
    }

    /// The tiles this frame casts (`claimed`) and draws (`drawn`), `cnt` of
    /// them, handed before it prepares, which makes its groups again where
    /// they are others.
    pub fn set_tiles(&mut self, claimed: (u32, u32), drawn: (u32, u32), cnt: u32) {
        self.claimed = claimed;
        self.tiles = (drawn, cnt);
    }

    /// The group over the surface's rays and the cast's eye (`rays`) and
    /// the tiles, made again where one is another or the lit pass's entries
    /// were handed; none before all are.
    fn regroup(&mut self, g: &Gpu, [rays, eye]: [u32; 2]) {
        self.handed = [rays, eye];
        let ((changed_at, tiles), _) = self.tiles;
        let bound = [rays, eye, changed_at, tiles];
        if bound == self.bound || self.lit.is_empty() || bound.contains(&0) {
            return;
        }
        release(self.group);
        let mut e = self.lit.clone();
        e.push(view_entry(3, rays));
        e.push(buf_entry(20, self.looks));
        e.push(buf_entry(21, eye));
        e.push(buf_entry(30, changed_at));
        e.push(buf_entry(31, tiles));
        self.group = g.bind_group(self.built.layouts[0], &e);
        self.bound = bound;
    }

    pub fn draw(&self, pass: &mut TrackedRenderPass) {
        if self.kind == FrameKind::plain || self.group == 0 {
            return;
        }
        pass.set_pipeline(self.built.pipeline);
        pass.set_bind_group(0, self.group, &[]);
        pass.draw(6, self.tiles.1, 0, 0);
    }
}
