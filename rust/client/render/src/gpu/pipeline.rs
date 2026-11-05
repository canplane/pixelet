// pxl-render/gpu/pipeline.rs

//! Pipelines made once each, before the renderer draws. Bind group layouts
//! are shared by their words, so a bind group made for one pipeline serves
//! every one declaring the same group.
//!
//! The renderer is made in three steps (gpu/renderer.rs `render_frame`):
//! every shader module, a few a turn (`make_modules`); then the renderer,
//! each of its pipelines made by WebGPU's async call, its ticket's word kept
//! here; then no frame until every word says it is made (`made`). A
//! pipeline asked for once the renderer draws is one found made, or a bug.

use crate::api_spec::stream;
use crate::frame::shader::record::{
    compute, pipeline as record_of, src, COMPUTES, HEAD, PIPELINES, SHADERS,
};
use crate::gpu::api::{room, shader_bytes, signaled, Format, Gpu, Layout, Stencil, GATED_DEPTH};
use crate::gpu::surface::WORLD_COLOR;

pub const GROUPS: usize = 4;
pub const LANES: usize = 8;

#[derive(Clone, Copy, Default)]
pub struct Pipeline {
    pub id: u32,
    pub layouts: [Layout; GROUPS],
    pub strides: [u32; LANES],
    pub lanes: usize,
}

/// The most distinct group layouts there are: every render pipeline's groups,
/// and a compute pipeline's one.
const LAYOUTS: usize = PIPELINES as usize * GROUPS + COMPUTES as usize;
/// Every pipeline the renderer makes, render and compute, each held to it
/// as it is made.
const PIPELINE_CAP: usize = 96;

pub struct Pipelines {
    modules: [u32; SHADERS as usize],
    /// The shaders whose module is made, the first `modules_made` by kind.
    modules_made: u32,
    layouts: [(u64, Layout); LAYOUTS],
    layout_len: usize,
    pipelines: [(u64, Pipeline); PIPELINE_CAP],
    pipeline_len: usize,
    /// Each pipeline's fence, by its place in `pipelines`, where the
    /// host stores `TICKET` once it is made: boxed, so it stays where the
    /// records named it. Let go with the renderer, on a lost device alone,
    /// whose host writes nothing more.
    words: Box<[u32; PIPELINE_CAP]>,
    /// How many of the pipelines are made, in whatever order WebGPU made
    /// them, as `made` last counted.
    made_cnt: usize,
}

impl Default for Pipelines {
    fn default() -> Self {
        Self::new()
    }
}

/// Each pipeline's ticket: its fence's first, as each word is asked
/// once.
const TICKET: u32 = 1;

fn fold(key: u64, word: u32) -> u64 {
    (key ^ word as u64).wrapping_mul(0x100000001b3)
}

impl Pipelines {
    pub fn new() -> Self {
        Pipelines {
            modules: [0; SHADERS as usize],
            modules_made: 0,
            layouts: [(0, Layout::default()); LAYOUTS],
            layout_len: 0,
            pipelines: [(0, Pipeline::default()); PIPELINE_CAP],
            pipeline_len: 0,
            words: Box::new([0; PIPELINE_CAP]),
            made_cnt: 0,
        }
    }

    /// The next shaders' modules, by kind: one, then more while their
    /// sources stay within `stream::MODULE_BYTES`, each only where the stream
    /// keeps `stream::RESERVE` past it, as an upload waits (`room`). Whether
    /// every module is made.
    pub fn make_modules(&mut self, g: &Gpu) -> bool {
        let mut turn_bytes = None;
        while self.modules_made < SHADERS {
            let code = src(self.modules_made);
            let past =
                turn_bytes.is_some_and(|b: usize| b + code.len() > stream::MODULE_BYTES as usize);
            if past || !room(shader_bytes(code)) {
                break;
            }
            self.modules[self.modules_made as usize] = g.shader(code);
            self.modules_made += 1;
            turn_bytes = Some(turn_bytes.unwrap_or(0) + code.len());
        }
        self.modules_made == SHADERS
    }

    fn module(&self, kind: u32) -> u32 {
        let m = self.modules[kind as usize];
        xpute_kit::ensure!(m != 0, ENOENT, kind);
        m
    }

    fn layout(&mut self, g: &Gpu, group: &'static [u32]) -> Layout {
        let key = group.iter().fold(0xcbf29ce484222325, |k, &w| fold(k, w));
        if let Some(&(_, layout)) = self.layouts[..self.layout_len]
            .iter()
            .find(|(k, _)| *k == key)
        {
            return layout;
        }
        xpute_kit::ensure!(self.layout_len < LAYOUTS, ENOSPC, LAYOUTS);
        let layout = g.layout(group);
        self.layouts[self.layout_len] = (key, layout);
        self.layout_len += 1;
        layout
    }

    /// The pipeline made under `key`, where one is.
    fn existing(&self, key: u64) -> Option<Pipeline> {
        self.pipelines[..self.pipeline_len]
            .iter()
            .find(|(k, _)| *k == key)
            .map(|&(_, b)| b)
    }

    /// The next pipeline's place, its fence and its ticket: none once
    /// the renderer draws, as every pipeline it draws with is made before.
    fn next(&self) -> (usize, &u32, u32) {
        let at = self.pipeline_len;
        xpute_kit::ensure!(!self.is_made(), EBUSY, at);
        xpute_kit::ensure!(at < PIPELINE_CAP, ENOSPC, PIPELINE_CAP);
        (at, &self.words[at], TICKET)
    }

    fn keep(&mut self, at: usize, key: u64, b: Pipeline) -> Pipeline {
        self.pipelines[at] = (key, b);
        self.pipeline_len = at + 1;
        b
    }

    /// Pipeline `id` drawing into `format`, depth32 its depth.
    pub fn build(&mut self, g: &Gpu, id: u32, format: Format) -> Pipeline {
        self.build_onto(g, id, format, (Format::depth32float, Stencil::Unused))
    }

    /// Pipeline `id` drawing into a world surface's color and depth
    /// (`WORLD_COLOR`, `GATED_DEPTH`), through the gate
    /// (`Stencil::Gated`).
    pub fn build_world(&mut self, g: &Gpu, id: u32) -> Pipeline {
        self.build_gated(g, id, WORLD_COLOR, Stencil::Gated)
    }

    /// The gate itself, onto a world surface (`Stencil::Gate`).
    pub fn build_gate(&mut self, g: &Gpu, id: u32) -> Pipeline {
        self.build_gated(g, id, WORLD_COLOR, Stencil::Gate)
    }

    /// Pipeline `id` drawing into `format` and a gated depth
    /// (`GATED_DEPTH`), as `stencil` asks: into the depth alone where
    /// `format` is depth32's (`build`).
    pub fn build_gated(&mut self, g: &Gpu, id: u32, format: Format, stencil: Stencil) -> Pipeline {
        self.build_onto(g, id, format, (GATED_DEPTH, stencil))
    }

    fn build_onto(
        &mut self,
        g: &Gpu,
        id: u32,
        format: Format,
        (depth, stencil): (Format, Stencil),
    ) -> Pipeline {
        let key = fold(
            fold(
                fold(fold(0xcbf29ce484222325, id), format as u32),
                depth as u32,
            ),
            stencil as u32,
        );
        if let Some(b) = self.existing(key) {
            return b;
        }
        let words = record_of(id);
        let module = self.module(words[0]);
        let mut b = Pipeline::default();
        let mut at = HEAD as usize;
        b.lanes = words[at] as usize;
        at += 1;
        for lane in 0..b.lanes {
            b.strides[lane] = words[at];
            at += 3 + 3 * words[at + 2] as usize;
        }
        let groups = words[at] as usize;
        at += 1;
        for group in 0..groups {
            let n = words[at] as usize;
            b.layouts[group] = self.layout(g, &words[at + 1..at + 1 + 3 * n]);
            at += 1 + 3 * n;
        }
        let (place, word, ticket) = self.next();
        b.id = g.pipeline(
            words,
            &b.layouts[..groups],
            module,
            format,
            (depth, stencil),
            (word, ticket),
        );
        self.keep(place, key, b)
    }

    /// Compute pipeline `id` (record.rs `compute`) over `group`, group 0's
    /// (binding, stages, kind) words: kept by both, as a render pipeline by
    /// its record and targets, so owners asking the same share it.
    pub fn build_compute(&mut self, g: &Gpu, id: u32, group: &'static [u32]) -> Pipeline {
        // Past every render pipeline's key's first word, a record's number.
        let key = group
            .iter()
            .fold(fold(0xcbf29ce484222325, PIPELINES + id), |k, &w| fold(k, w));
        if let Some(b) = self.existing(key) {
            return b;
        }
        let (shader, entry) = compute(id);
        let module = self.module(shader);
        let mut b = Pipeline::default();
        b.layouts[0] = self.layout(g, group);
        let (place, word, ticket) = self.next();
        b.id = g.compute_pipeline(module, entry, &b.layouts[..1], (word, ticket));
        self.keep(place, key, b)
    }

    /// Whether every pipeline asked for is made, its fence holding its ticket:
    /// the renderer draws only once it is.
    pub fn is_made(&self) -> bool {
        self.made_cnt == self.pipeline_len && self.pipeline_len != 0
    }

    /// The pipelines whose word says they are made, counted; whether all
    /// are (`is_made`).
    pub fn made(&mut self) -> bool {
        self.made_cnt = self.words[..self.pipeline_len]
            .iter()
            .filter(|w| signaled(w, TICKET))
            .count();
        self.is_made()
    }

    /// How much of what the renderer makes before it draws is made, 0 to 1:
    /// the pipelines made of those asked for, 0 before any is asked.
    pub fn share(&self) -> f64 {
        if self.pipeline_len == 0 {
            return 0.0;
        }
        self.made_cnt as f64 / self.pipeline_len as f64
    }
}

#[cfg(test)]
#[path = "pipeline.test.rs"]
mod test;
