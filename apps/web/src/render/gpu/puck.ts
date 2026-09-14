// @pxl/web/render/gpu/puck.ts

/**
 * The location puck through WebGPU: the module's zone and beacon draws
 * (crates/pxl-core/src/marker.rs). The zone is a unit box a cell, instanced
 * from the zone block, drawn unlit and half transparent with a border on the
 * edges the ring's rim runs along — location_puck.tsx's shader, which was
 * deliberately unlit so the zone reads as an indicator rather than a surface.
 * The beacon is a small flat-shaded octahedron in the day or night colour,
 * glowing as its emissive material did.
 */

import type { u32 } from "@xpute/core/kit/word.ts";

import { marker_colors } from "@/module/world/marker/style.ts";
import { GRADE_WGSL } from "./grade.ts";
import { DEPTH_COMPARE } from "./depth.ts";
import { FRAME_WGSL as FRAME_STRUCT_WGSL, SHADE_WGSL } from "./frame_wgsl.ts";
import { linear_rgb as linear } from "./color.ts";

const DRAW_WORDS = 5;
const UNIFORM_FLOATS = 8;
const PIPELINE_PUCK_ZONE = 6;
const PIPELINE_PUCK_BEACON = 7;

const ZONE_FILL_OPACITY = 0.4;
const ZONE_BORDER_WIDTH = 0.04;
const BEACON_RADIUS = 0.3;
const BEACON_DAY = "#ff1493";
const BEACON_NIGHT = "#00f2ff";

const wgsl_vec3 = (c: [number, number, number]) => `vec3f(${c.map((v) => v.toFixed(6)).join(", ")})`;

const FRAME_WGSL = /* wgsl */ `
${FRAME_STRUCT_WGSL}
struct Draw {
  translation: vec3f,
  fourth: f32,
  night: f32,
  _p0: f32,
  _p1: f32,
  _p2: f32,
};
@group(0) @binding(0) var<uniform> frame: Frame;
@group(0) @binding(1) var<uniform> draw: Draw;
`;

export const ZONE_SHADER: string = /* wgsl */ `
${FRAME_WGSL}
@group(1) @binding(0) var<storage, read> zone: array<f32>;

struct VertexIn {
  @builtin(instance_index) i: u32,
  @location(0) position: vec3f,
  @location(1) uv: vec2f,
};
struct VertexOut {
  @builtin(position) clip: vec4f,
  @location(0) uv: vec2f,
  @location(1) @interpolate(flat) borders: vec4f,
};

@vertex
fn vs(v: VertexIn) -> VertexOut {
  let k = v.i * 7u;
  let centre = vec3f(zone[k], zone[k + 1u], zone[k + 2u]);
  var o: VertexOut;
  o.clip = frame.view_proj * vec4f(v.position + centre + draw.translation, 1.0);
  o.uv = v.uv;
  o.borders = vec4f(zone[k + 3u], zone[k + 4u], zone[k + 5u], zone[k + 6u]);
  return o;
}

${GRADE_WGSL}
${SHADE_WGSL}

fn aastep(thres: f32, value: f32) -> f32 {
  let w = fwidth(value) * 0.5;
  return smoothstep(thres - w, thres + w, value);
}

@fragment
fn fs(i: VertexOut) -> @location(0) vec4f {
  let bw = ${ZONE_BORDER_WIDTH};
  let top = aastep(1.0 - bw, i.uv.y) * i.borders.x;
  let bottom = (1.0 - aastep(bw, i.uv.y)) * i.borders.y;
  let right = aastep(1.0 - bw, i.uv.x) * i.borders.z;
  let left = (1.0 - aastep(bw, i.uv.x)) * i.borders.w;
  let border = max(max(top, bottom), max(left, right));
  let colour = ${wgsl_vec3(linear(marker_colors.ACTION))};
  return vec4f(finish(colour, i.clip.xy), mix(${ZONE_FILL_OPACITY}, 1.0, border));
}
`;

export const BEACON_SHADER: string = /* wgsl */ `
${FRAME_WGSL}

struct VertexOut {
  @builtin(position) clip: vec4f,
  @location(0) normal: vec3f,
};

fn spin(v: vec3f) -> vec3f {
  let c = cos(draw.fourth);
  let s = sin(draw.fourth);
  return vec3f(v.x * c - v.y * s, v.x * s + v.y * c, v.z);
}

@vertex
fn vs(@location(0) position: vec3f, @location(1) normal: vec3f) -> VertexOut {
  var o: VertexOut;
  o.clip = frame.view_proj * vec4f(spin(position) + draw.translation, 1.0);
  o.normal = spin(normal);
  return o;
}

${GRADE_WGSL}
${SHADE_WGSL}

@fragment
fn fs(i: VertexOut) -> @location(0) vec4f {
  let colour = select(${wgsl_vec3(linear(BEACON_DAY))}, ${wgsl_vec3(linear(BEACON_NIGHT))}, draw.night > 0.5);
  let glow = select(0.25, 1.0, draw.night > 0.5);
  let n = normalize(i.normal);
  // Mostly metal, which three lit with no environment to reflect: a little
  // diffuse under the lights, and the emissive glow.
  let lit = colour * (frame.sun_color * max(dot(n, frame.sun_dir), 0.0) + frame.ambient) / 3.14159265 * 0.2;
  return vec4f(finish(lit + colour * glow, i.clip.xy), 1.0);
}
`;

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
  private readonly zone_frame_group: GPUBindGroup;
  private readonly beacon_frame_group: GPUBindGroup;
  private readonly box: { position: GPUBuffer; uv: GPUBuffer; index: GPUBuffer };
  private readonly octahedron: { position: GPUBuffer; normal: GPUBuffer };
  private readonly zone_groups = new WeakMap<GPUBuffer, GPUBindGroup>();
  private readonly scratch = new Float32Array(new ArrayBuffer(32));

  constructor(private readonly device: GPUDevice, format: GPUTextureFormat, depth_format: GPUTextureFormat, frame_buffer: GPUBuffer, sample_count: u32) {
    const blend: GPUBlendState = {
      color: { srcFactor: "src-alpha", dstFactor: "one-minus-src-alpha", operation: "add" },
      alpha: { srcFactor: "one", dstFactor: "one-minus-src-alpha", operation: "add" },
    };
    const zone_module = device.createShaderModule({ label: "puck/zone", code: ZONE_SHADER });
    this.zone_pipeline = device.createRenderPipeline({
      label: "puck/zone",
      layout: "auto",
      vertex: {
        module: zone_module,
        entryPoint: "vs",
        buffers: [
          { arrayStride: 12, attributes: [{ shaderLocation: 0, offset: 0, format: "float32x3" }] },
          { arrayStride: 8, attributes: [{ shaderLocation: 1, offset: 0, format: "float32x2" }] },
        ],
      },
      fragment: { module: zone_module, entryPoint: "fs", targets: [{ format, blend }] },
      primitive: { topology: "triangle-list", cullMode: "none" },
      depthStencil: { format: depth_format, depthWriteEnabled: false, depthCompare: DEPTH_COMPARE, depthBias: 4, depthBiasSlopeScale: 1 },
      multisample: { count: sample_count },
    });
    const beacon_module = device.createShaderModule({ label: "puck/beacon", code: BEACON_SHADER });
    this.beacon_pipeline = device.createRenderPipeline({
      label: "puck/beacon",
      layout: "auto",
      vertex: {
        module: beacon_module,
        entryPoint: "vs",
        buffers: [
          { arrayStride: 12, attributes: [{ shaderLocation: 0, offset: 0, format: "float32x3" }] },
          { arrayStride: 12, attributes: [{ shaderLocation: 1, offset: 0, format: "float32x3" }] },
        ],
      },
      fragment: { module: beacon_module, entryPoint: "fs", targets: [{ format }] },
      primitive: { topology: "triangle-list", cullMode: "back", frontFace: "ccw" },
      depthStencil: { format: depth_format, depthWriteEnabled: true, depthCompare: DEPTH_COMPARE },
      multisample: { count: sample_count },
    });
    const uniform = (label: string) => device.createBuffer({ label, size: 32, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST });
    this.zone_uniform = uniform("puck/zone/draw");
    this.beacon_uniform = uniform("puck/beacon/draw");
    const frame_group = (pipeline: GPURenderPipeline, draw: GPUBuffer) =>
      device.createBindGroup({ layout: pipeline.getBindGroupLayout(0), entries: [{ binding: 0, resource: { buffer: frame_buffer } }, { binding: 1, resource: { buffer: draw } }] });
    this.zone_frame_group = frame_group(this.zone_pipeline, this.zone_uniform);
    this.beacon_frame_group = frame_group(this.beacon_pipeline, this.beacon_uniform);
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
    s[4] = night;
    this.device.queue.writeBuffer(buffer, 0, s);
  }

  /** The beacon, opaque; before anything blended. */
  draw_beacon(pass: GPURenderPassEncoder, draws: Uint32Array, uniforms: Float32Array): void {
    for (let k = 0; k < draws.length; k += DRAW_WORDS) {
      if (draws[k + 4] !== PIPELINE_PUCK_BEACON) continue;
      this.write_uniform(this.beacon_uniform, uniforms, draws[k + 3] * UNIFORM_FLOATS, draws[k + 1]);
      pass.setPipeline(this.beacon_pipeline);
      pass.setBindGroup(0, this.beacon_frame_group);
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
        group = this.device.createBindGroup({ layout: this.zone_pipeline.getBindGroupLayout(1), entries: [{ binding: 0, resource: { buffer: block } }] });
        this.zone_groups.set(block, group);
      }
      pass.setPipeline(this.zone_pipeline);
      pass.setBindGroup(0, this.zone_frame_group);
      pass.setBindGroup(1, group);
      pass.setVertexBuffer(0, this.box.position);
      pass.setVertexBuffer(1, this.box.uv);
      pass.setIndexBuffer(this.box.index, "uint32");
      pass.drawIndexed(36, draws[k + 1]);
    }
  }
}
