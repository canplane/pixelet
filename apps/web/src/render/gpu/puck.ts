// @pxl/web/render/gpu/puck.ts

/**
 * The location puck through WebGPU: the module's zone and beacon draws
 * (crates/pxl-core/src/world/marker.rs) — the zone a unit box a cell instanced
 * from the zone block, the beacon a small octahedron — under
 * shader/puck_zone.wgsl and puck_beacon.wgsl. The geometry is built here,
 * once.
 */

import type { u32 } from "@xpute/core/kit/word.ts";

import { marker_colors } from "@/module/world/marker/style.ts";
import type { PipelineBuilder } from "./pipeline.ts";
import { linear_rgb as linear } from "./color.ts";

const DRAW_WORDS = 5;
const UNIFORM_FLOATS = 8;
const PIPELINE_PUCK_ZONE = 6;
const PIPELINE_PUCK_BEACON = 7;

const BEACON_RADIUS = 0.3;
/** The zone's colour, the UI's action colour, in linear light: written
 * into the draw uniform (shader/puck.wgsl), since it is the glue's. */
const ZONE_COLOR = linear(marker_colors.ACTION);

/** A unit box centred on the origin, a face at a time with its own 0..1 UVs. */
function box_lanes(): { position: Float32Array<ArrayBuffer>; uv: Float32Array<ArrayBuffer>; index: Uint32Array<ArrayBuffer> } {
  const faces: [number[], number[], number[]][] = [
    [[1, 0, 0], [0, 1, 0], [0, 0, 1]],
    [[-1, 0, 0], [0, -1, 0], [0, 0, 1]],
    [[0, 1, 0], [-1, 0, 0], [0, 0, 1]],
    [[0, -1, 0], [1, 0, 0], [0, 0, 1]],
    [[0, 0, 1], [1, 0, 0], [0, 1, 0]],
    [[0, 0, -1], [-1, 0, 0], [0, 1, 0]],
  ];
  const position = new Float32Array(24 * 3);
  const uv = new Float32Array(24 * 2);
  const index = new Uint32Array(36);
  faces.forEach(([n, u, w], f) => {
    [[0, 0], [1, 0], [1, 1], [0, 1]].forEach(([a, b], c) => {
      const k = f * 4 + c;
      for (let x = 0; x < 3; x++) position[k * 3 + x] = n[x] * 0.5 + u[x] * (a - 0.5) + w[x] * (b - 0.5);
      uv[k * 2] = a;
      uv[k * 2 + 1] = b;
    });
    [0, 1, 2, 0, 2, 3].forEach((d, i) => (index[f * 6 + i] = f * 4 + d));
  });
  return { position, uv, index };
}

/** An octahedron of BEACON_RADIUS, a face at a time with its flat normal. */
function octahedron_lanes(): { position: Float32Array<ArrayBuffer>; normal: Float32Array<ArrayBuffer> } {
  const r = BEACON_RADIUS;
  const position = new Float32Array(24 * 3);
  const normal = new Float32Array(24 * 3);
  let k = 0;
  for (const sx of [1, -1]) {
    for (const sy of [1, -1]) {
      for (const sz of [1, -1]) {
        const tri = [[sx * r, 0, 0], [0, sy * r, 0], [0, 0, sz * r]];
        // Outward winding: flip when an odd number of axes are negative.
        if (sx * sy * sz < 0) tri.reverse();
        const len = Math.sqrt(3);
        for (const p of tri) {
          position.set(p, k * 3);
          normal.set([sx / len, sy / len, sz / len], k * 3);
          k++;
        }
      }
    }
  }
  return { position, normal };
}

export class PuckPass {
  private readonly zone_pipeline: GPURenderPipeline;
  private readonly beacon_pipeline: GPURenderPipeline;
  private readonly zone_uniform: GPUBuffer;
  private readonly beacon_uniform: GPUBuffer;
  private readonly frame_group: GPUBindGroup;
  private readonly zone_draw_group: GPUBindGroup;
  private readonly beacon_draw_group: GPUBindGroup;
  private readonly zone_layout: GPUBindGroupLayout;
  private readonly box: { position: GPUBuffer; uv: GPUBuffer; index: GPUBuffer };
  private readonly octahedron: { position: GPUBuffer; normal: GPUBuffer };
  private readonly zone_groups = new WeakMap<GPUBuffer, GPUBindGroup>();
  private readonly scratch = new Float32Array(new ArrayBuffer(32));

  constructor(private readonly device: GPUDevice, pipelines: PipelineBuilder, format: GPUTextureFormat, frame_buffer: GPUBuffer, sample_count: u32) {
    const zone = pipelines.build(PIPELINE_PUCK_ZONE, "puck/zone", format, sample_count);
    const beacon = pipelines.build(PIPELINE_PUCK_BEACON, "puck/beacon", format, sample_count);
    this.zone_pipeline = zone.pipeline;
    this.beacon_pipeline = beacon.pipeline;
    this.zone_layout = zone.layouts[2];
    const uniform = (label: string) => device.createBuffer({ label, size: 32, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST });
    this.zone_uniform = uniform("puck/zone/draw");
    this.beacon_uniform = uniform("puck/beacon/draw");
    this.frame_group = device.createBindGroup({ layout: zone.layouts[0], entries: [{ binding: 0, resource: { buffer: frame_buffer } }] });
    this.zone_draw_group = device.createBindGroup({ layout: zone.layouts[1], entries: [{ binding: 0, resource: { buffer: this.zone_uniform } }] });
    this.beacon_draw_group = device.createBindGroup({ layout: beacon.layouts[1], entries: [{ binding: 0, resource: { buffer: this.beacon_uniform } }] });
    const put = (data: ArrayBufferView<ArrayBuffer>, usage: GPUBufferUsageFlags) => {
      const b = device.createBuffer({ size: Math.max(4, (data.byteLength + 3) & ~3), usage: usage | GPUBufferUsage.COPY_DST });
      device.queue.writeBuffer(b, 0, data);
      return b;
    };
    const box = box_lanes();
    this.box = { position: put(box.position, GPUBufferUsage.VERTEX), uv: put(box.uv, GPUBufferUsage.VERTEX), index: put(box.index, GPUBufferUsage.INDEX) };
    const oct = octahedron_lanes();
    this.octahedron = { position: put(oct.position, GPUBufferUsage.VERTEX), normal: put(oct.normal, GPUBufferUsage.VERTEX) };
  }

  private write_uniform(buffer: GPUBuffer, uniforms: Float32Array, at: u32, night: number): void {
    const s = this.scratch;
    s[0] = uniforms[at];
    s[1] = uniforms[at + 1];
    s[2] = uniforms[at + 2];
    s[3] = uniforms[at + 3];
    s[4] = ZONE_COLOR[0];
    s[5] = ZONE_COLOR[1];
    s[6] = ZONE_COLOR[2];
    s[7] = night;
    this.device.queue.writeBuffer(buffer, 0, s);
  }

  /** The beacon, opaque; before anything blended. */
  draw_beacon(pass: GPURenderPassEncoder, draws: Uint32Array, uniforms: Float32Array): void {
    for (let k = 0; k < draws.length; k += DRAW_WORDS) {
      if (draws[k + 4] !== PIPELINE_PUCK_BEACON) continue;
      this.write_uniform(this.beacon_uniform, uniforms, draws[k + 3] * UNIFORM_FLOATS, draws[k + 1]);
      pass.setPipeline(this.beacon_pipeline);
      pass.setBindGroup(0, this.frame_group);
      pass.setBindGroup(1, this.beacon_draw_group);
      pass.setVertexBuffer(0, this.octahedron.position);
      pass.setVertexBuffer(1, this.octahedron.normal);
      pass.draw(24);
    }
  }

  /** The zone, blended; with the rest of what is blended. */
  draw_zone(pass: GPURenderPassEncoder, draws: Uint32Array, uniforms: Float32Array, buffers: ReadonlyMap<u32, GPUBuffer>): void {
    for (let k = 0; k < draws.length; k += DRAW_WORDS) {
      if (draws[k + 4] !== PIPELINE_PUCK_ZONE) continue;
      const block = buffers.get(draws[k]);
      if (!block || draws[k + 1] === 0) continue;
      this.write_uniform(this.zone_uniform, uniforms, draws[k + 3] * UNIFORM_FLOATS, 0);
      let group = this.zone_groups.get(block);
      if (!group) {
        group = this.device.createBindGroup({ layout: this.zone_layout, entries: [{ binding: 0, resource: { buffer: block } }] });
        this.zone_groups.set(block, group);
      }
      pass.setPipeline(this.zone_pipeline);
      pass.setBindGroup(0, this.frame_group);
      pass.setBindGroup(1, this.zone_draw_group);
      pass.setBindGroup(2, group);
      pass.setVertexBuffer(0, this.box.position);
      pass.setVertexBuffer(1, this.box.uv);
      pass.setIndexBuffer(this.box.index, "uint32");
      pass.drawIndexed(36, draws[k + 1]);
    }
  }
}
