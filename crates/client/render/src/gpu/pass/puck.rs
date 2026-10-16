// pxl-render/gpu/pass/puck.rs

//! The location puck: an instanced zone of unit boxes and an octahedron beacon.

use crate::frame::draw::{uniform_of, PIPELINE_PUCK_BEACON, PIPELINE_PUCK_ZONE};
use crate::gpu::api::{buffer_entry, buffer_usage, bytes, Gpu};
use crate::gpu::atmosphere::AIR_WORDS;
use crate::gpu::pipeline::{Built, Pipelines};
use crate::gpu::renderer::{buffer_size, Blocks, Draws};
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
    zone_uniform: u32,
    beacon_uniform: u32,
    frame_group: u32,
    zone_draw_group: u32,
    beacon_draw_group: u32,
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

impl PuckPass {
    pub fn new(g: &Gpu, pipelines: &mut Pipelines, frame_buffer: u32, air: [u32; AIR_WORDS]) -> PuckPass {
        let zone = pipelines.build(g, PIPELINE_PUCK_ZONE, g.canvas());
        let beacon = pipelines.build(g, PIPELINE_PUCK_BEACON, g.canvas());
        let uniform = || g.buffer(32, buffer_usage::UNIFORM | buffer_usage::COPY_DST);
        let (zone_uniform, beacon_uniform) = (uniform(), uniform());
        let group = |layout: u32, buffer: u32| g.bind_group(layout, &buffer_entry(0, buffer));
        let (box_p, box_uv, box_i) = box_lanes();
        let (oct_p, oct_n) = octahedron_lanes();
        let [r, gr, b] = linear_rgb(indicator::ACTION);
        // The beacon is lit through the air: the air's words, transmittance and sampler.
        let mut lit = buffer_entry(0, frame_buffer).to_vec();
        lit.extend(air.as_chunks::<5>().0.iter().filter(|c| [8, 9, 11].contains(&c[0])).flatten());
        let beacon_frame_group = g.bind_group(beacon.layouts[0], &lit);
        PuckPass {
            frame_group: group(zone.layouts[0], frame_buffer),
            beacon_frame_group,
            zone_draw_group: group(zone.layouts[1], zone_uniform),
            beacon_draw_group: group(beacon.layouts[1], beacon_uniform),
            zone,
            beacon,
            zone_uniform,
            beacon_uniform,
            box_position: put(g, bytes(&box_p), buffer_usage::VERTEX),
            box_uv: put(g, bytes(&box_uv), buffer_usage::VERTEX),
            box_index: put(g, bytes(&box_i), buffer_usage::INDEX),
            oct_position: put(g, bytes(&oct_p), buffer_usage::VERTEX),
            oct_normal: put(g, bytes(&oct_n), buffer_usage::VERTEX),
            color: [r as f32, gr as f32, b as f32],
        }
    }

    fn write_uniform(&self, g: &Gpu, buffer: u32, u: &[f32], night: f32) {
        let c = self.color;
        g.write_buffer(buffer, 0, bytes(&[u[0], u[1], u[2], u[3], c[0], c[1], c[2], night]));
    }

    pub fn draw_beacon(&self, g: &Gpu, pass: u32, draws: &Draws, uniforms: &[f32]) {
        for d in draws.of(PIPELINE_PUCK_BEACON) {
            self.write_uniform(g, self.beacon_uniform, uniform_of(uniforms, d), d[1] as f32);
            g.set_pipeline(pass, self.beacon.pipeline);
            g.set_bind_group(pass, 0, self.beacon_frame_group, &[]);
            g.set_bind_group(pass, 1, self.beacon_draw_group, &[]);
            g.set_vertex_buffer(pass, 0, self.oct_position, 0, 0);
            g.set_vertex_buffer(pass, 1, self.oct_normal, 0, 0);
            g.draw(pass, 24, 1, 0, 0);
        }
    }

    /// Every block group the zone draws will bind is made here, so drawing makes
    /// nothing that outlives its frame.
    pub fn prepare_zone(&self, g: &Gpu, draws: &Draws, blocks: &mut Blocks) {
        for d in draws.of(PIPELINE_PUCK_ZONE).filter(|d| d[1] > 0) {
            blocks.group(g, d[0], self.zone.layouts[2]);
        }
    }

    pub fn draw_zone(&self, g: &Gpu, pass: u32, draws: &Draws, uniforms: &[f32], blocks: &mut Blocks) {
        for d in draws.of(PIPELINE_PUCK_ZONE).filter(|d| d[1] > 0) {
            let Some(group) = blocks.group(g, d[0], self.zone.layouts[2]) else { continue };
            self.write_uniform(g, self.zone_uniform, uniform_of(uniforms, d), 0.0);
            g.set_pipeline(pass, self.zone.pipeline);
            g.set_bind_group(pass, 0, self.frame_group, &[]);
            g.set_bind_group(pass, 1, self.zone_draw_group, &[]);
            g.set_bind_group(pass, 2, group, &[]);
            g.set_vertex_buffer(pass, 0, self.box_position, 0, 0);
            g.set_vertex_buffer(pass, 1, self.box_uv, 0, 0);
            g.set_index_buffer(pass, self.box_index, 0, 0);
            g.draw_indexed(pass, 36, d[1], 0, 0);
        }
    }
}
