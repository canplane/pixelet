// pxl-render/gpu/pass/location.rs

//! The location indicator: a zone of unit boxes drawn instanced, and an
//! octahedron beacon. Each of the two whose draw changed marks the screen's
//! tiles it covers where it was and where it is (gpu/pass/mark.rs). The
//! upload a zone reads holds its boxes; the beacon reads none.

use crate::frame::shader::record::{PIPELINE_LOCATION_BEACON, PIPELINE_LOCATION_ZONE};
use crate::frame::shader::record::{PIPELINE_LOCATION_BEACON_MARK, PIPELINE_LOCATION_ZONE_MARK};
use crate::frame::shader::word::{location, BOX_VERTICES};
use crate::gpu::api::{buf_entry, buffer_usage, bytes, release, Gpu, TrackedRenderPass};
use crate::gpu::draw::{uniform_of, DRAW_WORDS};
use crate::gpu::draw::{DrawStat, DrawTable};
use crate::gpu::pass::mark::{put_whole, MarkPass};
use crate::gpu::pass::model::{UniformBuffer, DRAW_BYTES};
use crate::gpu::pipeline::{Pipeline, PipelineCache};
use crate::gpu::put::put_f32;
use crate::gpu::upload::{buf_size, UploadTable};
use crate::look::color::indicator;
use crate::look::color::linear_rgb;
use crate::param::location::beacon;

/// The octahedron's vertices: three for each of its eight faces.
const BEACON_VERTICES: u32 = 24;

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

/// The floats in one of the octahedron's vertex lanes, three a vertex.
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
                // The triangle as listed winds outward when an even number of
                // its axes are negative, and is reversed otherwise.
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
    /// The zones' uniforms, a slot a draw, each bound at its own offset
    /// (`prepare`).
    zone_uniforms: UniformBuffer,
    /// The bind group the beacon reads the list's uniforms through, and the
    /// buffer it is made over (`set_draws`).
    draws: u32,
    draws_buf: u32,
    frame_group: u32,
    box_pos: u32,
    box_uv: u32,
    box_idx: u32,
    oct_pos: u32,
    oct_normal: u32,
    /// The UI's action color, in linear light.
    color: [f32; 3],
    pub zone_mark: MarkPass,
    pub beacon_mark: MarkPass,
}

fn put(g: &Gpu, data: &[u8], usage: u32) -> u32 {
    let id = g.buf(buf_size(data.len() as u32), usage | buffer_usage::COPY_DST);
    g.write_buf(id, 0, data);
    id
}

/// The zones that are drawn. A zone of no boxes is not.
fn zones<'a>(draws: &DrawTable<'a>) -> impl Iterator<Item = &'a [u32; DRAW_WORDS]> + Clone + 'a {
    draws.of(PIPELINE_LOCATION_ZONE).filter(|d| d[1] > 0)
}

impl LocationPass {
    pub fn new(g: &Gpu, pipelines: &mut PipelineCache, frame_buf: u32, align: u32) -> LocationPass {
        let zone = pipelines.build_world(g, PIPELINE_LOCATION_ZONE);
        let beacon = pipelines.build_world(g, PIPELINE_LOCATION_BEACON);
        let (box_p, box_uv, box_i) = box_lanes();
        let (oct_p, oct_n) = octahedron_lanes();
        let [r, gr, b] = linear_rgb(indicator::ACTION);
        LocationPass {
            frame_group: g.bind_group(zone.layouts[0], &[buf_entry(0, frame_buf)]),
            zone_uniforms: UniformBuffer::new(DRAW_BYTES, align),
            draws: 0,
            draws_buf: 0,
            zone,
            beacon,
            box_pos: put(g, bytes(&box_p), buffer_usage::VERTEX),
            box_uv: put(g, bytes(&box_uv), buffer_usage::VERTEX),
            box_idx: put(g, bytes(&box_i), buffer_usage::INDEX),
            oct_pos: put(g, bytes(&oct_p), buffer_usage::VERTEX),
            oct_normal: put(g, bytes(&oct_n), buffer_usage::VERTEX),
            color: [r as f32, gr as f32, b as f32],
            zone_mark: MarkPass::new(
                g,
                pipelines,
                (PIPELINE_LOCATION_ZONE_MARK, PIPELINE_LOCATION_ZONE),
                align,
                (|d| d[0], put_whole, |d| d[1] > 0),
            ),
            beacon_mark: MarkPass::new(
                g,
                pipelines,
                (PIPELINE_LOCATION_BEACON_MARK, PIPELINE_LOCATION_BEACON),
                align,
                (|_| 0, put_whole, |_| true),
            ),
        }
    }

    /// Draws this frame's marks into `pass`. `frames` holds their group 0 as
    /// they are and as they were (tile_mark.wgsl, `MarkPass::draw`).
    pub fn draw_marks(&self, pass: &mut TrackedRenderPass, frames: [u32; 2]) {
        self.zone_mark.draw(pass, frames, |pass, d, upload| {
            pass.set_vertex_buf(0, self.box_pos, 0, 0);
            pass.set_vertex_buf(1, self.box_uv, 0, 0);
            pass.set_idx_buf(self.box_idx, 0, 0);
            pass.set_bind_group(2, upload, &[]);
            pass.draw_indexed(BOX_VERTICES, d[1], 0, 0);
        });
        self.beacon_mark.draw(pass, frames, |pass, _, _| {
            pass.set_vertex_buf(0, self.oct_pos, 0, 0);
            pass.set_vertex_buf(1, self.oct_normal, 0, 0);
            pass.draw(BEACON_VERTICES, 1, 0, 0);
        });
    }

    /// Pushes onto `out` the uploads the zones read (`MarkPass::reads`). A
    /// beacon reads none.
    pub fn marks_read(&self, draws: &DrawTable, out: &mut Vec<u32>) {
        self.zone_mark.reads(zones(draws), out);
        self.beacon_mark
            .reads(draws.of(PIPELINE_LOCATION_BEACON), out);
    }

    /// Adds the pass's draws to `stat`: a box's triangles for each instance
    /// of a zone, and the octahedron's for each beacon.
    pub fn count(&self, draws: &DrawTable, stat: &mut DrawStat) {
        draws.count(PIPELINE_LOCATION_ZONE, stat, |d| BOX_VERTICES / 3 * d[1]);
        draws.count(PIPELINE_LOCATION_BEACON, stat, |_| BEACON_VERTICES / 3);
    }

    /// Writes each zone's uniform, with the action color, into a slot of its
    /// own once before the pass, when the lists it is made from `changed`. It
    /// also makes every upload group the zones will bind, so that drawing
    /// makes none: a group made inside a frame that is dropped would be gone.
    pub fn prepare(
        &mut self,
        g: &Gpu,
        draws: &DrawTable,
        uniforms: &[f32],
        changed: bool,
        uploads: &mut UploadTable,
    ) {
        let c = self.color;
        let put = |buf: &mut [u8], d: &[u32; DRAW_WORDS]| {
            buf[..DRAW_BYTES as usize].copy_from_slice(bytes(uniform_of(uniforms, d)));
            for (k, v) in c.into_iter().enumerate() {
                put_f32(buf, 4 * (location::COLOR + k), v);
            }
        };
        let zones = zones(draws);
        self.zone_uniforms.write(
            g,
            self.zone.layouts[1],
            zones.clone().count() as u32,
            zones.clone(),
            changed,
            put,
        );
        for d in zones {
            uploads.group(g, d[0]);
        }
    }

    /// Makes the group 1 through which the beacon reads the list's uniforms,
    /// over the buffer `buf`. It makes the group again only when the buffer is
    /// another. The caller calls it before the frame's commands, since a group
    /// made inside a frame that is dropped would be gone.
    pub fn set_draws(&mut self, g: &Gpu, buf: u32) {
        if buf == self.draws_buf {
            return;
        }
        release(self.draws);
        self.draws = g.bind_group(self.beacon.layouts[1], &[buf_entry(0, buf)]);
        self.draws_buf = buf;
    }

    /// Draws each beacon. The pipeline, the models' group 0 `frame`
    /// (gpu/pass/model.rs `FrameGroup`), the list's uniforms (`set_draws`)
    /// and the octahedron are bound once. Each beacon is drawn as the
    /// instance its slot names, and its vertices read their draw at that slot
    /// (location.slot.wgsl).
    pub fn draw_beacon(&self, pass: &mut TrackedRenderPass, draws: &DrawTable, frame: u32) {
        if self.draws == 0 || draws.of(PIPELINE_LOCATION_BEACON).len() == 0 {
            return;
        }
        pass.set_pipeline(self.beacon.id);
        pass.set_bind_group(0, frame, &[]);
        pass.set_bind_group(1, self.draws, &[]);
        pass.set_vertex_buf(0, self.oct_pos, 0, 0);
        pass.set_vertex_buf(1, self.oct_normal, 0, 0);
        for d in draws.of(PIPELINE_LOCATION_BEACON) {
            pass.draw(BEACON_VERTICES, 1, 0, d[3]);
        }
    }

    /// Draws each zone. The pipeline, the frame's group and the box are bound
    /// once, and each zone binds only its uniform's slot and its upload. A
    /// zone whose upload is not on the device yet is passed over.
    pub fn draw_zone(
        &self,
        pass: &mut TrackedRenderPass,
        draws: &DrawTable,
        uploads: &mut UploadTable,
    ) {
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
