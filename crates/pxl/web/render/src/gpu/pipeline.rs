// pxl-render/gpu/pipeline.rs

//! The pipelines as shader.rs defines them, made on the device once
//! each: a pipeline at a format and a sample count, its shader modules, and
//! its bind group layouts — shared by their words, so every pipeline that
//! declares the same group gets the same layout and a bind group made for one
//! serves the others.

use crate::frame::shader::record::{pipeline as record_of, source, HEAD, ONE_SAMPLE, SHADERS};
use crate::gpu::api::{object, GpuApi};

/// Group layouts a pipeline has at most, and vertex lanes.
pub const GROUPS: usize = 4;
pub const LANES: usize = 8;

#[derive(Clone, Copy, Default)]
pub struct Built {
    pub pipeline: u32,
    pub layouts: [u32; GROUPS],
    /// Each vertex buffer's stride, in the record's order.
    pub strides: [u32; LANES],
    pub lanes: usize,
}

impl Built {
    /// The bytes a vertex takes across every lane.
    pub fn vertex_bytes(&self) -> u32 {
        self.strides[..self.lanes].iter().sum()
    }
}

const LAYOUTS: usize = 32;
const BUILT: usize = 48;

pub struct Pipelines {
    modules: [u32; SHADERS as usize],
    layouts: [(u64, u32); LAYOUTS],
    layout_len: usize,
    built: [(u64, Built); BUILT],
    built_len: usize,
}

impl Default for Pipelines {
    fn default() -> Self {
        Self::new()
    }
}

fn fold(key: u64, word: u32) -> u64 {
    (key ^ word as u64).wrapping_mul(0x100000001b3)
}

impl Pipelines {
    pub fn new() -> Self {
        Pipelines {
            modules: [0; SHADERS as usize],
            layouts: [(0, 0); LAYOUTS],
            layout_len: 0,
            built: [(0, Built::default()); BUILT],
            built_len: 0,
        }
    }

    fn module(&mut self, g: &GpuApi, kind: u32) -> u32 {
        let m = &mut self.modules[kind as usize];
        if *m == 0 {
            *m = object();
            (g.shader)(*m, source(kind));
        }
        *m
    }

    fn layout(&mut self, g: &GpuApi, entries: &[u32], samples: u32) -> u32 {
        let key = entries.iter().fold(fold(0xcbf29ce484222325, (samples > 1) as u32), |k, &w| fold(k, w));
        if let Some(&(_, id)) = self.layouts[..self.layout_len].iter().find(|(k, _)| *k == key) {
            return id;
        }
        assert!(self.layout_len < LAYOUTS, "more than {LAYOUTS} bind group layouts");
        let id = object();
        (g.layout)(id, entries, samples);
        self.layouts[self.layout_len] = (key, id);
        self.layout_len += 1;
        id
    }

    /// Pipeline `id` drawing into `format` at `samples` a pixel — unless its
    /// record fixes one sample.
    pub fn build(&mut self, g: &GpuApi, id: u32, format: u32, samples: u32) -> Built {
        let key = fold(fold(fold(0xcbf29ce484222325, id), format), samples);
        if let Some(&(_, b)) = self.built[..self.built_len].iter().find(|(k, _)| *k == key) {
            return b;
        }
        let words = record_of(id);
        assert!(!words.is_empty(), "no pipeline {id}");
        let count = if words[10] == ONE_SAMPLE { 1 } else { samples };
        let module = self.module(g, if count > 1 { words[1] } else { words[0] });
        let mut b = Built::default();
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
            b.layouts[group] = self.layout(g, &words[at + 1..at + 1 + 3 * n], count);
            at += 1 + 3 * n;
        }
        b.pipeline = object();
        (g.pipeline)(b.pipeline, &words, &b.layouts[..groups], module, format, count);
        assert!(self.built_len < BUILT, "more than {BUILT} pipelines");
        self.built[self.built_len] = (key, b);
        self.built_len += 1;
        b
    }
}
