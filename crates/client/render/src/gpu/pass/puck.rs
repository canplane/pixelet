// pxl-render/gpu/pass/puck.rs

//! The location puck: an instanced zone of unit boxes and an octahedron beacon.

use crate::frame::draw::{uniform_of, DRAW_WORDS, PIPELINE_PUCK_BEACON, PIPELINE_PUCK_ZONE};
use crate::gpu::api::{buffer_entry, buffer_usage, bytes, Entry, Gpu};
use crate::gpu::atmosphere::AIR_ENTRIES;
use crate::gpu::blocks::{buffer_size, Blocks, Draws};
use crate::gpu::pass::model::{DrawUniforms, DRAW_BYTES};
use crate::gpu::pipeline::{Built, Pipelines};
use crate::look::color::indicator;
use crate::look::color::linear_rgb;

const BEACON_RADIUS: f32 = 0.3;

fn box_lanes() -> ([f32; 72], [f32; 48], [u32; 36]) {
    let faces: [[[f32; 3]; 3]; 6] = [
        [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        [[-1.0, 0.0, 0.0], [0.0, -1.0, 0.0], [0.0, 0.0, 1.0]],
        [[0.0, 1.0, 0.0], [-1.0, 0.0, 0.0], [0.0, 0.0, 1.0]],
        [[0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]],
        [[0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
        [[0.0, 0.0, -1.0], [-1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
    ];
    let (mut position, mut uv, mut index) = ([0.0; 72], [0.0; 48], [0u32; 36]);
    for (f, [n, u, w]) in faces.iter().enumerate() {
        for (c, (a, b)) in [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)].into_iter().enumerate() {
            let k = f * 4 + c;
            for x in 0..3 {
                position[k * 3 + x] = n[x] * 0.5 + u[x] * (a - 0.5) + w[x] * (b - 0.5);
            }
            uv[k * 2] = a;
            uv[k * 2 + 1] = b;
        }
        for (i, d) in [0, 1, 2, 0, 2, 3].into_iter().enumerate() {
            index[f * 6 + i] = (f * 4) as u32 + d;
        }
    }
    (position, uv, index)
}

fn octahedron_lanes() -> ([f32; 72], [f32; 72]) {
    let r = BEACON_RADIUS;
    let (mut position, mut normal) = ([0.0; 72], [0.0; 72]);
    let len = 3f32.sqrt();
    let mut k = 0;
    for sx in [1.0f32, -1.0] {
        for sy in [1.0f32, -1.0] {
            for sz in [1.0f32, -1.0] {
                let mut tri = [[sx * r, 0.0, 0.0], [0.0, sy * r, 0.0], [0.0, 0.0, sz * r]];
                // Outward winding: flipped when an odd number of axes are negative.
                if sx * sy * sz < 0.0 {
                    tri.reverse();
                }
                for p in tri {
                    position[k * 3..k * 3 + 3].copy_from_slice(&p);
                    normal[k * 3..k * 3 + 3].copy_from_slice(&[sx / len, sy / len, sz / len]);
                    k += 1;
                }
            }
        }
    }
    (position, normal)
}

pub struct PuckPass {
    zone: Built,
    beacon: Built,
    /// A slot a draw, each read at its own offset (`prepare`).
    zone_uniforms: DrawUniforms,
    beacon_uniforms: DrawUniforms,
    frame_group: u32,
    beacon_frame_group: u32,
    box_position: u32,
    box_uv: u32,
    box_index: u32,
    oct_position: u32,
    oct_normal: u32,
    /// The UI's action color, in linear light.
    color: [f32; 3],
}

fn put(g: &Gpu, data: &[u8], usage: u32) -> u32 {
    let id = g.buffer(buffer_size(data.len() as u32), usage | buffer_usage::COPY_DST);
    g.write_buffer(id, 0, data);
    id
}

/// The zones drawn: one of no boxes is not.
fn zones<'a>(draws: &Draws<'a>) -> impl Iterator<Item = &'a [u32; DRAW_WORDS]> + Clone + 'a {
    draws.of(PIPELINE_PUCK_ZONE).filter(|d| d[1] > 0)
}

impl PuckPass {
    pub fn new(g: &Gpu, pipelines: &mut Pipelines, frame_buffer: u32, air: [Entry; AIR_ENTRIES], align: u32) -> PuckPass {
        let zone = pipelines.build(g, PIPELINE_PUCK_ZONE, g.canvas());
        let beacon = pipelines.build(g, PIPELINE_PUCK_BEACON, g.canvas());
        let (box_p, box_uv, box_i) = box_lanes();
        let (oct_p, oct_n) = octahedron_lanes();
        let [r, gr, b] = linear_rgb(indicator::ACTION);
        // The beacon is lit through the air: the air's words, transmittance and sampler.
        let mut lit = vec![buffer_entry(0, frame_buffer)];
        lit.extend(air.iter().filter(|e| [8, 9, 11].contains(&e.binding)));
        let beacon_frame_group = g.bind_group(beacon.layouts[0], &lit);
        PuckPass {
            frame_group: g.bind_group(zone.layouts[0], &[buffer_entry(0, frame_buffer)]),
            beacon_frame_group,
            zone_uniforms: DrawUniforms::new(DRAW_BYTES, align),
            beacon_uniforms: DrawUniforms::new(DRAW_BYTES, align),
            zone,
            beacon,
            box_position: put(g, bytes(&box_p), buffer_usage::VERTEX),
            box_uv: put(g, bytes(&box_uv), buffer_usage::VERTEX),
            box_index: put(g, bytes(&box_i), buffer_usage::INDEX),
            oct_position: put(g, bytes(&oct_p), buffer_usage::VERTEX),
            oct_normal: put(g, bytes(&oct_n), buffer_usage::VERTEX),
            color: [r as f32, gr as f32, b as f32],
        }
    }

    /// Each draw's uniform in a slot of its own, written once before the
    /// pass where the lists it is made from `moved`; and every block group
    /// the zones will bind, so drawing makes nothing that outlives its frame.
    pub fn prepare(&mut self, g: &Gpu, draws: &Draws, uniforms: &[f32], moved: bool, blocks: &mut Blocks) {
        let c = self.color;
        let put = |buf: &mut [u8], (d, night): (&[u32; DRAW_WORDS], f32)| {
            let u = uniform_of(uniforms, d);
            buf[..DRAW_BYTES as usize].copy_from_slice(bytes(&[u[0], u[1], u[2], u[3], c[0], c[1], c[2], night]));
        };
        let beacons = draws.of(PIPELINE_PUCK_BEACON);
        self.beacon_uniforms
            .write(g, self.beacon.layouts[1], beacons.len() as u32, beacons.map(|d| (d, d[1] as f32)), moved, put);
        let zones = zones(draws);
        self.zone_uniforms
            .write(g, self.zone.layouts[1], zones.clone().count() as u32, zones.clone().map(|d| (d, 0.0)), moved, put);
        for d in zones {
            blocks.group(g, d[0]);
        }
    }

    pub fn draw_beacon(&self, g: &Gpu, pass: u32, draws: &Draws) {
        let Some(slots) = self.beacon_uniforms.slots() else { return };
        for k in 0..draws.of(PIPELINE_PUCK_BEACON).len() as u32 {
            let (group, offset) = slots.at(k);
            g.set_pipeline(pass, self.beacon.pipeline);
            g.set_bind_group(pass, 0, self.beacon_frame_group, &[]);
            g.set_bind_group(pass, 1, group, &[offset]);
            g.set_vertex_buffer(pass, 0, self.oct_position, 0, 0);
            g.set_vertex_buffer(pass, 1, self.oct_normal, 0, 0);
            g.draw(pass, 24, 1, 0, 0);
        }
    }

    /// A zone whose block is not on the device yet is passed over.
    pub fn draw_zone(&self, g: &Gpu, pass: u32, draws: &Draws, blocks: &mut Blocks) {
        let Some(slots) = self.zone_uniforms.slots() else { return };
        for (k, d) in zones(draws).enumerate() {
            let Some(block) = blocks.group(g, d[0]) else { continue };
            let (group, offset) = slots.at(k as u32);
            g.set_pipeline(pass, self.zone.pipeline);
            g.set_bind_group(pass, 0, self.frame_group, &[]);
            g.set_bind_group(pass, 1, group, &[offset]);
            g.set_bind_group(pass, 2, block, &[]);
            g.set_vertex_buffer(pass, 0, self.box_position, 0, 0);
            g.set_vertex_buffer(pass, 1, self.box_uv, 0, 0);
            g.set_index_buffer(pass, self.box_index, 0, 0);
            g.draw_indexed(pass, 36, d[1], 0, 0);
        }
    }
}
