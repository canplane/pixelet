// pxl-render/gpu/ray/raster.rs

//! The eye's record of voxels and the raster that draws it (pxl-engine `record`, `raster`):
//! the record, its owner here, kept across frames; after the samples, each stopped sample
//! written into it (frame/shader/sample_box.wgsl, span `sample_box`, a thread a sample of
//! the camera grid), every held voxel in view written as a box (record_draw.wgsl
//! `record_draw_cs`, same span) and the boxes drawn at the rays' pixels (raster.wgsl, span
//! `sraster`, its fragments the drawn pixels), the nearest face a pixel's ray enters kept
//! with its voxel, which the cast's pixels read (frame/shader/cast.wgsl `cast_cs`); then what
//! lies about each new face asked once a face (frame/shader/face.wgsl `face_cs`, span
//! `face`). The record is forgotten with the node cache, by its rule (`forget`). Its targets are
//! written whole each frame before they are read: in a frame not drawn
//! whole, through a gate of their own, the depth cleared to the nearest and
//! the claimed tiles set back to the far plane (raster_gate.wgsl), so the
//! boxes pass the depth test, early, there alone, and the cast's pixels,
//! which pass the other tiles by, read nothing there. Its args and boxes
//! each frame its samples are cast, and kept where only the pixels are
//! cast again over them.

use super::sequence;
use pxl_engine::{beam, eye, light, raster, record};

use crate::frame::shader::floor;
use crate::frame::shader::record::{
    binds, COMPUTE, COMPUTE_FACE, COMPUTE_LIGHT_RECORD_DEFERRED, COMPUTE_LIGHT_RECORD_DRAW,
    COMPUTE_LIGHT_RECORD_STOPS, COMPUTE_LIGHT_SAMPLE_BOX, COMPUTE_LIGHT_SAMPLE_OBSERVE,
    COMPUTE_RECORD_DEFERRED, COMPUTE_RECORD_DRAW, COMPUTE_RECORD_FACES, COMPUTE_RECORD_FORGET,
    COMPUTE_RECORD_STOPS, COMPUTE_SAMPLE_BOX, COMPUTE_SAMPLE_OBSERVE, PIPELINE_RASTER,
    PIPELINE_RASTER_GATE, STORAGE, STORAGE_RW, STORAGE_TEXTURE_RW, UNIFORM,
};
use crate::frame::shader::word::{BOX_VERTICES, RASTER_FLOATS, SQUARE_SIDE};
use crate::gpu::api::{
    buf_entry, buf_range_entry, buffer_usage, bytes, pass_desc, release, texture_usage, view_entry,
    word_bytes, Format, Gpu, Load, RenderContext,
};
use crate::gpu::pipeline::{Pipeline, Pipelines};
use crate::gpu::span_spec::GpuSpanName;
use pxl_engine::param::raster::GROWTH;

/// Where an observer's record binds, every pass that reads or writes it, the eye's or the
/// light's (frame/shader/record.wgsl).
pub(super) const RECORD_BINDING: u32 = 23;

/// The eye's samples' writing into its record (sample_box.wgsl): the eye, the samples'
/// words, the faces' asks' args and the record.
pub(super) const SAMPLE_BOX_GROUP: [u32; 12] = [
    0,
    COMPUTE,
    UNIFORM,
    8,
    COMPUTE,
    STORAGE_RW,
    16,
    COMPUTE,
    STORAGE_RW,
    RECORD_BINDING,
    COMPUTE,
    STORAGE_RW,
];
/// The record over every slot (record_draw.wgsl): its forgetting, over the node cache's
/// head, and its drawing, over the eye, the boxes and the raster's args.
pub(super) const RECORD_GROUP: [u32; 27] = [
    0,
    COMPUTE,
    UNIFORM,
    9,
    COMPUTE,
    STORAGE_RW,
    14,
    COMPUTE,
    STORAGE_RW,
    16,
    COMPUTE,
    STORAGE_RW,
    RECORD_BINDING,
    COMPUTE,
    STORAGE_RW,
    super::cast::HEAD_BINDING,
    COMPUTE,
    UNIFORM,
    12,
    COMPUTE,
    STORAGE_RW,
    21,
    COMPUTE,
    STORAGE_RW,
    22,
    COMPUTE,
    STORAGE_RW,
];
/// The holes' stops into the record (record_stops.wgsl): the eye, the rays, the stops among the
/// beam rays' words, the node cache's head their count lies in, the faces' asks' args and
/// the record.
pub(super) const STOPS_GROUP: [u32; 21] = [
    0,
    COMPUTE,
    UNIFORM,
    3,
    COMPUTE,
    STORAGE_TEXTURE_RW,
    8,
    COMPUTE,
    STORAGE_RW,
    9,
    COMPUTE,
    STORAGE,
    16,
    COMPUTE,
    STORAGE_RW,
    RECORD_BINDING,
    COMPUTE,
    STORAGE_RW,
    DEFERRED_BINDING,
    COMPUTE,
    STORAGE_RW,
];
/// The binding of the dispatch over the stops put off (record_stops.wgsl `deferred`).
pub(super) const DEFERRED_BINDING: u32 = 24;
/// The stops put off taken past the stops' dispatch's end (record_deferred.wgsl): the eye,
/// the beam rays' words their numbers and the stops lie in, the args they count and lay their
/// faces' asks in, and the record; not the dispatch they are taken by, which this pass reads
/// its count from.
pub(super) const DEFERRED_GROUP: [u32; 12] = [
    0,
    COMPUTE,
    UNIFORM,
    8,
    COMPUTE,
    STORAGE_RW,
    16,
    COMPUTE,
    STORAGE_RW,
    RECORD_BINDING,
    COMPUTE,
    STORAGE_RW,
];
/// The light's holes' stops into its record (record_stops.wgsl over record_stops.light.wgsl):
/// `STOPS_GROUP`, its map at 3 and its list at 8, and the light's head its slots' rays are
/// aimed by.
pub(super) const LIGHT_STOPS_GROUP: [u32; 24] = [
    0,
    COMPUTE,
    UNIFORM,
    3,
    COMPUTE,
    STORAGE_TEXTURE_RW,
    8,
    COMPUTE,
    STORAGE_RW,
    9,
    COMPUTE,
    STORAGE,
    super::cast::HEAD_BINDING,
    COMPUTE,
    UNIFORM,
    16,
    COMPUTE,
    STORAGE_RW,
    RECORD_BINDING,
    COMPUTE,
    STORAGE_RW,
    DEFERRED_BINDING,
    COMPUTE,
    STORAGE_RW,
];
/// The light's stops put off (record_deferred.wgsl over record_stops.light.wgsl): as
/// `DEFERRED_GROUP`, its list for the beam rays' words, and the light's head its slots' rays
/// are aimed by.
pub(super) const LIGHT_DEFERRED_GROUP: [u32; 15] = [
    0,
    COMPUTE,
    UNIFORM,
    8,
    COMPUTE,
    STORAGE_RW,
    super::cast::HEAD_BINDING,
    COMPUTE,
    UNIFORM,
    16,
    COMPUTE,
    STORAGE_RW,
    RECORD_BINDING,
    COMPUTE,
    STORAGE_RW,
];
/// The light's samples' writing into its record (sample_box.wgsl over sample.light.wgsl):
/// the eye, the samples' words, the light's head, its raster's args and its record.
pub(super) const LIGHT_SAMPLE_BOX_GROUP: [u32; 15] = [
    0,
    COMPUTE,
    UNIFORM,
    8,
    COMPUTE,
    STORAGE_RW,
    super::cast::HEAD_BINDING,
    COMPUTE,
    UNIFORM,
    16,
    COMPUTE,
    STORAGE_RW,
    RECORD_BINDING,
    COMPUTE,
    STORAGE_RW,
];
const _: () = assert!(
    floor::holds(&[&SAMPLE_BOX_GROUP])
        && floor::holds(&[&LIGHT_SAMPLE_BOX_GROUP])
        && floor::holds(&[&RECORD_GROUP])
        && floor::holds(&[&STOPS_GROUP])
        && floor::holds(&[&LIGHT_STOPS_GROUP]),
    "the boxes' writing's group binds past WebGPU's floor"
);

/// The raster's pipelines, made with the cast's; its args, the eye's record
/// (`record::WORDS`) and the eye's boxes, room for every slot of it, made with them;
/// the raster's view, a uniform; its targets at the rays'
/// size: a pixel's voxel (r32uint, drawn into as 2D and read as a
/// one-layer array, `visibility`, the visibility buffer) and the depth it is
/// tested by; and the groups of the record's writing, its forgetting and drawing, the
/// boxes' drawing and the faces' asking, the samples' and the holes' stops', made
/// again exactly when what they bind is another (`bindings`).
pub(super) struct Raster {
    /// The samples' writing into their observer's record, the eye's and the shadow map's
    /// (sample_box.wgsl), each over its own group.
    pub(super) boxes_built: Pipeline,
    /// The second of the samples' passes over the same group, the eye's and the light's
    /// (sample_box.wgsl `sample_observe_cs`).
    observe_built: Pipeline,
    /// The record over every slot: forgotten, and drawn as boxes, the eye's and the light's
    /// (record_draw.wgsl).
    forget_built: Pipeline,
    record_draw_built: Pipeline,
    light_record_draw_built: Pipeline,
    /// The missed pixels' hits into the record, and those put off (record_stops.wgsl).
    stops_built: Pipeline,
    deferred_built: Pipeline,
    /// What lies about each stop's face put at its pixel off the record (record_stops.wgsl
    /// `record_faces_cs`).
    faces_put_built: Pipeline,
    /// The light's holes' stops into its record, and those put off (record_stops.wgsl over
    /// record_stops.light.wgsl).
    light_stops_built: Pipeline,
    light_deferred_built: Pipeline,
    pub(super) light_observe_built: Pipeline,
    /// The shadow map's samples into its record (sample_box.wgsl over sample.light.wgsl),
    /// the light's head bound beside what the screen's binds.
    pub(super) light_boxes_built: Pipeline,
    faces_built: Pipeline,
    draw_built: Pipeline,
    /// The gate the boxes are drawn through in a frame not drawn whole, over
    /// the claimed tiles (`Resolve::tiles`).
    gate: Pipeline,
    pub(super) args: u32,
    /// The dispatch over the stops put off (`raster::DEFERRED_DISPATCH`), apart from the args
    /// the pass that takes them writes.
    deferred: u32,
    pub(super) record: u32,
    boxes: u32,
    uniform: u32,
    visibility: u32,
    visibility_view: u32,
    pub(super) visibility_array: u32,
    depth: u32,
    depth_view: u32,
    size: [u32; 2],
    groups: [u32; 7],
    bindings: [u32; 9],
    /// What the record's group binds past the raster's own, the light's: its head, its list,
    /// its args and its stood list's args, where its receivers stand the boxes the record's
    /// draw writes (record_draw.wgsl `record_draw_cs`; `record_group`).
    record_bound: [u32; 6],
    /// The light's, made while its shadows are on (`light_made`,
    /// `light_release`).
    pub(super) light: LightRaster,
}

/// The light's record (pxl-engine `record`), written by its samples' passes
/// (`light_boxes_built`), its boxes written off it (record_draw.wgsl `light_record_draw_cs`)
/// and drawn from the light by the same draw: the record; its boxes, room for every slot of
/// it; their args; its view, a uniform; its targets at the map's side, in the box's order;
/// the draw's group; and the record's group, made again exactly when what it binds is
/// another (`record_bound`).
#[derive(Default)]
pub(super) struct LightRaster {
    pub(super) record: u32,
    boxes: u32,
    pub(super) args: u32,
    /// The dispatch over its stops put off, as the eye's (`Raster::deferred`).
    deferred: u32,
    uniform: u32,
    visibility: u32,
    visibility_view: u32,
    pub(super) visibility_array: u32,
    depth: u32,
    depth_view: u32,
    group: u32,
    record_group: u32,
    record_bound: [u32; 7],
    /// Its holes' stops' group (`LIGHT_STOPS_GROUP`) and its stops put off's
    /// (`LIGHT_DEFERRED_GROUP`), made again with the record's.
    stops_group: u32,
    deferred_group: u32,
}

/// `face_cs`'s (face.wgsl): the eye, the world's query's buffers but the palette, which it
/// reads none of, the beam's words it asks off and leaves a hit's faces in, the args it is
/// bound to, and the record it leaves a voxel's faces in.
pub(super) const FACE_GROUP: [u32; 27] = [
    0,
    COMPUTE,
    UNIFORM,
    1,
    COMPUTE,
    STORAGE,
    2,
    COMPUTE,
    STORAGE,
    5,
    COMPUTE,
    STORAGE,
    6,
    COMPUTE,
    STORAGE,
    7,
    COMPUTE,
    STORAGE,
    8,
    COMPUTE,
    STORAGE_RW,
    16,
    COMPUTE,
    STORAGE,
    RECORD_BINDING,
    COMPUTE,
    STORAGE_RW,
];

// Each compute group here against what its shader reads, as its declarations take it
// (build.rs `BINDS`).
const _: () = assert!(
    binds(
        &[COMPUTE_SAMPLE_BOX, COMPUTE_SAMPLE_OBSERVE],
        &SAMPLE_BOX_GROUP
    ) && binds(
        &[COMPUTE_LIGHT_SAMPLE_BOX, COMPUTE_LIGHT_SAMPLE_OBSERVE],
        &LIGHT_SAMPLE_BOX_GROUP
    ) && binds(&[COMPUTE_FACE], &FACE_GROUP)
        && binds(
            &[
                COMPUTE_RECORD_FORGET,
                COMPUTE_RECORD_DRAW,
                COMPUTE_LIGHT_RECORD_DRAW
            ],
            &RECORD_GROUP
        )
        && binds(&[COMPUTE_RECORD_STOPS, COMPUTE_RECORD_FACES], &STOPS_GROUP)
        && binds(&[COMPUTE_RECORD_DEFERRED], &DEFERRED_GROUP)
        && binds(&[COMPUTE_LIGHT_RECORD_STOPS], &LIGHT_STOPS_GROUP)
        && binds(&[COMPUTE_LIGHT_RECORD_DEFERRED], &LIGHT_DEFERRED_GROUP),
    "a compute group lays out other than its shader reads (record.rs `binds`)"
);
const _: () = assert!(
    floor::holds(&[&FACE_GROUP]),
    "the faces' group binds past WebGPU's floor"
);

impl Raster {
    /// Its pipelines and its args and uniform, made where the cast's are
    /// (`RaysPass::make`).
    pub(super) fn new(g: &Gpu, pipelines: &mut Pipelines) -> Raster {
        Raster {
            boxes_built: pipelines.build_compute(g, COMPUTE_SAMPLE_BOX, &SAMPLE_BOX_GROUP),
            observe_built: pipelines.build_compute(g, COMPUTE_SAMPLE_OBSERVE, &SAMPLE_BOX_GROUP),
            light_observe_built: pipelines.build_compute(
                g,
                COMPUTE_LIGHT_SAMPLE_OBSERVE,
                &LIGHT_SAMPLE_BOX_GROUP,
            ),
            light_boxes_built: pipelines.build_compute(
                g,
                COMPUTE_LIGHT_SAMPLE_BOX,
                &LIGHT_SAMPLE_BOX_GROUP,
            ),
            forget_built: pipelines.build_compute(g, COMPUTE_RECORD_FORGET, &RECORD_GROUP),
            record_draw_built: pipelines.build_compute(g, COMPUTE_RECORD_DRAW, &RECORD_GROUP),
            light_record_draw_built: pipelines.build_compute(
                g,
                COMPUTE_LIGHT_RECORD_DRAW,
                &RECORD_GROUP,
            ),
            stops_built: pipelines.build_compute(g, COMPUTE_RECORD_STOPS, &STOPS_GROUP),
            deferred_built: pipelines.build_compute(g, COMPUTE_RECORD_DEFERRED, &DEFERRED_GROUP),
            faces_put_built: pipelines.build_compute(g, COMPUTE_RECORD_FACES, &STOPS_GROUP),
            light_stops_built: pipelines.build_compute(
                g,
                COMPUTE_LIGHT_RECORD_STOPS,
                &LIGHT_STOPS_GROUP,
            ),
            light_deferred_built: pipelines.build_compute(
                g,
                COMPUTE_LIGHT_RECORD_DEFERRED,
                &LIGHT_DEFERRED_GROUP,
            ),
            faces_built: pipelines.build_compute(g, COMPUTE_FACE, &FACE_GROUP),
            draw_built: pipelines.build(g, PIPELINE_RASTER, Format::r32uint),
            gate: pipelines.build(g, PIPELINE_RASTER_GATE, Format::r32uint),
            args: g.buf(
                word_bytes((raster::DEFERRED_AT + raster::DEFERRED_WORDS) as usize),
                buffer_usage::STORAGE
                    | buffer_usage::COPY_DST
                    | buffer_usage::INDIRECT
                    | buffer_usage::COPY_SRC,
            ),
            deferred: g.buf(
                word_bytes(raster::DEFERRED_DISPATCH.len()),
                buffer_usage::STORAGE | buffer_usage::COPY_DST | buffer_usage::INDIRECT,
            ),
            // Made zeroed: every slot free.
            record: g.buf(
                word_bytes(record::WORDS as usize),
                buffer_usage::STORAGE | buffer_usage::COPY_DST,
            ),
            boxes: g.buf(
                word_bytes((record::ENTRIES * raster::BOX_WORDS) as usize),
                buffer_usage::STORAGE | buffer_usage::COPY_DST,
            ),
            uniform: g.buf(
                word_bytes(RASTER_FLOATS),
                buffer_usage::UNIFORM | buffer_usage::COPY_DST,
            ),
            visibility: 0,
            visibility_view: 0,
            visibility_array: 0,
            depth: 0,
            depth_view: 0,
            size: [0; 2],
            groups: [0; 7],
            bindings: [0; 9],
            record_bound: [0; 6],
            light: LightRaster::default(),
        }
    }

    /// What a frame that casts needs, before its dispatches: its targets at the
    /// rays' `size`; the raster's view off the `eye`'s words; the record's frame, the node
    /// cache's `frame` (`record::FRAME`); the args
    /// cleared where the samples are cast again (`samples_cast`), else the
    /// boxes they left kept; its groups over the cast's `eye`, `beams` and
    /// `node_cache`, whose head the record is forgotten by, and, for the
    /// faces' asking, the world's query's buffers (the clip level, the tiles'
    /// frames, the service area's index, the overwrites, the ways' pool)
    /// and the rays, at whose pixels the holes' stops' faces are put (`stops`).
    pub(super) fn prepare(
        &mut self,
        g: &Gpu,
        e: &[u32],
        size: [u32; 2],
        grid: &beam::Grid,
        (samples_cast, frame): (bool, u32),
        [eye, beams, node_cache, clipmap, ground, shards, overwrites, pool, rays]: [u32; 9],
    ) {
        g.write_buf(
            self.record,
            word_bytes(record::FRAME as usize),
            bytes(&[frame]),
        );
        if size != self.size {
            for v in [
                self.visibility_view,
                self.visibility_array,
                self.visibility,
                self.depth_view,
                self.depth,
            ] {
                release(v);
            }
            let [w, h] = size;
            let usage = texture_usage::RENDER_ATTACHMENT | texture_usage::TEXTURE_BINDING;
            self.visibility = g.texture(w, h, 1, Format::r32uint, 1, usage);
            self.visibility_view = g.view(self.visibility, false, 0, 1, 0, 1);
            self.visibility_array = g.view(self.visibility, true, 0, 1, 0, 1);
            self.depth = g.texture(w, h, 1, Format::depth32float, 1, usage);
            self.depth_view = g.view(self.depth, false, 0, 1, 0, 1);
            self.size = size;
        }
        // The cast's view (frame/shader/grid.wgsl `px_tan`): its axes, the
        // screen's half-angles, the near plane, the angle a face grows by,
        // the drawn pixels and the rays' reach, each pixel's ray as the cast
        // forms it.
        let v = |at: usize| [e[at], e[at + 1], e[at + 2]].map(f32::from_bits);
        let (r, u, f) = (v(eye::RIGHT), v(eye::UP), v(eye::FORWARD));
        let tan = [e[eye::TAN], e[eye::TAN + 1]].map(f32::from_bits);
        let [w, h] = size.map(|s| s.max(1) as f32);
        let far = f32::from_bits(e[eye::FAR]);
        let px = (2.0 * tan[0] / w).max(2.0 * tan[1] / h);
        let turn = BOX_VERTICES * u32::from(raster::is_flipped(r, u, f));
        let uniform: [f32; RASTER_FLOATS] = [
            r[0],
            r[1],
            r[2],
            tan[0],
            u[0],
            u[1],
            u[2],
            tan[1],
            f[0],
            f[1],
            f[2],
            raster::NEAR,
            f32::from_bits(turn),
            0.0,
            0.0,
            0.0,
            GROWTH * px,
            w,
            h,
            far,
            f32::from_bits(e[eye::JITTER]),
            f32::from_bits(e[eye::JITTER + 1]),
            0.0,
            0.0,
        ];
        g.write_buf(self.uniform, 0, bytes(&uniform));
        if samples_cast {
            // The draw's args none, and the faces' asks' none, laid where the grid lays them
            // (pxl-engine `raster::ASKED_AT`).
            let mut args = [0u32; (raster::ASKED_AT + raster::ARGS_WORDS) as usize];
            args[..raster::ARGS_WORDS as usize].copy_from_slice(&draw_args());
            let asked = raster::ASKED_AT as usize;
            args[asked + raster::ARGS_FACES as usize + 1] = 1;
            args[asked + raster::ARGS_FACES as usize + 2] = 1;
            args[asked + raster::ARGS_ASKS as usize] = grid.asks_at;
            g.write_buf(self.args, sequence::OPEN_RASTER_ARGS.to_at(), bytes(&args));
        }
        // The faces' asks the hits' stops open none, every frame the pixels are cast, laid
        // where the grid lays them (pxl-engine `raster::STOP_ASKED_AT`).
        let mut stop_args = [0u32; raster::ARGS_WORDS as usize];
        stop_args[raster::ARGS_FACES as usize + 1] = 1;
        stop_args[raster::ARGS_FACES as usize + 2] = 1;
        stop_args[raster::ARGS_ASKS as usize] = grid.stop_asks_at;
        g.write_buf(
            self.args,
            sequence::OPEN_RASTER_STOP_ASKS.to_at(),
            bytes(&stop_args),
        );
        // No hit put off, its dispatch none, a workgroup deep and high (pxl-engine
        // `raster::DEFERRED_CNT`, `raster::DEFERRED_DISPATCH`).
        g.write_buf(
            self.args,
            sequence::OPEN_RASTER_DEFERRED_CNT.to_at(),
            bytes(&[0u32]),
        );
        g.write_buf(
            self.deferred,
            sequence::OPEN_RASTER_DEFERRED.to_at(),
            bytes(&raster::DEFERRED_DISPATCH),
        );
        let bindings = [
            eye, beams, node_cache, clipmap, ground, shards, overwrites, pool, rays,
        ];
        if bindings != self.bindings {
            for group in self.groups {
                release(group);
            }
            // The faces' pass over the record's asks, the samples' or the holes' stops',
            // its args the raster's: one pass, bound to either's args.
            let faces = |args: u32, at: u32| {
                g.bind_group(
                    self.faces_built.layouts[0],
                    &[
                        buf_entry(0, eye),
                        buf_entry(1, clipmap),
                        buf_entry(2, ground),
                        buf_entry(5, shards),
                        buf_entry(6, overwrites),
                        buf_entry(7, pool),
                        buf_entry(8, beams),
                        buf_range_entry(16, args, at, word_bytes(raster::ARGS_WORDS as usize)),
                        buf_entry(RECORD_BINDING, self.record),
                    ],
                )
            };
            self.groups = [
                g.bind_group(
                    self.boxes_built.layouts[0],
                    &[
                        buf_entry(0, eye),
                        buf_entry(8, beams),
                        buf_entry(16, self.args),
                        buf_entry(RECORD_BINDING, self.record),
                    ],
                ),
                g.bind_group(
                    self.draw_built.layouts[0],
                    &[buf_entry(0, self.uniform), buf_entry(1, self.boxes)],
                ),
                faces(self.args, word_bytes(raster::ASKED_AT as usize)),
                // Made over the light's buffers too (`record_group`).
                0,
                faces(self.args, word_bytes(raster::STOP_ASKED_AT as usize)),
                g.bind_group(
                    self.stops_built.layouts[0],
                    &[
                        buf_entry(0, eye),
                        view_entry(3, rays),
                        buf_entry(8, beams),
                        buf_entry(9, node_cache),
                        buf_entry(16, self.args),
                        buf_entry(RECORD_BINDING, self.record),
                        buf_entry(DEFERRED_BINDING, self.deferred),
                    ],
                ),
                g.bind_group(
                    self.deferred_built.layouts[0],
                    &[
                        buf_entry(0, eye),
                        buf_entry(8, beams),
                        buf_entry(16, self.args),
                        buf_entry(RECORD_BINDING, self.record),
                    ],
                ),
            ];
            self.bindings = bindings;
            self.record_bound = [0; 6];
        }
    }

    /// The record's group (`RECORD_GROUP`), over the eye and the node cache the raster's
    /// passes bind and the light's buffers its draw stands the boxes by, `light`: the light's
    /// head, its list, its args and its stood list's args (light_stood.wgsl); made again exactly
    /// when one of them is another, every frame the cast prepares, as the light's are made and
    /// let go apart from the raster's.
    pub(super) fn record_group(&mut self, g: &Gpu, light: [u32; 4]) {
        let [eye, _, node_cache, ..] = self.bindings;
        let [head, list, args, stood] = light;
        let bound = [eye, node_cache, head, list, args, stood];
        if eye == 0 || bound == self.record_bound {
            return;
        }
        release(self.groups[3]);
        self.groups[3] = g.bind_group(
            self.record_draw_built.layouts[0],
            &[
                buf_entry(0, eye),
                buf_entry(9, node_cache),
                buf_entry(14, self.boxes),
                buf_entry(16, self.args),
                buf_entry(RECORD_BINDING, self.record),
                buf_entry(super::cast::HEAD_BINDING, head),
                buf_entry(12, list),
                buf_entry(21, args),
                buf_entry(22, stood),
            ],
        );
        self.record_bound = bound;
    }

    /// The holes' stops into the record, in the cast's pass `pass` once the holes' queue is
    /// done (ray/cast.rs), a thread a stop off their count in the node cache's head
    /// (`node_cache`, pxl-engine `node_cache::HITS_ARGS`); those whose stop met its key's claim
    /// past that dispatch's end, off their dispatch's own buffer (`raster::DEFERRED_DISPATCH`),
    /// over a group that binds the args they write and not it; then what lies about each face they opened asked, the faces'
    /// pass over their asks, its args the raster's at `raster::STOP_ASKED_AT`; then each
    /// stop's face's word put at its pixel off its entry, a thread a stop.
    pub(super) fn stops(&self, g: &Gpu, pass: u32, node_cache: u32) {
        g.set_pipeline(pass, self.stops_built.id);
        g.set_bind_group(pass, 0, self.groups[5], &[]);
        g.dispatch_indirect(pass, node_cache, sequence::STOPS.at());
        g.set_bind_group(pass, 0, self.groups[6], &[]);
        g.set_pipeline(pass, self.deferred_built.id);
        g.dispatch_indirect(pass, self.deferred, sequence::STOPS_DEFERRED.at());
        g.set_pipeline(pass, self.faces_built.id);
        g.set_bind_group(pass, 0, self.groups[4], &[]);
        g.dispatch_indirect(pass, self.args, sequence::STOP_FACES.at());
        g.set_pipeline(pass, self.faces_put_built.id);
        g.set_bind_group(pass, 0, self.groups[5], &[]);
        g.dispatch_indirect(pass, node_cache, sequence::STOP_FACES_PUT.at());
    }

    /// The samples written into the record, a thread a sample of `grid`, where they were cast
    /// again (`samples_cast`); the record's voxels in view written as boxes, a thread a slot,
    /// where they were, or where the light's receivers stand this frame (`record_drawn`), else
    /// the last kept (span `sample_box`); and drawn into its targets as many as their args say,
    /// cleared first, through the gate in the `claimed` tiles where the frame
    /// is not drawn whole (`Resolve::tiles`, `cnt` of them; span `sraster`).
    pub(super) fn draw(
        &self,
        ctx: &mut RenderContext,
        grid: &beam::Grid,
        (samples_cast, record_drawn): (bool, bool),
        claimed: Option<((u32, u32), u32)>,
    ) {
        let (g, encoder) = (ctx.g, ctx.encoder);
        if samples_cast {
            // The samples into the record, the second pass reading what the first wrote
            // (pxl-engine `record`).
            let [x, y] = grid.samples;
            for built in [&self.boxes_built, &self.observe_built] {
                let pass = g.compute_pass(encoder, ctx.stamp(GpuSpanName::sample_box).pass());
                g.set_bind_group(pass, 0, self.groups[0], &[]);
                g.set_pipeline(pass, built.id);
                g.dispatch(pass, x.div_ceil(SQUARE_SIDE), y.div_ceil(SQUARE_SIDE), 1);
                g.end(pass);
            }
        }
        if record_drawn {
            self.record_draw(ctx);
        }
        // Gated, the depth starts at the nearest, which no box passes, and
        // the gate sets the claimed tiles back to the far plane; else it
        // starts there.
        let pass = g.pass(
            encoder,
            &pass_desc(
                self.visibility_view,
                Load::Clear,
                self.depth_view,
                Load::Clear,
                if claimed.is_some() { 1.0 } else { 0.0 },
                ctx.stamp(GpuSpanName::sraster).pass(),
            ),
        );
        g.declare_pass(pass, sequence::RASTER_PASS);
        if let Some((tiles, cnt)) = claimed {
            let group = g.bind_group(
                self.gate.layouts[0],
                &[buf_entry(30, tiles.0), buf_entry(31, tiles.1)],
            );
            g.set_pipeline(pass, self.gate.id);
            g.set_bind_group(pass, 0, group, &[]);
            g.draw(pass, 6, cnt, 0, 0);
            release(group);
        }
        g.set_pipeline(pass, self.draw_built.id);
        g.set_bind_group(pass, 0, self.groups[1], &[]);
        g.draw_indirect(pass, self.args, sequence::RASTER.at());
        g.end(pass);
        // What lies about each new face of the record, before the pixels read it off
        // their visible voxel; asked where the samples were cast.
        if !samples_cast {
            return;
        }
        let pass = g.compute_pass(encoder, ctx.stamp(GpuSpanName::face).pass());
        g.set_bind_group(pass, 0, self.groups[2], &[]);
        g.set_pipeline(pass, self.faces_built.id);
        g.dispatch_indirect(pass, self.args, sequence::FACE.at());
        g.end(pass);
    }

    /// Every held voxel of the record in view written as a box, a thread a slot, its draw's
    /// args first set to none, each box stood on by the light's receivers where they stand
    /// this frame (record_draw.wgsl `record_draw_cs`, pxl-engine `light::HEAD_STANDS_WORLD`):
    /// once a frame at most, by the raster's draw or, where the eye's pixels are not cast, by the
    /// light's cast ahead of its receivers' passes (ray/cast.rs).
    pub(super) fn record_draw(&self, ctx: &mut RenderContext) {
        let (g, encoder) = (ctx.g, ctx.encoder);
        g.write_buf(
            self.args,
            sequence::OPEN_RASTER_DRAW_ARGS.to_at(),
            bytes(&draw_args()),
        );
        let pass = g.compute_pass(encoder, ctx.stamp(GpuSpanName::sample_box).pass());
        g.set_bind_group(pass, 0, self.groups[3], &[]);
        g.set_pipeline(pass, self.record_draw_built.id);
        g.dispatch(pass, record::ENTRIES / record::DRAW_LANES, 1, 1);
        g.end(pass);
    }

    /// The record forgotten in the cast's pass `pass`, where the node cache is, by its head's
    /// rule (pxl-engine `record`; record_draw.wgsl `record_forget_cs`).
    pub(super) fn forget(&self, g: &Gpu, pass: u32) {
        g.set_bind_group(pass, 0, self.groups[3], &[]);
        g.set_pipeline(pass, self.forget_built.id);
        g.dispatch(pass, record::ENTRIES / record::DRAW_LANES, 1, 1);
    }
}

/// The draw's args as the record's draw begins: its vertices a box, no box (pxl-engine
/// `raster::ARGS_WORDS`).
fn draw_args() -> [u32; raster::ARGS_WORDS as usize] {
    let mut args = [0; raster::ARGS_WORDS as usize];
    args[raster::ARGS_VERTICES as usize] = raster::VERTICES;
    args
}

impl Raster {
    /// The light's buffers and its targets at the map's `size`, made where
    /// they are not; whether they are.
    pub(super) fn light_made(&mut self, g: &Gpu, size: u32) -> bool {
        let s = &mut self.light;
        if s.group == 0 {
            let storage = buffer_usage::STORAGE | buffer_usage::COPY_DST;
            // Made zeroed: every slot free.
            s.record = g.buf(word_bytes(record::WORDS as usize), storage);
            s.boxes = g.buf(
                word_bytes((record::ENTRIES * raster::BOX_WORDS) as usize),
                storage,
            );
            s.args = g.buf(
                word_bytes((raster::DEFERRED_AT + raster::DEFERRED_WORDS) as usize),
                storage | buffer_usage::INDIRECT | buffer_usage::COPY_SRC,
            );
            s.deferred = g.buf(
                word_bytes(raster::DEFERRED_DISPATCH.len()),
                storage | buffer_usage::INDIRECT,
            );
            s.uniform = g.buf(
                word_bytes(RASTER_FLOATS),
                buffer_usage::UNIFORM | buffer_usage::COPY_DST,
            );
            let usage = texture_usage::RENDER_ATTACHMENT | texture_usage::TEXTURE_BINDING;
            s.visibility = g.texture(size, size, 1, Format::r32uint, 1, usage);
            s.visibility_view = g.view(s.visibility, false, 0, 1, 0, 1);
            s.visibility_array = g.view(s.visibility, true, 0, 1, 0, 1);
            s.depth = g.texture(size, size, 1, Format::depth32float, 1, usage);
            s.depth_view = g.view(s.depth, false, 0, 1, 0, 1);
            s.group = g.bind_group(
                self.draw_built.layouts[0],
                &[buf_entry(0, s.uniform), buf_entry(1, s.boxes)],
            );
        }
        s.group != 0
    }

    /// The light's buffers and its targets let go, as its shadows go off;
    /// made again by `light_made`.
    pub(super) fn light_release(&mut self) {
        let s = core::mem::take(&mut self.light);
        for id in [
            s.group,
            s.record_group,
            s.stops_group,
            s.deferred_group,
            s.record,
            s.visibility_view,
            s.visibility_array,
            s.visibility,
            s.depth_view,
            s.depth,
            s.uniform,
            s.args,
            s.deferred,
            s.boxes,
        ] {
            release(id);
        }
    }

    /// The light's view off the eye's words `e` (pxl-engine `light::Light`) for
    /// the map `size` a side, as `prepare` lays the eye's; the args cleared; its record's
    /// frame the light's cast's `number` (`record::FRAME`), which its slots and its stops mark
    /// an entry seen by.
    pub(super) fn light_prepare(&self, g: &Gpu, e: &[u32], size: u32, number: u32) {
        let s = &self.light;
        if s.group == 0 {
            return;
        }
        g.write_buf(
            s.record,
            word_bytes(record::FRAME as usize),
            bytes(&[number]),
        );
        let words: &[u32; eye::LIGHT_WORDS] = e[eye::LIGHT..eye::LIGHT + eye::LIGHT_WORDS]
            .try_into()
            .unwrap_or(&[0; eye::LIGHT_WORDS]);
        let v = light::Light::of_words(words);
        let length = |a: [f32; 3]| libm::sqrtf(a[0] * a[0] + a[1] * a[1] + a[2] * a[2]);
        let unit = |a: [f32; 3]| {
            let l = length(a).max(f32::MIN_POSITIVE);
            [a[0] / l, a[1] / l, a[2] / l]
        };
        let (r, d) = (unit(v.across), unit(v.down));
        let up = [-d[0], -d[1], -d[2]];
        let turn = BOX_VERTICES * u32::from(raster::is_flipped(r, up, v.dir));
        let n = size as f32;
        let uniform: [f32; RASTER_FLOATS] = [
            r[0],
            r[1],
            r[2],
            length(v.across),
            d[0],
            d[1],
            d[2],
            length(v.down),
            v.dir[0],
            v.dir[1],
            v.dir[2],
            0.0,
            f32::from_bits(turn),
            f32::from_bits(1),
            0.0,
            0.0,
            GROWTH * length(v.across),
            n,
            n,
            v.t_max,
            // The boxes are laid from box texel (0, 0)'s start
            // (frame/shader/record_draw.wgsl `light_record_draw_cs`).
            0.0,
            0.0,
            0.0,
            0.0,
        ];
        g.write_buf(s.uniform, 0, bytes(&uniform));
        let mut args = [0u32; raster::ARGS_WORDS as usize];
        args[raster::ARGS_VERTICES as usize] = raster::VERTICES;
        g.write_buf(
            s.args,
            sequence::OPEN_LIGHT_RASTER_ARGS.to_at(),
            bytes(&args),
        );
        // No stop put off, its dispatch none, a workgroup deep and high (pxl-engine
        // `raster::DEFERRED_CNT`, `raster::DEFERRED_DISPATCH`).
        g.write_buf(
            s.args,
            sequence::OPEN_LIGHT_RASTER_DEFERRED_CNT.to_at(),
            bytes(&[0u32]),
        );
        g.write_buf(
            s.deferred,
            sequence::OPEN_LIGHT_RASTER_DEFERRED.to_at(),
            bytes(&raster::DEFERRED_DISPATCH),
        );
    }

    /// The light's record's group (`RECORD_GROUP`) over the eye and the node cache the
    /// raster's passes bind and the light's own buffers, `light`: its head, its list, its args,
    /// its stood list's args, which only the eye's draw reads, and its map, which its stops'
    /// group binds (`LIGHT_STOPS_GROUP`); made again, the stops' group with it, exactly when
    /// one of them is another.
    pub(super) fn light_record_group(&mut self, g: &Gpu, light: [u32; 5]) {
        let [eye, _, node_cache, ..] = self.bindings;
        let [head, list, args, stood, map] = light;
        let bound = [eye, node_cache, head, list, args, stood, map];
        let s = &mut self.light;
        if eye == 0 || s.group == 0 || bound == s.record_bound {
            return;
        }
        release(s.record_group);
        s.record_group = g.bind_group(
            self.light_record_draw_built.layouts[0],
            &[
                buf_entry(0, eye),
                buf_entry(9, node_cache),
                buf_entry(14, s.boxes),
                buf_entry(16, s.args),
                buf_entry(RECORD_BINDING, s.record),
                buf_entry(super::cast::HEAD_BINDING, head),
                buf_entry(12, list),
                buf_entry(21, args),
                buf_entry(22, stood),
            ],
        );
        release(s.stops_group);
        s.stops_group = g.bind_group(
            self.light_stops_built.layouts[0],
            &[
                buf_entry(0, eye),
                view_entry(3, map),
                buf_entry(8, list),
                buf_entry(9, node_cache),
                buf_entry(super::cast::HEAD_BINDING, head),
                buf_entry(16, s.args),
                buf_entry(RECORD_BINDING, s.record),
                buf_entry(DEFERRED_BINDING, s.deferred),
            ],
        );
        release(s.deferred_group);
        s.deferred_group = g.bind_group(
            self.light_deferred_built.layouts[0],
            &[
                buf_entry(0, eye),
                buf_entry(8, list),
                buf_entry(super::cast::HEAD_BINDING, head),
                buf_entry(16, s.args),
                buf_entry(RECORD_BINDING, s.record),
            ],
        );
        s.record_bound = bound;
    }

    /// The light's holes' stops into its record, every one its queue listed, dispatched off
    /// their count in the node cache's head, then those put off (record_stops.wgsl over
    /// record_stops.light.wgsl), in the pass `pass` after the light's queue: the light's
    /// record's draw takes them in its next cast. Its slots read no face's word, so no face is
    /// asked or put.
    pub(super) fn light_stops(&self, g: &Gpu, pass: u32, node_cache: u32) {
        let s = &self.light;
        if s.stops_group == 0 {
            return;
        }
        g.set_bind_group(pass, 0, s.stops_group, &[]);
        g.set_pipeline(pass, self.light_stops_built.id);
        g.dispatch_indirect(pass, node_cache, sequence::LIGHT_STOPS.at());
        g.set_bind_group(pass, 0, s.deferred_group, &[]);
        g.set_pipeline(pass, self.light_deferred_built.id);
        g.dispatch_indirect(pass, s.deferred, sequence::LIGHT_STOPS_DEFERRED.at());
    }

    /// The light's record forgotten in the cast's pass `pass` with the eye's, by the node
    /// cache's head's rule (record_draw.wgsl `record_forget_cs`), where it is made.
    pub(super) fn light_forget(&self, g: &Gpu, pass: u32) {
        if self.light.record_group == 0 {
            return;
        }
        g.set_bind_group(pass, 0, self.light.record_group, &[]);
        g.set_pipeline(pass, self.forget_built.id);
        g.dispatch(pass, record::ENTRIES / record::DRAW_LANES, 1, 1);
    }

    /// The light's record's voxels on its box written as boxes, a thread a slot
    /// (record_draw.wgsl `light_record_draw_cs`), then drawn into its targets as many as their
    /// args say, cleared first, timed in the span `light_raster`.
    pub(super) fn light_draw(&self, ctx: &mut RenderContext) {
        let (g, encoder) = (ctx.g, ctx.encoder);
        let stamp = ctx.stamp(GpuSpanName::light_raster);
        let s = &self.light;
        let pass = g.compute_pass(encoder, stamp.pass());
        g.set_bind_group(pass, 0, s.record_group, &[]);
        g.set_pipeline(pass, self.light_record_draw_built.id);
        g.dispatch(pass, record::ENTRIES / record::DRAW_LANES, 1, 1);
        g.end(pass);
        let stamp = ctx.stamp(GpuSpanName::light_raster);
        let pass = g.pass(
            encoder,
            &pass_desc(
                s.visibility_view,
                Load::Clear,
                s.depth_view,
                Load::Clear,
                0.0,
                stamp.pass(),
            ),
        );
        g.declare_pass(pass, sequence::LIGHT_RASTER_PASS);
        g.set_pipeline(pass, self.draw_built.id);
        g.set_bind_group(pass, 0, s.group, &[]);
        g.draw_indirect(pass, s.args, sequence::LIGHT_RASTER.at());
        g.end(pass);
    }
}
