// pxl-render/gpu/ray/proxy.rs

//! The cut a rasterized proxy draws, built on the GPU each frame the setting
//! asks for it (pxl-shader proxy.rs, pxl-spec-core `proxy`), over the cast's
//! world as the cast binds it: from the seeds over the eye's coarsest window
//! a zoom a dispatch, in a pass timed apart (span `proxy`); and, where the
//! setting asks for that too, its boxes drawn at the eye's rays' pixels
//! (proxy_raster.wgsl), the nearest face a pixel's middle meets kept with
//! its box, in a pass timed apart (span `raster`). Its scratch, counts,
//! args and targets are written whole each frame before they are read, so
//! nothing in them outlives the frame; only its counts are read back, for
//! the profiler.

use pxl_spec_core::eye;
use pxl_spec_core::proxy::{
    self, ARGS_DRAW, ARGS_WORDS, BIAS, BOX_VERTICES, COUNTS, COUNT_COARSER, COUNT_COVER,
    COUNT_PIXEL, COUNT_QUEUE, DATA_WORDS, LEVELS, NEAR, NODE_WORDS, SEEDS,
};
use pxl_spec_core::window::{EYE_WINDOWS, ORIGIN_AT, SLOTS};

use crate::frame::shader::record::{
    compute, source, PIPELINE_PROXY_RASTER, SHADER_PROXY, SHADER_PROXY_ARGS, SHADER_PROXY_RASTER,
};
use crate::gpu::api::{
    buffer_entry, buffer_usage, bytes, pass_desc, release, texture_usage, word_bytes, Format, Gpu,
    Load, Stamp, DONE,
};
use crate::gpu::pipeline::{Built, Pipelines};
use crate::gpu::renderer::Cast;
use crate::gpu::span_spec::GpuSpanName;
use xpute_guest::mem::off_of;

/// What the profiler is handed of the cut (`ProxyBuffers::sums`): the
/// frames whose counts were read back, then the boxes, the nodes asked, the
/// nodes or boxes dropped past the scratch's room, the frames whose eye was
/// not in the open (pxl-spec-core `proxy::aired`), the boxes dropped of
/// those dropped, the boxes whose floor was not known (`proxy::floor`), and
/// the squares the eye's windows held present in the frames counted, summed
/// over them.
pub const PROXY_SUMS: usize = 8;

/// Where `Cast::proxy` asks for the raster beside the cut, over its
/// `coarser` in the bits under it.
pub const PROXY_RASTER: u32 = 1 << 8;

/// What a frame hands the cut, off the frame's cast and the drawn pixels:
/// its `coarser`, none for 0, and whether it is drawn; the eye's Q3, its
/// right, up and forward and the tangents of the screen's half-angles
/// (pxl-spec-core `eye`); the eye's coarsest window's first square and
/// whether it is laid out (`window::ORIGIN_AT`); the drawn pixels.
#[derive(Clone, Copy)]
pub struct ProxyFrame {
    pub coarser: u32,
    pub raster: bool,
    origin: [i32; 3],
    axes: [[f32; 3]; 3],
    tan: [f32; 2],
    /// How far the eye's rays reach.
    reach: f32,
    first: [i32; 2],
    laid: bool,
    pub size: [u32; 2],
    /// The squares the eye's windows hold present (`window::Slot`).
    present: u32,
}

impl ProxyFrame {
    pub fn of(cast: &Cast, size: [u32; 2]) -> ProxyFrame {
        let e = &cast.eye;
        let v = |at: usize| [e[at], e[at + 1], e[at + 2]].map(f32::from_bits);
        let row = 4 * ORIGIN_AT as usize;
        let w = |k: usize| cast.window.get(row + k).copied().unwrap_or(0);
        ProxyFrame {
            coarser: cast.proxy & (PROXY_RASTER - 1),
            raster: cast.proxy & PROXY_RASTER != 0,
            origin: [e[eye::ORIGIN], e[eye::ORIGIN + 1], e[eye::ORIGIN + 2]].map(|w| w as i32),
            axes: [v(eye::RIGHT), v(eye::UP), v(eye::FORWARD)],
            tan: [f32::from_bits(e[eye::TAN]), f32::from_bits(e[eye::TAN + 1])],
            reach: f32::from_bits(e[eye::FORWARD + 3]),
            first: [w(0), w(1)],
            laid: w(2) != 0,
            size,
            present: (0..(EYE_WINDOWS * SLOTS) as usize)
                .filter(|&r| cast.window.get(4 * r + 2).is_some_and(|&p| p != 0))
                .count() as u32,
        }
    }

    /// The angle a drawn pixel spans, the wider way.
    fn pixel(&self) -> f32 {
        let [w, h] = self.size.map(|v| v.max(1) as f32);
        (2.0 * self.tan[0] / w).max(2.0 * self.tan[1] / h)
    }
}

/// The cut's pipelines, made where the setting first asks for the cut, a
/// pipeline a turn, before the frame and only where the turn's stream has
/// room for its module (`made`): its modules are large, and a turn that
/// makes them past the stream's room is lost whole. Its buffers, made where
/// the setting first asks for the cut, and its targets where it first asks
/// for the raster at a size; its groups, over the cast's world (`bound`: the eye,
/// the windows, the ground, the service area, the overwrites and the ways'
/// pool), made again exactly when one of them is another. The counts are
/// copied into `read` where no mapping of it is out (`copied`), mapped back
/// into `landed` (`mapped`), and summed as they land. `ran` is whether this
/// frame built the cut, `rastered` whether it drew it.
pub(super) struct ProxyBuffers {
    built: Built,
    args_built: Built,
    raster_built: Built,
    counts: u32,
    data: u32,
    args: u32,
    read: u32,
    uniform: u32,
    group: u32,
    args_group: u32,
    raster_group: u32,
    bound: [u32; 6],
    /// The raster's targets, each at the drawn pixels' size: a pixel's box
    /// plus one and its face's reversed depth as f32 bits (rg32uint, read by
    /// `proxy::bound`; a 2D view and a one-layer array's), and the depth it
    /// is tested by (depth32float), and their views.
    index: u32,
    index_view: u32,
    index_array: u32,
    depth: u32,
    depth_view: u32,
    target_size: [u32; 2],
    landed: Box<[u32; COUNTS as usize]>,
    mapping: Box<u32>,
    copied: bool,
    mapped: bool,
    /// The squares present in the frame whose counts were copied.
    copied_present: u32,
    sums: [f64; PROXY_SUMS],
    pub(super) ran: bool,
    pub(super) rastered: bool,
}

impl ProxyBuffers {
    pub(super) fn new() -> ProxyBuffers {
        ProxyBuffers {
            built: Built::default(),
            args_built: Built::default(),
            raster_built: Built::default(),
            counts: 0,
            data: 0,
            args: 0,
            read: 0,
            uniform: 0,
            group: 0,
            args_group: 0,
            raster_group: 0,
            bound: [0; 6],
            index: 0,
            index_view: 0,
            index_array: 0,
            depth: 0,
            depth_view: 0,
            target_size: [0; 2],
            landed: Box::new([0; COUNTS as usize]),
            mapping: Box::new(DONE),
            copied: false,
            mapped: false,
            copied_present: 0,
            sums: [0.0; PROXY_SUMS],
            ran: false,
            rastered: false,
        }
    }

    /// Its pipelines made, as many as the turn's stream has room for, the
    /// next one each turn; whether all of them are. Before the frame, as its
    /// transaction holds no room for a module.
    pub(super) fn made(&mut self, g: &Gpu, pipelines: &mut Pipelines) -> bool {
        // A module's record: its source, and a page for its words.
        let room = |kind: u32| crate::gpu::api::room(source(kind).len() + 4096);
        if self.built.pipeline == 0 && room(SHADER_PROXY) {
            self.built = pipelines.build_compute(g, SHADER_PROXY, &compute::PROXY_GROUP);
        } else if self.args_built.pipeline == 0 && room(SHADER_PROXY_ARGS) {
            self.args_built =
                pipelines.build_compute(g, SHADER_PROXY_ARGS, &compute::PROXY_ARGS_GROUP);
        } else if self.raster_built.pipeline == 0 && room(SHADER_PROXY_RASTER) {
            self.raster_built = pipelines.build(g, PIPELINE_PROXY_RASTER, Format::rg32uint);
        }
        self.built.pipeline != 0 && self.args_built.pipeline != 0 && self.raster_built.pipeline != 0
    }

    /// The cut at the frame's `coarser` (pxl-spec-core `proxy::cut`) built
    /// over the cast's world `bound`, into `encoder`: its counts and header,
    /// its first queue (the seeds, `proxy::seed`) and its first dispatch
    /// written, then a zoom's dispatch and the next one's args a level,
    /// `LEVELS` of them, in one pass; then, where the frame asks, its boxes
    /// drawn in a pass of their own; its counts copied for the profiler
    /// where none are out. Nothing where the eye's coarsest window is not
    /// laid out.
    pub(super) fn dispatch(
        &mut self,
        g: &Gpu,
        encoder: u32,
        stamps: &mut [Stamp],
        frame: &ProxyFrame,
        bound: [u32; 6],
    ) {
        (self.ran, self.rastered) = (false, false);
        if !frame.laid {
            return;
        }
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
            self.uniform = g.buffer(
                word_bytes(16),
                buffer_usage::UNIFORM | buffer_usage::COPY_DST,
            );
            self.raster_group = g.bind_group(
                self.raster_built.layouts[0],
                &[
                    buffer_entry(0, self.uniform),
                    buffer_entry(1, self.data),
                    buffer_entry(2, self.counts),
                ],
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
        let o = frame.origin;
        // The seeds the eye may see, culled here as the GPU culls every node
        // after them (`proxy::visible`): the costliest it asks.
        let mut seeds = Vec::with_capacity((SEEDS * NODE_WORDS) as usize);
        for k in 0..SEEDS {
            let n = proxy::seed(o, frame.first, k);
            if proxy::visible(&proxy::planes(o, &n), frame.axes, frame.tan, frame.reach) {
                seeds.extend_from_slice(&proxy::node_words(&n));
            }
        }
        let sown = (seeds.len() / NODE_WORDS as usize) as u32;
        let mut counts = [0u32; COUNTS as usize];
        counts[COUNT_QUEUE as usize] = sown;
        counts[COUNT_COARSER as usize] = frame.coarser;
        let c = proxy::coverage(o, frame.first);
        for a in 0..3 {
            counts[COUNT_COVER as usize + a] = c.lo[a].to_bits();
            counts[COUNT_COVER as usize + 3 + a] = c.hi[a].to_bits();
        }
        counts[COUNT_PIXEL as usize] = frame.pixel().to_bits();
        g.write_buffer(self.counts, 0, bytes(&counts));
        if !seeds.is_empty() {
            g.write_buffer(self.data, 0, bytes(&seeds));
        }
        let mut args = [0u32; ARGS_WORDS as usize];
        args[..3].copy_from_slice(&[sown.div_ceil(proxy::WORKGROUP), 1, 1]);
        args[ARGS_DRAW as usize] = BOX_VERTICES;
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
        self.ran = true;
        if frame.raster {
            self.raster(g, encoder, stamps, frame);
        }
        if !self.copied && !self.mapped {
            let size = word_bytes(COUNTS as usize);
            g.copy_buffer(encoder, (self.counts, 0), (self.read, 0), size);
            (self.copied, self.copied_present) = (true, frame.present);
        }
    }

    /// The boxes the cut wrote drawn at the drawn pixels (proxy_raster.wgsl),
    /// as many as its args say, their faces turned away culled, into
    /// targets of their own, made again where the size is another: a pixel
    /// keeps the nearest face its middle's ray meets, drawn `BIAS` nearer,
    /// that face's box plus one and its depth, none where it meets none.
    fn raster(&mut self, g: &Gpu, encoder: u32, stamps: &mut [Stamp], frame: &ProxyFrame) {
        if frame.size != self.target_size {
            release(self.index_view);
            release(self.index_array);
            release(self.index);
            release(self.depth_view);
            release(self.depth);
            let [w, h] = frame.size;
            let usage = texture_usage::RENDER_ATTACHMENT | texture_usage::TEXTURE_BINDING;
            self.index = g.texture(w, h, 1, Format::rg32uint, 1, usage);
            self.index_view = g.view(self.index, false, 0, 1, 0, 1);
            self.index_array = g.view(self.index, true, 0, 1, 0, 1);
            self.depth = g.texture(w, h, 1, Format::depth32float, 1, usage);
            self.depth_view = g.view(self.depth, false, 0, 1, 0, 1);
            self.target_size = frame.size;
        }
        let [r, u, f] = frame.axes;
        let flipped = 36 * u32::from(proxy::flipped(r, u, f));
        let uniform = [
            r[0],
            r[1],
            r[2],
            frame.tan[0],
            u[0],
            u[1],
            u[2],
            frame.tan[1],
            f[0],
            f[1],
            f[2],
            NEAR * (1.0 + BIAS),
            f32::from_bits(flipped),
            0.0,
            0.0,
            0.0,
        ];
        g.write_buffer(self.uniform, 0, bytes(&uniform));
        let stamp = stamps[GpuSpanName::raster as usize].pass();
        let pass = g.pass(
            encoder,
            &pass_desc(
                self.index_view,
                Load::Clear,
                self.depth_view,
                Load::Clear,
                0.0,
                stamp,
            ),
        );
        g.set_pipeline(pass, self.raster_built.pipeline);
        g.set_bind_group(pass, 0, self.raster_group, &[]);
        g.draw_indirect(pass, self.args, word_bytes(ARGS_DRAW as usize));
        g.end(pass);
        self.rastered = true;
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
            self.sums[5] += f64::from(l[proxy::COUNT_DROPPED_BOXES as usize]);
            self.sums[6] += f64::from(l[proxy::COUNT_UNFLOORED as usize]);
            self.sums[7] += f64::from(self.copied_present);
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

    /// What a reader of the cut binds (the cast's proxied mode): its counts
    /// and header (`proxy::COUNTS`), its scratch, the boxes in it from
    /// `proxy::box_at`, and the raster's target's views, a pixel's box plus
    /// one and its depth's bits, 2D and as a one-layer array, and the depth
    /// it was tested by; all none before they are made.
    pub(super) fn bindings(&self) -> ProxyBindings {
        ProxyBindings {
            counts: self.counts,
            data: self.data,
            index: self.index_view,
            index_array: self.index_array,
            depth: self.depth_view,
            ran: self.ran,
            rastered: self.rastered,
        }
    }
}

/// The cut's buffers and the raster's views for a reader (`bindings`), and
/// whether this frame built the cut and drew it.
#[derive(Clone, Copy)]
pub struct ProxyBindings {
    pub counts: u32,
    pub data: u32,
    pub index: u32,
    pub index_array: u32,
    pub depth: u32,
    pub ran: bool,
    pub rastered: bool,
}
