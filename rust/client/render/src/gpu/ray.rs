// pxl-render/gpu/ray.rs

//! The eye's rays, each drawn at its own pixel where it met a face, painted
//! by paint.wgsl's `paint` (pxl-spec-core ray.rs), from the world surface's
//! `rays` the GPU's cast leaves (gpu/renderer.rs `Cast`): the ground it
//! casts over (ray/ground.rs) and the cast (ray/cast.rs).

mod cast;
mod ground;
mod proxy;

use crate::frame::shader::record::{PIPELINE_SCREEN_PREPASS, PIPELINE_SCREEN_RAY};
use crate::gpu::api::Stamp;
use crate::gpu::api::{
    buffer_entry, buffer_usage, bytes, release, view_entry, word_bytes, Entry, Format, Gpu,
};
use crate::gpu::pipeline::{Built, Pipelines};
use crate::gpu::span_spec::FrameKind;
use crate::mesh::paint::ground::look_words;
use cast::CastBuffers;
pub use cast::ORACLE_COUNTS;
use ground::GroundBuffers;
use proxy::ProxyBuffers;
pub use proxy::{ProxyBindings, ProxyFrame, PROXY_RASTER, PROXY_SUMS};

pub struct RaysPass {
    built: Built,
    /// The same pixels' depth alone, into the occlusion's prepass.
    depth: Built,
    /// Its group 0 over the lit pass's entries and what it binds besides
    /// (`bound`, the surface's rays and the cast's eye): made again exactly
    /// when one of them is another.
    group: u32,
    bound: [u32; 2],
    /// The lit pass's entries it paints by (`Renderer::lit_entries`); none
    /// until the renderer hands them.
    lit: Vec<Entry>,
    looks: u32,
    ground: Option<GroundBuffers>,
    cast: Option<CastBuffers>,
    /// The cut a proxy would draw, built over the cast's world where the
    /// setting asks for it (ray/proxy.rs).
    proxy: ProxyBuffers,
    /// What this frame does with the cast, as `prepare` decided it: nothing
    /// cast (`plain`), the rays standing (`standing`), or a cast run
    /// (`cast`).
    pub kind: FrameKind,
}

impl RaysPass {
    pub fn new(g: &Gpu, pipelines: &mut Pipelines) -> RaysPass {
        let built = pipelines.build(g, PIPELINE_SCREEN_RAY, g.canvas());
        let depth = pipelines.build(g, PIPELINE_SCREEN_PREPASS, Format::depth32float);
        let words = look_words();
        let looks = g.buffer(
            word_bytes(4 * words.len()),
            buffer_usage::STORAGE | buffer_usage::COPY_DST,
        );
        g.write_buffer(looks, 0, bytes(&words));
        RaysPass {
            built,
            depth,
            group: 0,
            bound: [0; 2],
            lit: Vec::new(),
            looks,
            ground: None,
            cast: None,
            proxy: ProxyBuffers::new(),
            kind: FrameKind::plain,
        }
    }

    /// The cut's pipelines made where `frame` asks for the cut, one a turn
    /// where the stream has room (`ProxyBuffers::made`), before the frame;
    /// whether they all are.
    pub fn proxy_made(
        &mut self,
        g: &Gpu,
        pipelines: &mut Pipelines,
        frame: Option<ProxyFrame>,
    ) -> bool {
        frame.is_some_and(|f| f.coarser != 0) && self.proxy.made(g, pipelines)
    }

    /// The cut a proxy draws as `frame` asks (pxl-spec-core `proxy::cut`),
    /// none at a `coarser` of 0 or before its pipelines are made
    /// (`proxy_made`), built into `encoder` over the world the cast binds
    /// once the cast has bound it, and drawn where it asks.
    pub fn proxy(
        &mut self,
        g: &Gpu,
        encoder: u32,
        stamps: &mut [Stamp],
        frame: Option<ProxyFrame>,
        made: bool,
    ) {
        (self.proxy.ran, self.proxy.rastered) = (false, false);
        let Some(bound) = self.cast.as_ref().and_then(CastBuffers::world) else {
            return;
        };
        if let Some(f) = frame.filter(|f| made && f.coarser != 0) {
            self.proxy.dispatch(g, encoder, stamps, &f, bound);
        }
    }

    /// What a reader of the cut binds, and whether this frame built and
    /// drew it (`ProxyBindings`).
    pub fn proxy_bindings(&self) -> ProxyBindings {
        self.proxy.bindings()
    }

    /// The cut's counts copied last frame mapped back (`ProxyBuffers::back`).
    pub fn proxy_back(&mut self, g: &Gpu) {
        self.proxy.back(g);
    }

    /// The cut's sums since boot (`PROXY_SUMS`).
    pub fn proxy_sums(&self) -> [f64; PROXY_SUMS] {
        self.proxy.sums()
    }

    /// The frame the cut's copy was encoded in was dropped.
    pub fn proxy_dropped(&mut self) {
        self.proxy.dropped();
    }

    /// The lit pass's entries, handed again whenever one of its objects is
    /// made again: the group is made again over them when next drawn.
    pub fn lit(&mut self, entries: Vec<Entry>) {
        self.lit = entries;
        self.bound = [0; 2];
    }

    /// The group over `bound`, the surface's rays and the cast's eye, made
    /// again where either is another or the lit pass's entries were handed.
    fn regroup(&mut self, g: &Gpu, bound: [u32; 2]) {
        if bound == self.bound || self.lit.is_empty() {
            return;
        }
        release(self.group);
        let mut e = self.lit.clone();
        e.push(view_entry(3, bound[0]));
        e.push(buffer_entry(20, self.looks));
        e.push(buffer_entry(21, bound[1]));
        self.group = g.bind_group(self.built.layouts[0], &e);
        self.bound = bound;
    }

    /// Returns the group 0 it leaves bound: none where no eye casts.
    pub fn draw(&self, g: &Gpu, pass: u32) -> u32 {
        self.draw_by(g, pass, &self.built)
    }

    /// Its depth alone, the occlusion's prepass's (gpu/ao.rs).
    pub fn draw_depth(&self, g: &Gpu, pass: u32) -> u32 {
        self.draw_by(g, pass, &self.depth)
    }

    fn draw_by(&self, g: &Gpu, pass: u32, built: &Built) -> u32 {
        if self.kind == FrameKind::plain || self.group == 0 {
            return 0;
        }
        g.set_pipeline(pass, built.pipeline);
        g.set_bind_group(pass, 0, self.group, &[]);
        g.draw(pass, 3, 1, 0, 0);
        self.group
    }
}
