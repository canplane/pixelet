// @pxl/web/render/gpu/skinned.ts

/**
 * Actors through WebGPU: the module's skinned draws
 * (crates/pxl-core/src/world/model.rs) executed against each model's mesh block
 * and each actor's joint palette, both as the module wrote them, under
 * shader/skinned.wgsl. A model's skin is the glue's, as decor's are:
 * handed over by model id.
 */

import type { u32 } from "@xpute/core/kit/word.ts";

import { PIPELINE_SKINNED_DEPTH, type PipelineBuilder, type PipelineRecord } from "./pipeline.ts";
import { ShadowPass } from "./shadow.ts";

const DRAW_WORDS = 5;
const UNIFORM_FLOATS = 8;
const PIPELINE_SKINNED = 5;
const DRAW_UNIFORM_BYTES = 32;
/** model.rs's MODEL_VERTEX_WORDS. */
const MODEL_VERTEX_WORDS = 16;

export type ModelSkin = ImageBitmap | HTMLImageElement | HTMLCanvasElement;

export class SkinnedPass {
  private readonly pipeline: GPURenderPipeline;
  private readonly depth_pipeline: GPURenderPipeline;
  private readonly record: PipelineRecord;
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

  constructor(private readonly device: GPUDevice, pipelines: PipelineBuilder, format: GPUTextureFormat, frame_buffer: GPUBuffer, shadow: ShadowPass | null, sample_count: u32) {
    const lit = pipelines.build(PIPELINE_SKINNED, "skinned", format, sample_count);
    this.pipeline = lit.pipeline;
    this.record = lit.record;
    this.depth_pipeline = pipelines.build(PIPELINE_SKINNED_DEPTH, "skinned/depth", format, sample_count).pipeline;
    const [frame_layout, draw_layout, skin_layout, palette_layout] = lit.layouts;
    this.draw_layout = draw_layout;
    this.skin_layout = skin_layout;
    this.palette_layout = palette_layout;
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
      pass.setBindGroup(1, this.draw_group, [offset]);
      pass.setBindGroup(2, skin.group);
      pass.setBindGroup(3, palette_group);
      let at = 0;
      this.record.buffers.forEach((lane, slot) => {
        pass.setVertexBuffer(slot, mesh, at, vertex_cnt * lane.stride);
        at += vertex_cnt * lane.stride;
      });
      pass.setIndexBuffer(mesh, "uint32", vertex_cnt * MODEL_VERTEX_WORDS * 4, index_cnt * 4);
      pass.drawIndexed(index_cnt);
      drawn++;
    }
    return drawn;
  }
}
