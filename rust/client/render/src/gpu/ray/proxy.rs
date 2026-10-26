// pxl-render/gpu/ray/proxy.rs

//! The cut a rasterized proxy would draw, built on the GPU each frame the
//! setting asks for it (pxl-shader proxy.rs, pxl-spec-core `proxy`), over
//! the cast's world as the cast binds it: from the roots about the eye a
//! zoom a dispatch, in a pass timed apart (span `proxy`). Its scratch and
//! counts are written whole each frame before it runs, so nothing in them
//! is read again; only its counts are read back, for the profiler. Nothing
//! draws the boxes.

use pxl_spec_core::proxy::{
    self, ARGS_WORDS, BOX_VERTICES, COUNTS, COUNT_COARSER, COUNT_QUEUE, DATA_WORDS, LEVELS,
    NODE_WORDS, ROOTS,
};

use crate::frame::shader::record::{compute, SHADER_PROXY, SHADER_PROXY_ARGS};
use crate::gpu::api::{buffer_entry, buffer_usage, bytes, release, word_bytes, Gpu, Stamp, DONE};
use crate::gpu::pipeline::{Built, Pipelines};
use crate::gpu::span_spec::GpuSpanName;
use xpute_guest::mem::off_of;

/// What the profiler is handed of the cut (`ProxyBuffers::sums`): the
/// frames whose counts were read back, then the boxes, the nodes asked, the
/// nodes or boxes dropped past the scratch's room, and the frames whose eye
/// was not in the open (pxl-spec-core `proxy::aired`), summed over them.
pub const PROXY_SUMS: usize = 5;

/// The cut's pipelines, made once; its buffers, made where the setting
/// first asks for the cut; its groups, over the cast's world (`bound`: the
/// eye, the windows, the ground, the service area, the overwrites and the
/// ways' pool), made again exactly when one of them is another. The counts
/// are copied into `read` where no mapping of it is out (`copied`), mapped
/// back into `landed` (`mapped`), and summed as they land.
pub(super) struct ProxyBuffers {
    built: Built,
    args_built: Built,
    counts: u32,
    data: u32,
    args: u32,
    read: u32,
    group: u32,
    args_group: u32,
    bound: [u32; 6],
    landed: Box<[u32; COUNTS as usize]>,
    mapping: Box<u32>,
    copied: bool,
    mapped: bool,
    sums: [f64; PROXY_SUMS],
}

impl ProxyBuffers {
    pub(super) fn new(g: &Gpu, pipelines: &mut Pipelines) -> ProxyBuffers {
        ProxyBuffers {
            built: pipelines.build_compute(g, SHADER_PROXY, &compute::PROXY_GROUP),
            args_built: pipelines.build_compute(g, SHADER_PROXY_ARGS, &compute::PROXY_ARGS_GROUP),
            counts: 0,
            data: 0,
            args: 0,
            read: 0,
            group: 0,
            args_group: 0,
            bound: [0; 6],
            landed: Box::new([0; COUNTS as usize]),
            mapping: Box::new(DONE),
            copied: false,
            mapped: false,
            sums: [0.0; PROXY_SUMS],
        }
    }

    /// The cut at `coarser` (pxl-spec-core `proxy::cut`) built over the
    /// cast's world `bound`, into `encoder`: its counts, its first queue
    /// (the roots, `proxy::root`) and its first dispatch written, then a
    /// zoom's dispatch and the next one's args a level, `LEVELS` of them,
    /// in one pass; its counts copied for the profiler where none are out.
    pub(super) fn dispatch(
        &mut self,
        g: &Gpu,
        encoder: u32,
        stamps: &mut [Stamp],
        coarser: u32,
        bound: [u32; 6],
    ) {
        if self.counts == 0 {
            let storage = buffer_usage::STORAGE | buffer_usage::COPY_DST;
            self.counts = g.buffer(
                word_bytes(COUNTS as usize),
                storage | buffer_usage::COPY_SRC,
            );
            self.data = g.buffer(word_bytes(DATA_WORDS as usize), storage);
            self.args = g.buffer(
                word_bytes(ARGS_WORDS as usize),
                storage | buffer_usage::INDIRECT,
            );
            self.read = g.buffer(
                word_bytes(COUNTS as usize),
                buffer_usage::MAP_READ | buffer_usage::COPY_DST,
            );
        }
        if bound != self.bound {
            release(self.group);
            release(self.args_group);
            let [eye, window, ground, shards, overwrites, pool] = bound;
            self.group = g.bind_group(
                self.built.layouts[0],
                &[
                    buffer_entry(0, eye),
                    buffer_entry(1, window),
                    buffer_entry(2, ground),
                    buffer_entry(5, shards),
                    buffer_entry(6, overwrites),
                    buffer_entry(7, pool),
                    buffer_entry(8, self.counts),
                    buffer_entry(9, self.data),
                ],
            );
            self.args_group = g.bind_group(
                self.args_built.layouts[0],
                &[buffer_entry(8, self.counts), buffer_entry(10, self.args)],
            );
            self.bound = bound;
        }
        let mut counts = [0u32; COUNTS as usize];
        counts[COUNT_QUEUE as usize] = ROOTS;
        counts[COUNT_COARSER as usize] = coarser;
        g.write_buffer(self.counts, 0, bytes(&counts));
        let mut roots = [0u32; (ROOTS * NODE_WORDS) as usize];
        for k in 0..ROOTS {
            let at = (k * NODE_WORDS) as usize;
            roots[at..at + NODE_WORDS as usize]
                .copy_from_slice(&proxy::node_words(&proxy::root(k)));
        }
        g.write_buffer(self.data, 0, bytes(&roots));
        let mut args = [0u32; ARGS_WORDS as usize];
        args[..3].copy_from_slice(&[1, 1, 1]);
        args[proxy::ARGS_DRAW as usize] = BOX_VERTICES;
        g.write_buffer(self.args, 0, bytes(&args));
        let pass = g.compute_pass(encoder, stamps[GpuSpanName::proxy as usize].pass());
        for _ in 0..LEVELS {
            g.set_pipeline(pass, self.built.pipeline);
            g.set_bind_group(pass, 0, self.group, &[]);
            g.dispatch_indirect(pass, self.args, 0);
            g.set_pipeline(pass, self.args_built.pipeline);
            g.set_bind_group(pass, 0, self.args_group, &[]);
            g.dispatch(pass, 1, 1, 1);
        }
        g.end(pass);
        if !self.copied && !self.mapped {
            let size = word_bytes(COUNTS as usize);
            g.copy_buffer(encoder, (self.counts, 0), (self.read, 0), size);
            self.copied = true;
        }
    }

    /// The counts copied last frame mapped back, and those that landed
    /// summed: after the frame that copied them was submitted.
    pub(super) fn back(&mut self, g: &Gpu) {
        if self.mapped && *self.mapping & DONE != 0 {
            let l = &self.landed;
            self.sums[0] += 1.0;
            self.sums[1] += f64::from(l[proxy::COUNT_BOXES as usize]);
            self.sums[2] += f64::from(l[proxy::COUNT_NODES as usize]);
            self.sums[3] += f64::from(l[proxy::COUNT_DROPPED as usize]);
            self.sums[4] += f64::from(u32::from(l[proxy::COUNT_SHUT as usize] != 0));
            self.mapped = false;
        }
        if self.copied && *self.mapping & DONE != 0 {
            let address = off_of(self.landed.as_ptr()) as u32;
            let size = word_bytes(COUNTS as usize);
            g.map_read(self.read, 0, size, address, &mut self.mapping, 1);
            (self.copied, self.mapped) = (false, true);
        }
    }

    /// Its sums since boot (`PROXY_SUMS`).
    pub(super) fn sums(&self) -> [f64; PROXY_SUMS] {
        self.sums
    }

    /// The frame its copy was encoded in was dropped.
    pub(super) fn dropped(&mut self) {
        self.copied = false;
    }
}
