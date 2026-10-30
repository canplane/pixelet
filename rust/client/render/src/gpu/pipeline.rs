// pxl-render/gpu/pipeline.rs

//! Pipelines made once each. Bind group layouts are shared by their words, so a
//! bind group made for one pipeline serves every one declaring the same group.

use crate::frame::shader::record::{
    compute_entry, pipeline as record_of, src, HEAD, PIPELINES, SHADERS,
};
use crate::gpu::api::{Format, Gpu, Stencil};
use crate::gpu::surface::WORLD_COLOR;

pub const GROUPS: usize = 4;
pub const LANES: usize = 8;

#[derive(Clone, Copy, Default)]
pub struct Built {
    pub pipeline: u32,
    pub layouts: [u32; GROUPS],
    pub strides: [u32; LANES],
    pub lanes: usize,
}

/// The most distinct group layouts there are: every pipeline's groups, and
/// a compute shader's one.
const LAYOUTS: usize = PIPELINES as usize * GROUPS + SHADERS as usize;
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

    fn module(&mut self, g: &Gpu, kind: u32) -> u32 {
        let m = &mut self.modules[kind as usize];
        if *m == 0 {
            *m = g.shader(src(kind));
        }
        *m
    }

    fn layout(&mut self, g: &Gpu, entries: &[u32]) -> u32 {
        let key = entries.iter().fold(0xcbf29ce484222325, |k, &w| fold(k, w));
        if let Some(&(_, id)) = self.layouts[..self.layout_len]
            .iter()
            .find(|(k, _)| *k == key)
        {
            return id;
        }
        xpute_kit::ensure!(self.layout_len < LAYOUTS, ENOSPC, LAYOUTS);
        let id = g.layout(entries);
        self.layouts[self.layout_len] = (key, id);
        self.layout_len += 1;
        id
    }

    /// Pipeline `id` drawing into `format`, depth32 its depth.
    pub fn build(&mut self, g: &Gpu, id: u32, format: Format) -> Built {
        self.build_onto(g, id, format, (Format::depth32float, Stencil::Unused))
    }

    /// Pipeline `id` drawing into a world surface's color and depth
    /// (`WORLD_COLOR`, `Gpu::gated_depth`), through the gate where the depth
    /// carries one (`Stencil::Gated`).
    pub fn build_world(&mut self, g: &Gpu, id: u32) -> Built {
        self.build_gated(g, id, WORLD_COLOR, Stencil::Gated)
    }

    /// The gate itself, onto a world surface (`Stencil::Gate`).
    pub fn build_gate(&mut self, g: &Gpu, id: u32) -> Built {
        self.build_gated(g, id, WORLD_COLOR, Stencil::Gate)
    }

    /// Pipeline `id` drawing into `format` and a gated depth
    /// (`Gpu::gated_depth`), as `stencil` asks where the depth carries one:
    /// into the depth alone where `format` is depth32's (`build`).
    pub fn build_gated(&mut self, g: &Gpu, id: u32, format: Format, stencil: Stencil) -> Built {
        let stencil = if g.gates() { stencil } else { Stencil::Unused };
        self.build_onto(g, id, format, (g.gated_depth(), stencil))
    }

    fn build_onto(
        &mut self,
        g: &Gpu,
        id: u32,
        format: Format,
        (depth, stencil): (Format, Stencil),
    ) -> Built {
        let key = fold(
            fold(
                fold(fold(0xcbf29ce484222325, id), format as u32),
                depth as u32,
            ),
            stencil as u32,
        );
        if let Some(&(_, b)) = self.built[..self.built_len].iter().find(|(k, _)| *k == key) {
            return b;
        }
        let words = record_of(id);
        let module = self.module(g, words[0]);
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
            b.layouts[group] = self.layout(g, &words[at + 1..at + 1 + 3 * n]);
            at += 1 + 3 * n;
        }
        b.pipeline = g.pipeline(
            words,
            &b.layouts[..groups],
            module,
            format,
            (depth, stencil),
        );
        xpute_kit::ensure!(self.built_len < BUILT, ENOSPC, BUILT);
        self.built[self.built_len] = (key, b);
        self.built_len += 1;
        b
    }

    /// A compute pipeline over shader `kind`'s one compute entry and `group`,
    /// group 0's (binding, stages, kind) words: made each call, as its one
    /// owner makes it once.
    pub fn build_compute(&mut self, g: &Gpu, kind: u32, group: &[u32]) -> Built {
        let module = self.module(g, kind);
        let mut b = Built::default();
        b.layouts[0] = self.layout(g, group);
        b.pipeline = g.compute_pipeline(module, compute_entry(kind), &b.layouts[..1]);
        b
    }
}
