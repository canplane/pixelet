// @pxl/web/render/gpu/skinned.ts

/**
 * Actors through WebGPU: the module's skinned draws
 * (crates/pxl-core/src/model.rs) executed against each model's mesh block
 * and each actor's joint palette, both as the module wrote them. The vertex
 * shader skins a vertex by its four joints and weights out of the palette,
 * scales it to the model's height, turns glTF's Y-up to this world's Z-up
 * and the actor's heading about Z, and places it; the fragment shader is
 * make_lit_gltf_material's recipe, decor's.
 *
 * A model's skin is the glue's, as decor's are: handed over by model id.
 */

import type { u32 } from "@xpute/core/kit/word.ts";

import { GRADE_WGSL } from "./grade.ts";
import { FRAME_WGSL, SHADE_WGSL, SHADOW_WGSL } from "./frame_wgsl.ts";
import { DEPTH_COMPARE } from "./depth.ts";
import { SHADOW_FORMAT, ShadowPass } from "./shadow.ts";

const DRAW_WORDS = 5;
const UNIFORM_FLOATS = 8;
const PIPELINE_SKINNED = 5;
const DRAW_UNIFORM_BYTES = 32;
/** model.rs's MODEL_VERTEX_WORDS. */
const MODEL_VERTEX_WORDS = 16;

export const SKINNED_SHADER: string = /* wgsl */ `
${FRAME_WGSL}

struct Draw {
  translation: vec3f,
  rotation: f32,
  opacity: f32,
  scale: f32,
  _p0: f32,
  _p1: f32,
};

@group(0) @binding(0) var<uniform> frame: Frame;
@group(1) @binding(0) var<uniform> draw: Draw;
@group(2) @binding(0) var skin: texture_2d<f32>;
@group(2) @binding(1) var skin_sampler: sampler;
@group(3) @binding(0) var<storage, read> palette: array<mat4x4f>;

struct VertexIn {
  @location(0) position: vec3f,
  @location(1) normal: vec3f,
  @location(2) uv: vec2f,
  @location(3) joints: vec4<u32>,
  @location(4) weights: vec4f,
};

struct VertexOut {
  @builtin(position) clip: vec4f,
  @location(0) uv: vec2f,
  @location(1) normal: vec3f,
  @location(2) world_pos: vec3f,
};

fn place(v: vec3f) -> vec3f {
  let up = vec3f(v.x, -v.z, v.y);
  let c = cos(draw.rotation);
  let s = sin(draw.rotation);
  return vec3f(up.x * c - up.y * s, up.x * s + up.y * c, up.z);
}

@vertex
fn vs(v: VertexIn) -> VertexOut {
  let m = palette[v.joints.x] * v.weights.x + palette[v.joints.y] * v.weights.y + palette[v.joints.z] * v.weights.z + palette[v.joints.w] * v.weights.w;
  let p = (m * vec4f(v.position, 1.0)).xyz * draw.scale;
  let n = (m * vec4f(v.normal, 0.0)).xyz;
  let world = place(p) + draw.translation;
  var o: VertexOut;
  o.clip = frame.view_proj * vec4f(world, 1.0);
  o.uv = v.uv;
  o.normal = place(n);
  o.world_pos = world;
  return o;
}

${GRADE_WGSL}
${SHADE_WGSL}
${SHADOW_WGSL(0, 1)}

@fragment
fn fs(i: VertexOut) -> @location(0) vec4f {
  let albedo = textureSample(skin, skin_sampler, i.uv).rgb;
  let n = normalize(i.normal);
  let color = shade(albedo, n, i.world_pos, sun_lit(i.world_pos, n));
  return vec4f(finish(color, i.clip.xy), draw.opacity);
}
`;

export type ModelSkin = ImageBitmap | HTMLImageElement | HTMLCanvasElement;

export class SkinnedPass {
  private readonly pipeline: GPURenderPipeline;
  private readonly depth_pipeline: GPURenderPipeline;
  private readonly frame_group: GPUBindGroup;
  private readonly light_group: GPUBindGroup;
  private readonly draw_layout: GPUBindGroupLayout;
  private readonly skin_layout: GPUBindGroupLayout;
  private readonly palette_layout: GPUBindGroupLayout;
  private readonly sampler: GPUSampler;
  private readonly stride: u32;
  private draw_buffer: GPUBuffer;
  private draw_group: GPUBindGroup;
  private draw_scratch: ArrayBuffer;
  private readonly skins = new Map<u32, { image: ModelSkin; group: GPUBindGroup }>();
  private readonly palette_groups = new WeakMap<GPUBuffer, GPUBindGroup>();

  constructor(private readonly device: GPUDevice, format: GPUTextureFormat, depth_format: GPUTextureFormat, frame_buffer: GPUBuffer, shadow: ShadowPass | null, sample_count: u32) {
    const frame_layout = device.createBindGroupLayout({
      entries: [{ binding: 0, visibility: GPUShaderStage.VERTEX | GPUShaderStage.FRAGMENT, buffer: { type: "uniform" } }, ...ShadowPass.layout_entries(1)],
    });
    this.draw_layout = device.createBindGroupLayout({ entries: [{ binding: 0, visibility: GPUShaderStage.VERTEX | GPUShaderStage.FRAGMENT, buffer: { type: "uniform", hasDynamicOffset: true } }] });
    this.skin_layout = device.createBindGroupLayout({
      entries: [
        { binding: 0, visibility: GPUShaderStage.FRAGMENT, texture: {} },
        { binding: 1, visibility: GPUShaderStage.FRAGMENT, sampler: {} },
      ],
    });
    this.palette_layout = device.createBindGroupLayout({ entries: [{ binding: 0, visibility: GPUShaderStage.VERTEX, buffer: { type: "read-only-storage" } }] });
    const module = device.createShaderModule({ label: "skinned", code: SKINNED_SHADER });
    const vertex: GPUVertexState = {
      module,
      entryPoint: "vs",
      buffers: [
        { arrayStride: 12, attributes: [{ shaderLocation: 0, offset: 0, format: "float32x3" }] },
        { arrayStride: 12, attributes: [{ shaderLocation: 1, offset: 0, format: "float32x3" }] },
        { arrayStride: 8, attributes: [{ shaderLocation: 2, offset: 0, format: "float32x2" }] },
        { arrayStride: 16, attributes: [{ shaderLocation: 3, offset: 0, format: "uint32x4" }] },
        { arrayStride: 16, attributes: [{ shaderLocation: 4, offset: 0, format: "float32x4" }] },
      ],
    };
    this.pipeline = device.createRenderPipeline({
      label: "skinned",
      layout: device.createPipelineLayout({ bindGroupLayouts: [frame_layout, this.draw_layout, this.skin_layout, this.palette_layout] }),
      vertex,
      fragment: { module, entryPoint: "fs", targets: [{ format }] },
      // Back faces culled, as three's material (FrontSide) did: drawn both
      // sides, a closed body's inner faces fight its outer ones for the same
      // depth at a distance, and the character shimmers.
      primitive: { topology: "triangle-list", cullMode: "back", frontFace: "ccw" },
      depthStencil: { format: depth_format, depthWriteEnabled: true, depthCompare: DEPTH_COMPARE },
      multisample: { count: sample_count },
    });
    this.depth_pipeline = device.createRenderPipeline({
      label: "skinned/depth",
      layout: device.createPipelineLayout({ bindGroupLayouts: [frame_layout, this.draw_layout, this.skin_layout, this.palette_layout] }),
      vertex,
      primitive: { topology: "triangle-list", cullMode: "none" },
      depthStencil: { format: SHADOW_FORMAT, depthWriteEnabled: true, depthCompare: "less" },
    });
    // A case (preview.ts) draws with no shadow map: a blank one stands
    // where the map would be, and every fragment counts as lit.
    const shadow_entries = shadow ? shadow.group_entries(1) : ShadowPass.blank_entries(device, 1);
    this.frame_group = device.createBindGroup({ layout: frame_layout, entries: [{ binding: 0, resource: { buffer: frame_buffer } }, ...shadow_entries] });
    this.light_group = shadow ? device.createBindGroup({ layout: frame_layout, entries: [{ binding: 0, resource: { buffer: shadow.light_frame } }, ...shadow.light_entries(1)] }) : this.frame_group;
    this.sampler = device.createSampler({ magFilter: "linear", minFilter: "linear", addressModeU: "repeat", addressModeV: "repeat" });
    this.stride = Math.max(DRAW_UNIFORM_BYTES, device.limits.minUniformBufferOffsetAlignment);
    this.draw_scratch = new ArrayBuffer(this.stride * 8);
    this.draw_buffer = device.createBuffer({ label: "skinned/draws", size: this.stride * 8, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST });
    this.draw_group = this.make_draw_group();
  }

  private make_draw_group(): GPUBindGroup {
    return this.device.createBindGroup({ layout: this.draw_layout, entries: [{ binding: 0, resource: { buffer: this.draw_buffer, size: DRAW_UNIFORM_BYTES } }] });
  }

  /** Model `model`'s skin; the same image again changes nothing. */
  set_skin(model: u32, image: ModelSkin): void {
    if (this.skins.get(model)?.image === image) return;
    const d = this.device;
    const texture = d.createTexture({
      size: [image.width, image.height],
      format: "rgba8unorm-srgb",
      usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST | GPUTextureUsage.RENDER_ATTACHMENT,
    });
    d.queue.copyExternalImageToTexture({ source: image }, { texture }, [image.width, image.height]);
    const group = d.createBindGroup({ layout: this.skin_layout, entries: [{ binding: 0, resource: texture.createView() }, { binding: 1, resource: this.sampler }] });
    this.skins.set(model, { image, group });
  }

  /** Draws the frame's skinned draws into the open pass. `counts` names each
   * model block's vertex and index counts, as their uploads said. */
  draw(pass: GPURenderPassEncoder, draws: Uint32Array, uniforms: Float32Array, buffers: ReadonlyMap<u32, GPUBuffer>, counts: ReadonlyMap<u32, [u32, u32]>): u32 {
    let n = 0;
    for (let k = 0; k < draws.length; k += DRAW_WORDS) if (draws[k + 4] === PIPELINE_SKINNED) n++;
    if (n === 0) return 0;
    if (n * this.stride > this.draw_buffer.size) {
      this.draw_buffer.destroy();
      const size = this.stride * (1 << Math.ceil(Math.log2(n)));
      this.draw_buffer = this.device.createBuffer({ label: "skinned/draws", size, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST });
      this.draw_scratch = new ArrayBuffer(size);
      this.draw_group = this.make_draw_group();
    }
    const f = new Float32Array(this.draw_scratch);
    let slot = 0;
    for (let k = 0; k < draws.length; k += DRAW_WORDS) {
      if (draws[k + 4] !== PIPELINE_SKINNED) continue;
      const at = (slot++ * this.stride) >> 2;
      const uniform = draws[k + 3] * UNIFORM_FLOATS;
      for (let i = 0; i < 6; i++) f[at + i] = uniforms[uniform + i];
    }
    this.device.queue.writeBuffer(this.draw_buffer, 0, this.draw_scratch, 0, n * this.stride);
    return this.draw_pipeline(pass, draws, buffers, counts, this.pipeline, this.frame_group);
  }

  /** The actors' depth from the light, after `draw` wrote the uniforms. */
  draw_depth(pass: GPURenderPassEncoder, draws: Uint32Array, buffers: ReadonlyMap<u32, GPUBuffer>, counts: ReadonlyMap<u32, [u32, u32]>): u32 {
    return this.draw_pipeline(pass, draws, buffers, counts, this.depth_pipeline, this.light_group);
  }

  private draw_pipeline(
    pass: GPURenderPassEncoder,
    draws: Uint32Array,
    buffers: ReadonlyMap<u32, GPUBuffer>,
    counts: ReadonlyMap<u32, [u32, u32]>,
    pipeline: GPURenderPipeline,
    frame_group: GPUBindGroup,
  ): u32 {
    let slot = 0;
    let drawn = 0;
    for (let k = 0; k < draws.length; k += DRAW_WORDS) {
      if (draws[k + 4] !== PIPELINE_SKINNED) continue;
      const offset = slot++ * this.stride;
      const mesh = buffers.get(draws[k]);
      const palette = buffers.get(draws[k + 1]);
      const skin = this.skins.get(draws[k + 2]);
      const count = counts.get(draws[k]);
      if (!mesh || !palette || !skin || !count) continue;
      if (drawn === 0) {
        pass.setPipeline(pipeline);
        pass.setBindGroup(0, frame_group);
      }
      let palette_group = this.palette_groups.get(palette);
      if (!palette_group) {
        palette_group = this.device.createBindGroup({ layout: this.palette_layout, entries: [{ binding: 0, resource: { buffer: palette } }] });
        this.palette_groups.set(palette, palette_group);
      }
      const [vertex_cnt, index_cnt] = count;
      const lane = (words: u32) => words * vertex_cnt * 4;
      pass.setBindGroup(1, this.draw_group, [offset]);
      pass.setBindGroup(2, skin.group);
      pass.setBindGroup(3, palette_group);
      pass.setVertexBuffer(0, mesh, lane(0), lane(3));
      pass.setVertexBuffer(1, mesh, lane(3), lane(3));
      pass.setVertexBuffer(2, mesh, lane(6), lane(2));
      pass.setVertexBuffer(3, mesh, lane(8), lane(4));
      pass.setVertexBuffer(4, mesh, lane(12), lane(4));
      pass.setIndexBuffer(mesh, "uint32", lane(MODEL_VERTEX_WORDS), index_cnt * 4);
      pass.drawIndexed(index_cnt);
      drawn++;
    }
    return drawn;
  }
}
