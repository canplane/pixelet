// @pxl/web/render/gpu/decor.ts

/**
 * Decor through WebGPU: the module's decor draws (crates/pxl-core/src/draw.rs)
 * executed as instanced draws of each variant's model, the instances read
 * straight out of the decor block the module built (decor.rs's layout) as
 * it lies on the GPU. What decor/field.ts did for three — a matrix per
 * instance in a buffer per variant, runs per chunk, compaction, rebasing —
 * has nothing to do here: the block is the instance buffer, the chunk's
 * translation is its uniform, and the module says which block a chunk shows.
 *
 * The models and skins are the glue's, as the texture pack is: loaded by
 * decor/asset_store.ts and handed over by group — every bundle's variants in
 * bundle order, the order the kinds were taught.
 *
 * The shader is make_lit_gltf_material's recipe: the skin under three's
 * Lambert (albedo over pi) for the sun, the ambient and the hemisphere, the
 * skin again as the emissive floor, fog, and the grade. An instance is its
 * position, its spin about Z after the glTF Y-up to Z-up turn, and its
 * scale, composed as instanced_model.ts's compose_instance_matrix did; a
 * hidden one is sent outside the clip volume.
 */

import type { u32 } from "@xpute/core/kit/word.ts";

import { light_cfg } from "@/module/world/environment/config.ts";
import { GRADE_WGSL } from "./grade.ts";

/** Words and floats of draw.rs's records the decor pass reads. */
const DRAW_WORDS = 5;
const UNIFORM_FLOATS = 8;
const PIPELINE_DECOR = 1;
/** A decor draw's dynamic uniform: translation (3), the group's first lane
 * word, the instance count, padding. */
const DRAW_UNIFORM_BYTES = 32;

export type DecorModel = {
  position: Float32Array<ArrayBuffer>;
  normal: Float32Array<ArrayBuffer>;
  uv: Float32Array<ArrayBuffer>;
  index: Uint32Array<ArrayBuffer>;
  skin: ImageBitmap | HTMLImageElement | HTMLCanvasElement;
};

type GpuModel = {
  position: GPUBuffer;
  normal: GPUBuffer;
  uv: GPUBuffer;
  index: GPUBuffer;
  index_cnt: u32;
  skin: GPUTexture;
  skin_group: GPUBindGroup;
};

export const DECOR_SHADER: string = /* wgsl */ `
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
  offset: u32,
  count: u32,
  _p0: u32,
  _p1: u32,
  _p2: u32,
};

@group(0) @binding(0) var<uniform> frame: Frame;
@group(1) @binding(0) var<uniform> draw: Draw;
@group(2) @binding(0) var skin: texture_2d<f32>;
@group(2) @binding(1) var skin_sampler: sampler;
@group(3) @binding(0) var<storage, read> block: array<u32>;

struct VertexIn {
  @builtin(instance_index) i: u32,
  @location(0) position: vec3f,
  @location(1) normal: vec3f,
  @location(2) uv: vec2f,
};

struct VertexOut {
  @builtin(position) clip: vec4f,
  @location(0) uv: vec2f,
  @location(1) normal: vec3f,
  @location(2) view_depth: f32,
};

fn lane(k: u32, i: u32) -> f32 {
  return bitcast<f32>(block[draw.offset + k * draw.count + i]);
}

// glTF is Y-up: a quarter turn about X, (x, y, z) to (x, -z, y), then the
// instance's spin about Z.
fn place(v: vec3f, spin: f32) -> vec3f {
  let up = vec3f(v.x, -v.z, v.y);
  let c = cos(spin);
  let s = sin(spin);
  return vec3f(up.x * c - up.y * s, up.x * s + up.y * c, up.z);
}

@vertex
fn vs(v: VertexIn) -> VertexOut {
  let n = draw.count;
  let flags = (block[draw.offset + 5u * n + v.i / 4u] >> ((v.i % 4u) * 8u)) & 255u;
  var o: VertexOut;
  if ((flags & 1u) != 0u) {
    o.clip = vec4f(0.0, 0.0, -2.0, 1.0);
    return o;
  }
  let at = vec3f(lane(0u, v.i), lane(1u, v.i), lane(2u, v.i));
  let spin = lane(3u, v.i);
  let scale = lane(4u, v.i);
  let world = place(v.position * scale, spin) + at + draw.translation;
  o.clip = frame.view_proj * vec4f(world, 1.0);
  o.uv = v.uv;
  o.normal = place(v.normal, spin);
  o.view_depth = length(world - frame.camera_pos);
  return o;
}

${GRADE_WGSL}

@fragment
fn fs(i: VertexOut) -> @location(0) vec4f {
  let albedo = textureSample(skin, skin_sampler, i.uv).rgb;
  let n = normalize(i.normal);
  let sun = frame.sun_color * max(dot(n, frame.sun_dir), 0.0);
  let hemi = mix(frame.hemi_ground, frame.hemi_sky, n.z * 0.5 + 0.5);
  var color = albedo * (sun + frame.ambient + hemi) / 3.14159265 + albedo * ${light_cfg.FLOOR};
  let fog = clamp((i.view_depth - frame.fog_near) / (frame.fog_far - frame.fog_near), 0.0, 1.0);
  color = mix(color, frame.fog_color, fog);
  return vec4f(to_srgb(grade(color)), 1.0);
}
`;

export class DecorPass {
  private readonly pipeline: GPURenderPipeline;
  private readonly frame_group: GPUBindGroup;
  private readonly draw_layout: GPUBindGroupLayout;
  private readonly skin_layout: GPUBindGroupLayout;
  private readonly block_layout: GPUBindGroupLayout;
  private readonly sampler: GPUSampler;
  private readonly stride: u32;
  private draw_buffer: GPUBuffer;
  private draw_group: GPUBindGroup;
  private draw_scratch: ArrayBuffer;
  private models: (GpuModel | null)[] = [];
  private sources: readonly (DecorModel | null)[] = [];
  private readonly block_groups = new WeakMap<GPUBuffer, GPUBindGroup>();

  constructor(private readonly device: GPUDevice, format: GPUTextureFormat, depth_format: GPUTextureFormat, frame_buffer: GPUBuffer) {
    const frame_layout = device.createBindGroupLayout({ entries: [{ binding: 0, visibility: GPUShaderStage.VERTEX | GPUShaderStage.FRAGMENT, buffer: { type: "uniform" } }] });
    this.draw_layout = device.createBindGroupLayout({ entries: [{ binding: 0, visibility: GPUShaderStage.VERTEX, buffer: { type: "uniform", hasDynamicOffset: true } }] });
    this.skin_layout = device.createBindGroupLayout({
      entries: [
        { binding: 0, visibility: GPUShaderStage.FRAGMENT, texture: {} },
        { binding: 1, visibility: GPUShaderStage.FRAGMENT, sampler: {} },
      ],
    });
    this.block_layout = device.createBindGroupLayout({ entries: [{ binding: 0, visibility: GPUShaderStage.VERTEX, buffer: { type: "read-only-storage" } }] });
    const module = device.createShaderModule({ label: "decor", code: DECOR_SHADER });
    this.pipeline = device.createRenderPipeline({
      label: "decor",
      layout: device.createPipelineLayout({ bindGroupLayouts: [frame_layout, this.draw_layout, this.skin_layout, this.block_layout] }),
      vertex: {
        module,
        entryPoint: "vs",
        buffers: [
          { arrayStride: 12, attributes: [{ shaderLocation: 0, offset: 0, format: "float32x3" }] },
          { arrayStride: 12, attributes: [{ shaderLocation: 1, offset: 0, format: "float32x3" }] },
          { arrayStride: 8, attributes: [{ shaderLocation: 2, offset: 0, format: "float32x2" }] },
        ],
      },
      fragment: { module, entryPoint: "fs", targets: [{ format }] },
      primitive: { topology: "triangle-list", cullMode: "none" },
      depthStencil: { format: depth_format, depthWriteEnabled: true, depthCompare: "less" },
    });
    this.frame_group = device.createBindGroup({ layout: frame_layout, entries: [{ binding: 0, resource: { buffer: frame_buffer } }] });
    this.sampler = device.createSampler({ magFilter: "linear", minFilter: "linear", addressModeU: "repeat", addressModeV: "repeat" });
    this.stride = Math.max(DRAW_UNIFORM_BYTES, device.limits.minUniformBufferOffsetAlignment);
    this.draw_scratch = new ArrayBuffer(this.stride * 256);
    this.draw_buffer = device.createBuffer({ label: "decor/draws", size: this.stride * 256, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST });
    this.draw_group = this.make_draw_group();
  }

  private make_draw_group(): GPUBindGroup {
    return this.device.createBindGroup({ layout: this.draw_layout, entries: [{ binding: 0, resource: { buffer: this.draw_buffer, size: DRAW_UNIFORM_BYTES } }] });
  }

  /** The models by group, as the assets stand; a group still loading is null. */
  set_models(sources: readonly (DecorModel | null)[]): void {
    for (let g = 0; g < sources.length; g++) {
      if (sources[g] === (this.sources[g] ?? null)) continue;
      this.models[g] = sources[g] ? this.upload(sources[g]!) : null;
    }
    this.sources = sources;
  }

  private upload(m: DecorModel): GpuModel {
    const d = this.device;
    const buffer = (data: ArrayBufferView<ArrayBuffer>, usage: GPUBufferUsageFlags) => {
      const b = d.createBuffer({ size: Math.max(4, (data.byteLength + 3) & ~3), usage: usage | GPUBufferUsage.COPY_DST });
      d.queue.writeBuffer(b, 0, data.buffer, data.byteOffset, data.byteLength);
      return b;
    };
    const w = m.skin.width;
    const h = m.skin.height;
    const skin = d.createTexture({ size: [w, h], format: "rgba8unorm-srgb", usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST | GPUTextureUsage.RENDER_ATTACHMENT });
    d.queue.copyExternalImageToTexture({ source: m.skin }, { texture: skin }, [w, h]);
    return {
      position: buffer(m.position, GPUBufferUsage.VERTEX),
      normal: buffer(m.normal, GPUBufferUsage.VERTEX),
      uv: buffer(m.uv, GPUBufferUsage.VERTEX),
      index: buffer(m.index, GPUBufferUsage.INDEX),
      index_cnt: m.index.length,
      skin,
      skin_group: d.createBindGroup({ layout: this.skin_layout, entries: [{ binding: 0, resource: skin.createView() }, { binding: 1, resource: this.sampler }] }),
    };
  }

  /** Draws the frame's decor draws into the open pass. */
  draw(pass: GPURenderPassEncoder, draws: Uint32Array, uniforms: Float32Array, buffers: ReadonlyMap<u32, GPUBuffer>): u32 {
    let n = 0;
    for (let k = 0; k < draws.length; k += DRAW_WORDS) if (draws[k + 4] === PIPELINE_DECOR) n++;
    if (n === 0) return 0;
    if (n * this.stride > this.draw_buffer.size) {
      this.draw_buffer.destroy();
      const size = this.stride * (1 << Math.ceil(Math.log2(n)));
      this.draw_buffer = this.device.createBuffer({ label: "decor/draws", size, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST });
      this.draw_scratch = new ArrayBuffer(size);
      this.draw_group = this.make_draw_group();
    }
    const f = new Float32Array(this.draw_scratch);
    const u = new Uint32Array(this.draw_scratch);
    let slot = 0;
    for (let k = 0; k < draws.length; k += DRAW_WORDS) {
      if (draws[k + 4] !== PIPELINE_DECOR) continue;
      const at = (slot * this.stride) >> 2;
      const uniform = draws[k + 3] * UNIFORM_FLOATS;
      f[at] = uniforms[uniform];
      f[at + 1] = uniforms[uniform + 1];
      f[at + 2] = uniforms[uniform + 2];
      u[at + 3] = uniforms[uniform + 3];
      u[at + 4] = draws[k + 2];
      slot++;
    }
    this.device.queue.writeBuffer(this.draw_buffer, 0, this.draw_scratch, 0, n * this.stride);

    pass.setPipeline(this.pipeline);
    pass.setBindGroup(0, this.frame_group);
    slot = 0;
    let drawn = 0;
    for (let k = 0; k < draws.length; k += DRAW_WORDS) {
      if (draws[k + 4] !== PIPELINE_DECOR) continue;
      const offset = slot++ * this.stride;
      const model = this.models[draws[k + 1]];
      const block = buffers.get(draws[k]);
      if (!model || !block) continue;
      let block_group = this.block_groups.get(block);
      if (!block_group) {
        block_group = this.device.createBindGroup({ layout: this.block_layout, entries: [{ binding: 0, resource: { buffer: block } }] });
        this.block_groups.set(block, block_group);
      }
      pass.setBindGroup(1, this.draw_group, [offset]);
      pass.setBindGroup(2, model.skin_group);
      pass.setBindGroup(3, block_group);
      pass.setVertexBuffer(0, model.position);
      pass.setVertexBuffer(1, model.normal);
      pass.setVertexBuffer(2, model.uv);
      pass.setIndexBuffer(model.index, "uint32");
      pass.drawIndexed(model.index_cnt, draws[k + 2]);
      drawn++;
    }
    return drawn;
  }
}
