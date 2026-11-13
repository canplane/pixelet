// pxl-render/gpu/pipeline.rs

//! Pipelines made once each, before the renderer draws. Bind group layouts
//! are shared by their words, so a bind group made for one pipeline serves
//! every one declaring the same group.
//!
//! The renderer is made in three steps (renderer.rs `render_frame`). First every shader
//! module is made, a few a frame (`make_modules`). Then the renderer is made, and each of its
//! pipelines is made by WebGPU's async call, with the word its ticket lands in kept here. Then
//! no frame is drawn until every word says its pipeline is made (`made`). A pipeline asked for
//! once the renderer draws must be found already made; anything else is a bug.

use crate::api_spec::stream;
use crate::frame::shader::floor::{BIND_GROUPS, VERTEX_BUFFERS};
use crate::frame::shader::record::{
    compute, pipeline as rec_of, src, COMPUTES, HDR, PIPELINES, SHADERS,
};
use crate::gpu::api::{
    room, shader_bytes, signaled, Format, Gpu, Layout, Stencil, MASKED_DEPTH, WORLD_COLOR,
};

#[derive(Clone, Copy, Default)]
pub struct Pipeline {
    pub id: u32,
    pub layouts: [Layout; BIND_GROUPS],
    pub strides: [u32; VERTEX_BUFFERS],
    pub lane_cnt: usize,
}

/// The most distinct group layouts there are: every render pipeline's groups,
/// and a compute pipeline's one.
const LAYOUTS: usize = PIPELINES as usize * BIND_GROUPS + COMPUTES as usize;
/// The most pipelines the renderer makes, render and compute. Each pipeline is checked against
/// it as it is made.
const PIPELINE_CAP: usize = 118;

pub struct PipelineCache {
    modules: [u32; SHADERS as usize],
    /// How many shaders have their module made. They are the first ones in order of kind.
    modules_made: u32,
    layouts: [(u64, Layout); LAYOUTS],
    layout_len: usize,
    pipelines: [(u64, Pipeline); PIPELINE_CAP],
    pipeline_len: usize,
    /// Each pipeline's fence, by its place in `pipelines`. The host stores `TICKET` there
    /// once the pipeline is made. The words are boxed so they stay where the records named
    /// them. They are freed with the renderer, which happens only on a lost device, whose
    /// host writes nothing more.
    words: Box<[u32; PIPELINE_CAP]>,
    /// How many of the pipelines are made, in whatever order WebGPU made
    /// them, as `made` last counted.
    made_cnt: usize,
}

impl Default for PipelineCache {
    fn default() -> Self {
        Self::new()
    }
}

/// The ticket every pipeline's fence waits for. Each fence is asked once, so its first ticket
/// serves.
const TICKET: u32 = 1;

fn fold(key: u64, word: u32) -> u64 {
    (key ^ word as u64).wrapping_mul(0x100000001b3)
}

impl PipelineCache {
    pub fn new() -> Self {
        PipelineCache {
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

    /// Makes the next shaders' modules in order of kind: one, then more while their sources
    /// stay within `stream::MODULE_BYTES`. Each is made only when the stream keeps
    /// `stream::RESERVE` past it, as an upload waits for room (`room`). Returns whether every
    /// module is made.
    pub fn make_modules(&mut self, g: &Gpu) -> bool {
        let mut frame_bytes = None;
        while self.modules_made < SHADERS {
            let code = src(self.modules_made);
            let past =
                frame_bytes.is_some_and(|b: usize| b + code.len() > stream::MODULE_BYTES as usize);
            if past || !room(shader_bytes(code)) {
                break;
            }
            self.modules[self.modules_made as usize] = g.shader(code);
            self.modules_made += 1;
            frame_bytes = Some(frame_bytes.unwrap_or(0) + code.len());
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

    /// The next pipeline's place, its fence and its ticket. There is none once the renderer
    /// draws, as every pipeline it draws with is made before.
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

    /// Pipeline `id` drawing into `format`, with a depth32 depth.
    pub fn build(&mut self, g: &Gpu, id: u32, format: Format) -> Pipeline {
        self.build_targets(g, id, format, 1)
    }

    /// Pipeline `id` drawing into `target_cnt` colors of `format`, each the fragment's output at
    /// its own location, with a depth32 depth.
    pub fn build_targets(
        &mut self,
        g: &Gpu,
        id: u32,
        format: Format,
        target_cnt: usize,
    ) -> Pipeline {
        let depth = (Format::depth32float, Stencil::Unused);
        self.build_onto(g, id, (format, target_cnt), depth)
    }

    /// Pipeline `id` drawing into a world surface's color and depth
    /// (`WORLD_COLOR`, `MASKED_DEPTH`), through the gate
    /// (`Stencil::Masked`).
    pub fn build_world(&mut self, g: &Gpu, id: u32) -> Pipeline {
        self.build_masked(g, id, WORLD_COLOR, Stencil::Masked)
    }

    /// The gate itself, onto a world surface (`Stencil::Mask`).
    pub fn build_mask(&mut self, g: &Gpu, id: u32) -> Pipeline {
        self.build_masked(g, id, WORLD_COLOR, Stencil::Mask)
    }

    /// Pipeline `id` drawing into `format` and a gated depth (`MASKED_DEPTH`), as `stencil`
    /// asks. When `format` is depth32's, it draws into the depth alone (`build`).
    pub fn build_masked(&mut self, g: &Gpu, id: u32, format: Format, stencil: Stencil) -> Pipeline {
        self.build_onto(g, id, (format, 1), (MASKED_DEPTH, stencil))
    }

    fn build_onto(
        &mut self,
        g: &Gpu,
        id: u32,
        (format, targets): (Format, usize),
        (depth, stencil): (Format, Stencil),
    ) -> Pipeline {
        let key = fold(
            fold(
                fold(
                    fold(fold(0xcbf29ce484222325, id), format as u32),
                    targets as u32,
                ),
                depth as u32,
            ),
            stencil as u32,
        );
        if let Some(b) = self.existing(key) {
            return b;
        }
        let words = rec_of(id);
        let module = self.module(words[0]);
        let mut b = Pipeline::default();
        let mut at = HDR as usize;
        b.lane_cnt = words[at] as usize;
        at += 1;
        for lane in 0..b.lane_cnt {
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
            (format, targets),
            (depth, stencil),
            (word, ticket),
        );
        g.recorded(b.id, id);
        self.keep(place, key, b)
    }

    /// Compute pipeline `id` (record.rs `compute`) over `group`, group 0's (binding, stages,
    /// kind) words. It is kept under both, as a render pipeline is kept under its record and
    /// targets, so owners that ask for the same one share it.
    pub fn build_compute(&mut self, g: &Gpu, id: u32, group: &'static [u32]) -> Pipeline {
        // The key starts past every render pipeline's record number, so it never starts as a
        // render pipeline's key does.
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

    /// Counts the pipelines whose word says they are made, and returns whether all are
    /// (`is_made`).
    pub fn poll(&mut self) -> bool {
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
