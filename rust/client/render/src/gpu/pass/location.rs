// pxl-render/gpu/pass/location.rs

//! The location indicator: an instanced zone of unit boxes and an octahedron beacon.
//! Each, its draw changed, marks the screen's tiles it covers where it was and
//! where it is (gpu/pass/mark.rs), the zone its boxes the upload it reads.

use crate::frame::draw::{
    uniform_of, DRAW_WORDS, PIPELINE_LOCATION_BEACON, PIPELINE_LOCATION_ZONE,
};
use crate::frame::shader::record::{PIPELINE_LOCATION_BEACON_MARK, PIPELINE_LOCATION_ZONE_MARK};
use crate::frame::shader::word::BOX_VERTICES;
use crate::gpu::api::{buf_entry, buffer_usage, bytes, Gpu, TrackedRenderPass};
use crate::gpu::pass::mark::{self, Marks, UploadCopies};
use crate::gpu::pass::model::{DrawUniforms, DRAW_BYTES};
use crate::gpu::pipeline::{Pipeline, Pipelines};
use crate::gpu::upload::{buf_size, Draws, Uploads};
use crate::look::color::indicator;
use crate::look::color::linear_rgb;
use crate::param::location::beacon;

/// The octahedron's: three for each of its eight faces.
pub const BEACON_VERTICES: u32 = 24;

fn box_lanes() -> ([f32; 72], [f32; 48], [u32; BOX_VERTICES as usize]) {
    let faces: [[[f32; 3]; 3]; 6] = [
        [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        [[-1.0, 0.0, 0.0], [0.0, -1.0, 0.0], [0.0, 0.0, 1.0]],
        [[0.0, 1.0, 0.0], [-1.0, 0.0, 0.0], [0.0, 0.0, 1.0]],
        [[0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]],
        [[0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
        [[0.0, 0.0, -1.0], [-1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
    ];
    let (mut pos, mut uv, mut idx) = ([0.0; 72], [0.0; 48], [0u32; BOX_VERTICES as usize]);
    for (f, [n, u, w]) in faces.iter().enumerate() {
        for (c, (a, b)) in [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)]
            .into_iter()
            .enumerate()
        {
            let k = f * 4 + c;
            for x in 0..3 {
                pos[k * 3 + x] = n[x] * 0.5 + u[x] * (a - 0.5) + w[x] * (b - 0.5);
            }
            uv[k * 2] = a;
            uv[k * 2 + 1] = b;
        }
        for (i, d) in [0, 1, 2, 0, 2, 3].into_iter().enumerate() {
            idx[f * 6 + i] = (f * 4) as u32 + d;
        }
    }
    (pos, uv, idx)
}

/// A float lane of the octahedron's: three a vertex.
const BEACON_LANE: usize = 3 * BEACON_VERTICES as usize;

fn octahedron_lanes() -> ([f32; BEACON_LANE], [f32; BEACON_LANE]) {
    let r = beacon::RADIUS;
    let (mut pos, mut normal) = ([0.0; BEACON_LANE], [0.0; BEACON_LANE]);
    let len = 3f32.sqrt();
    let mut k = 0;
    for sx in [1.0f32, -1.0] {
        for sy in [1.0f32, -1.0] {
            for size in [1.0f32, -1.0] {
                let mut tri = [[sx * r, 0.0, 0.0], [0.0, sy * r, 0.0], [0.0, 0.0, size * r]];
                // Outward winding: flipped when an odd number of axes are negative.
                if sx * sy * size < 0.0 {
                    tri.reverse();
                }
                for p in tri {
                    pos[k * 3..k * 3 + 3].copy_from_slice(&p);
                    normal[k * 3..k * 3 + 3].copy_from_slice(&[sx / len, sy / len, size / len]);
                    k += 1;
                }
            }
        }
    }
    (pos, normal)
}

pub struct LocationPass {
    zone: Pipeline,
    beacon: Pipeline,
    /// A slot a draw, each read at its own offset (`prepare`).
    zone_uniforms: DrawUniforms,
    beacon_uniforms: DrawUniforms,
    frame_group: u32,
    box_pos: u32,
    box_uv: u32,
    box_idx: u32,
    oct_pos: u32,
    oct_normal: u32,
    /// The UI's action color, in linear light.
    color: [f32; 3],
    zone_marks: Marks,
    beacon_marks: Marks,
}

/// A mark's uniform: the draw's position and turn, where the lit pass reads
/// them (location.wgsl `Draw`).
fn put_mark(buf: &mut [u8], (_, u): &mark::Draw) {
    buf[..DRAW_BYTES as usize].copy_from_slice(bytes(&[
        u[0], u[1], u[2], u[3], 0.0, 0.0, 0.0, 0.0, u[8], u[9], u[10], 0.0,
    ]));
}

fn put(g: &Gpu, data: &[u8], usage: u32) -> u32 {
    let id = g.buf(buf_size(data.len() as u32), usage | buffer_usage::COPY_DST);
    g.write_buf(id, 0, data);
    id
}

/// The zones drawn: one of no boxes is not.
fn zones<'a>(draws: &Draws<'a>) -> impl Iterator<Item = &'a [u32; DRAW_WORDS]> + Clone + 'a {
    draws.of(PIPELINE_LOCATION_ZONE).filter(|d| d[1] > 0)
}

impl LocationPass {
    pub fn new(g: &Gpu, pipelines: &mut Pipelines, frame_buf: u32, align: u32) -> LocationPass {
        let zone = pipelines.build_world(g, PIPELINE_LOCATION_ZONE);
        let beacon = pipelines.build_world(g, PIPELINE_LOCATION_BEACON);
        let (box_p, box_uv, box_i) = box_lanes();
        let (oct_p, oct_n) = octahedron_lanes();
        let [r, gr, b] = linear_rgb(indicator::ACTION);
        LocationPass {
            frame_group: g.bind_group(zone.layouts[0], &[buf_entry(0, frame_buf)]),
            zone_uniforms: DrawUniforms::new(DRAW_BYTES, align),
            beacon_uniforms: DrawUniforms::new(DRAW_BYTES, align),
            zone,
            beacon,
            box_pos: put(g, bytes(&box_p), buffer_usage::VERTEX),
            box_uv: put(g, bytes(&box_uv), buffer_usage::VERTEX),
            box_idx: put(g, bytes(&box_i), buffer_usage::INDEX),
            oct_pos: put(g, bytes(&oct_p), buffer_usage::VERTEX),
            oct_normal: put(g, bytes(&oct_n), buffer_usage::VERTEX),
            color: [r as f32, gr as f32, b as f32],
            zone_marks: Marks::new(
                g,
                pipelines,
                PIPELINE_LOCATION_ZONE_MARK,
                align,
                (|d| d[0], put_mark),
            ),
            beacon_marks: Marks::new(
                g,
                pipelines,
                PIPELINE_LOCATION_BEACON_MARK,
                align,
                (|_| 0, put_mark),
            ),
        }
    }

    /// The marks this frame of the zones and the beacons (gpu/pass/mark.rs
    /// `Marks::prepare`). Whether any.
    pub fn prepare_marks(
        &mut self,
        g: &Gpu,
        draws: &Draws,
        uniforms: &[f32],
        touched: &[u32],
        uploads: &mut Uploads,
        last_frame_uploads: &UploadCopies,
    ) -> bool {
        let zones = self.zone_marks.prepare(
            g,
            zones(draws),
            uniforms,
            touched,
            uploads,
            last_frame_uploads,
        );
        let beacons = draws.of(PIPELINE_LOCATION_BEACON);
        let beacon =
            self.beacon_marks
                .prepare(g, beacons, uniforms, touched, uploads, last_frame_uploads);
        zones || beacon
    }

    /// The marks `prepare_marks` made drawn into `pass`, `frames` their group
    /// 0 as they are and as they were (tile_mark.wgsl, `Marks::draw`).
    pub fn draw_marks(&self, pass: &mut TrackedRenderPass, frames: [u32; 2]) {
        self.zone_marks.draw(pass, frames, |pass, d, upload| {
            pass.set_vertex_buf(0, self.box_pos, 0, 0);
            pass.set_vertex_buf(1, self.box_uv, 0, 0);
            pass.set_idx_buf(self.box_idx, 0, 0);
            pass.set_bind_group(2, upload, &[]);
            pass.draw_indexed(BOX_VERTICES, d[1], 0, 0);
        });
        self.beacon_marks.draw(pass, frames, |pass, _, _| {
            pass.set_vertex_buf(0, self.oct_pos, 0, 0);
            pass.set_vertex_buf(1, self.oct_normal, 0, 0);
            pass.draw(BEACON_VERTICES, 1, 0, 0);
        });
    }

    /// The uploads the zones and the beacons read, into `out`
    /// (`Marks::reads`).
    pub fn marks_read(&self, draws: &Draws, out: &mut Vec<u32>) {
        self.zone_marks.reads(zones(draws), out);
        self.beacon_marks
            .reads(draws.of(PIPELINE_LOCATION_BEACON), out);
    }

    /// What this frame listed kept for the next's marks where it was.
    pub fn keep(&mut self, draws: &Draws, uniforms: &[f32]) {
        self.zone_marks.keep(zones(draws), uniforms);
        self.beacon_marks
            .keep(draws.of(PIPELINE_LOCATION_BEACON), uniforms);
    }

    /// Each draw's uniform in a slot of its own, written once before the
    /// pass where the lists it is made from `moved`; and every upload group
    /// the zones will bind, so drawing makes nothing that outlives its frame.
    pub fn prepare(
        &mut self,
        g: &Gpu,
        draws: &Draws,
        uniforms: &[f32],
        moved: bool,
        uploads: &mut Uploads,
    ) {
        let c = self.color;
        let put = |buf: &mut [u8], (d, night): (&[u32; DRAW_WORDS], f32)| {
            let u = uniform_of(uniforms, d);
            buf[..DRAW_BYTES as usize].copy_from_slice(bytes(&[
                u[0], u[1], u[2], u[3], c[0], c[1], c[2], night, u[8], u[9], u[10], 0.0,
            ]));
        };
        let beacons = draws.of(PIPELINE_LOCATION_BEACON);
        self.beacon_uniforms.write(
            g,
            self.beacon.layouts[1],
            beacons.len() as u32,
            beacons.map(|d| (d, d[1] as f32)),
            moved,
            put,
        );
        let zones = zones(draws);
        self.zone_uniforms.write(
            g,
            self.zone.layouts[1],
            zones.clone().count() as u32,
            zones.clone().map(|d| (d, 0.0)),
            moved,
            put,
        );
        for d in zones {
            uploads.group(g, d[0]);
        }
    }

    /// The pipeline, the models' group 0 `frame` (gpu/pass/model.rs
    /// `FrameGroups`) and the octahedron bound once; a draw binds its
    /// uniform's slot alone.
    pub fn draw_beacon(&self, pass: &mut TrackedRenderPass, draws: &Draws, frame: u32) {
        let Some(slots) = self.beacon_uniforms.slots() else {
            return;
        };
        let cnt = draws.of(PIPELINE_LOCATION_BEACON).len() as u32;
        if cnt == 0 {
            return;
        }
        pass.set_pipeline(self.beacon.id);
        pass.set_bind_group(0, frame, &[]);
        pass.set_vertex_buf(0, self.oct_pos, 0, 0);
        pass.set_vertex_buf(1, self.oct_normal, 0, 0);
        for k in 0..cnt {
            let (group, off) = slots.at(k);
            pass.set_bind_group(1, group, &[off]);
            pass.draw(BEACON_VERTICES, 1, 0, 0);
        }
    }

    /// The pipeline, the frame's group and the box bound once; a draw binds
    /// its uniform's slot and its upload alone. A zone whose upload is not on
    /// the device yet is passed over.
    pub fn draw_zone(&self, pass: &mut TrackedRenderPass, draws: &Draws, uploads: &mut Uploads) {
        let Some(slots) = self.zone_uniforms.slots() else {
            return;
        };
        if zones(draws).next().is_none() {
            return;
        }
        pass.set_pipeline(self.zone.id);
        pass.set_bind_group(0, self.frame_group, &[]);
        pass.set_vertex_buf(0, self.box_pos, 0, 0);
        pass.set_vertex_buf(1, self.box_uv, 0, 0);
        pass.set_idx_buf(self.box_idx, 0, 0);
        for (k, d) in zones(draws).enumerate() {
            let Some(upload) = uploads.group_of(d[0]) else {
                continue;
            };
            let (group, off) = slots.at(k as u32);
            pass.set_bind_group(1, group, &[off]);
            pass.set_bind_group(2, upload, &[]);
            pass.draw_indexed(BOX_VERTICES, d[1], 0, 0);
        }
    }
}
