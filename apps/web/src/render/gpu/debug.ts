// @pxl/web/render/gpu/debug.ts

/**
 * The dev gizmos through WebGPU: the module's line segments
 * (crates/pxl-core/src/debug.rs) drawn as one line list over everything,
 * with no depth test — a gizmo marks a place whether or not the ground is
 * in front of it. A pixel wide, which is what a line list is; the labels
 * are DOM (marker/debug_layer.tsx).
 */

import type { u32 } from "@xpute/core/kit/word.ts";

const DRAW_WORDS = 5;
const UNIFORM_FLOATS = 8;
const PIPELINE_DEBUG = 9;

export const DEBUG_SHADER: string = /* wgsl */ `
struct Frame {
  view_proj: mat4x4f,
  camera_pos: vec3f,
  edge_strength: f32,
  sun_dir: vec3f,
  low_spec: f32,
  sun_color: vec3f,
  fog_near: f32,
  ambient: vec3f,
  fog_far: f32,
  hemi_sky: vec3f,
  floor: f32,
  hemi_ground: vec3f,
  edge_width: f32,
  fog_color: vec3f,
  _pad: f32,
};
struct Draw {
  translation: vec3f,
  _p: f32,
};
@group(0) @binding(0) var<uniform> frame: Frame;
@group(0) @binding(1) var<uniform> draw: Draw;

struct VertexOut {
  @builtin(position) clip: vec4f,
  @location(0) color: vec3f,
};

@vertex
fn vs(@location(0) position: vec3f, @location(1) color: vec3f) -> VertexOut {
  var o: VertexOut;
  o.clip = frame.view_proj * vec4f(position + draw.translation, 1.0);
  o.color = color;
  return o;
}

fn to_srgb(c: vec3f) -> vec3f {
  let x = clamp(c, vec3f(0.0), vec3f(1.0));
  let lo = x * 12.92;
  let hi = 1.055 * pow(x, vec3f(1.0 / 2.4)) - 0.055;
  return select(hi, lo, x <= vec3f(0.0031308));
}

@fragment
fn fs(i: VertexOut) -> @location(0) vec4f {
  return vec4f(to_srgb(i.color), 1.0);
}
`;

export class DebugPass {
  private readonly pipeline: GPURenderPipeline;
  private readonly draw_buffer: GPUBuffer;
  private readonly group: GPUBindGroup;
  private readonly scratch = new Float32Array(new ArrayBuffer(16));

  constructor(private readonly device: GPUDevice, format: GPUTextureFormat, depth_format: GPUTextureFormat, frame_buffer: GPUBuffer) {
    const module = device.createShaderModule({ label: "debug", code: DEBUG_SHADER });
    this.pipeline = device.createRenderPipeline({
      label: "debug",
      layout: "auto",
      vertex: {
        module,
        entryPoint: "vs",
        buffers: [{
          arrayStride: 24,
          attributes: [{ shaderLocation: 0, offset: 0, format: "float32x3" }, { shaderLocation: 1, offset: 12, format: "float32x3" }],
        }],
      },
      fragment: { module, entryPoint: "fs", targets: [{ format }] },
      primitive: { topology: "line-list" },
      depthStencil: { format: depth_format, depthWriteEnabled: false, depthCompare: "always" },
    });
    this.draw_buffer = device.createBuffer({ label: "debug/draw", size: 16, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST });
    this.group = device.createBindGroup({
      layout: this.pipeline.getBindGroupLayout(0),
      entries: [{ binding: 0, resource: { buffer: frame_buffer } }, { binding: 1, resource: { buffer: this.draw_buffer } }],
    });
  }

  /** The frame's segments into the open pass; how many vertices. */
  draw(pass: GPURenderPassEncoder, draws: Uint32Array, uniforms: Float32Array, buffers: ReadonlyMap<u32, GPUBuffer>): u32 {
    for (let k = 0; k < draws.length; k += DRAW_WORDS) {
      if (draws[k + 4] !== PIPELINE_DEBUG) continue;
      const block = buffers.get(draws[k]);
      const count = draws[k + 1];
      if (!block || count === 0) return 0;
      const at = draws[k + 3] * UNIFORM_FLOATS;
      this.scratch[0] = uniforms[at];
      this.scratch[1] = uniforms[at + 1];
      this.scratch[2] = uniforms[at + 2];
      this.device.queue.writeBuffer(this.draw_buffer, 0, this.scratch);
      pass.setPipeline(this.pipeline);
      pass.setBindGroup(0, this.group);
      pass.setVertexBuffer(0, block, 0, count * 24);
      pass.draw(count);
      return count;
    }
    return 0;
  }
}
