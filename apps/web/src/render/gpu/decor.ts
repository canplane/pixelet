// @pxl/web/render/gpu/decor.ts

/**
 * Decor through WebGPU: the module's decor draws (crates/pxl-core/src/draw.rs)
 * executed as instanced draws of each variant's model, the instances read
 * straight out of the decor block the module built (decor.rs's layout) as
 * it lies on the GPU: the block is the instance buffer, the chunk's
 * translation is its uniform, and the module says which block a chunk shows.
 *
 * A model is the module's (crates/pxl-core/src/model.rs), registered by
 * decor/asset_store.ts and drawn from the same block the module holds its
 * triangles in for the hit test — the vertex lanes of model_asset.ts's
 * layout, and the indices past them. The skins are the glue's, as the
 * texture pack is, handed over by group: every bundle's variants in bundle
 * order, the order the kinds were taught.
 *
 * The shader is shader/decor.wgsl; the pipelines are the module's records.
 */

import type { u32 } from "@xpute/core/kit/word.ts";

import { PIPELINE_DECOR_DEPTH, type PipelineBuilder, type PipelineRecord } from "./pipeline.ts";
import type { ShadowPass } from "./shadow.ts";

/** Words and floats of draw.rs's records the decor pass reads. */
const DRAW_WORDS = 5;
const UNIFORM_FLOATS = 8;
const PIPELINE_DECOR = 1;
const PIPELINE_GHOST_DECOR = 4;
/** A decor draw's dynamic uniform: translation (3), the group's first lane
 * word, the instance count, the opacity, padding. */
const DRAW_UNIFORM_BYTES = 32;

/** model.rs's words a vertex: position, normal, uv, joints, weights. */
const MODEL_VERTEX_WORDS = 16;

type GpuModel = {
  /** The module's mesh block, and its counts. */
  buffer: GPUBuffer;
  vertex_cnt: u32;
  index_cnt: u32;
  skin_group: GPUBindGroup;
};

export class DecorPass {
  private readonly pipeline: GPURenderPipeline;
  /** The ghost instance: blended, both sides, writing no depth. */
  private readonly ghost_pipeline: GPURenderPipeline;
  /** Depth only, from the light, for the shadow map. */
  private readonly depth_pipeline: GPURenderPipeline;
  /** The lanes as the record lays them out, for the vertex buffer offsets. */
  private readonly record: PipelineRecord;
  private readonly frame_group: GPUBindGroup;
  private readonly light_group: GPUBindGroup;
  private readonly draw_layout: GPUBindGroupLayout;
  private readonly skin_layout: GPUBindGroupLayout;
  private readonly block_layout: GPUBindGroupLayout;
  private readonly sampler: GPUSampler;
  private readonly stride: u32;
  private draw_buffer: GPUBuffer;
  private draw_group: GPUBindGroup;
  private draw_scratch: ArrayBuffer;
  private models: (GpuModel | null)[] = [];
  /** By group: the model's handle and the skin, as told. */
  private readonly handles: u32[] = [];
  private readonly skins: (ImageBitmap | null)[] = [];
  private readonly skin_groups = new WeakMap<ImageBitmap, GPUBindGroup>();
  private readonly block_groups = new WeakMap<GPUBuffer, GPUBindGroup>();

  constructor(private readonly device: GPUDevice, pipelines: PipelineBuilder, format: GPUTextureFormat, frame_buffer: GPUBuffer, shadow: ShadowPass, sample_count: u32) {
    const lit = pipelines.build(PIPELINE_DECOR, "decor", format, sample_count);
    this.pipeline = lit.pipeline;
    this.record = lit.record;
    this.depth_pipeline = pipelines.build(PIPELINE_DECOR_DEPTH, "decor/depth", format, sample_count).pipeline;
    this.ghost_pipeline = pipelines.build(PIPELINE_GHOST_DECOR, "decor/ghost", format, sample_count).pipeline;
    const [frame_layout, draw_layout, skin_layout, block_layout] = lit.layouts;
    this.draw_layout = draw_layout;
    this.skin_layout = skin_layout;
    this.block_layout = block_layout;
    this.frame_group = device.createBindGroup({ layout: frame_layout, entries: [{ binding: 0, resource: { buffer: frame_buffer } }, ...shadow.group_entries(1)] });
    this.light_group = device.createBindGroup({ layout: frame_layout, entries: [{ binding: 0, resource: { buffer: shadow.light_frame } }, ...shadow.light_entries(1)] });
    this.sampler = device.createSampler({ magFilter: "linear", minFilter: "linear", addressModeU: "repeat", addressModeV: "repeat" });
    this.stride = Math.max(DRAW_UNIFORM_BYTES, device.limits.minUniformBufferOffsetAlignment);
    this.draw_scratch = new ArrayBuffer(this.stride * 256);
    this.draw_buffer = device.createBuffer({ label: "decor/draws", size: this.stride * 256, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST });
    this.draw_group = this.make_draw_group();
  }

  private make_draw_group(): GPUBindGroup {
    return this.device.createBindGroup({ layout: this.draw_layout, entries: [{ binding: 0, resource: { buffer: this.draw_buffer, size: DRAW_UNIFORM_BYTES } }] });
  }

  /** Triangles one instance of `group` draws; 0 while it loads. */
  triangles_of(group: u32): u32 {
    const m = this.models[group];
    return m ? m.index_cnt / 3 : 0;
  }

  /** A group's model, by the handle of the module's mesh block, and its
   * skin; either still loading is 0 or null, and the group draws nothing. */
  set_model(group: u32, handle: u32, skin: ImageBitmap | null): void {
    this.handles[group] = handle;
    this.skins[group] = skin;
    this.models[group] = null;
  }

  /** The group's model as the renderer holds it now, made on first use once
   * the block is uploaded and the skin here. */
  private model_of(group: u32, buffers: ReadonlyMap<u32, GPUBuffer>, counts: ReadonlyMap<u32, [u32, u32]>): GpuModel | null {
    const m = this.models[group];
    if (m) return m;
    const handle = this.handles[group];
    const skin = this.skins[group];
    const buffer = handle ? buffers.get(handle) : undefined;
    const count = handle ? counts.get(handle) : undefined;
    if (!buffer || !count || !skin) return null;
    let skin_group = this.skin_groups.get(skin);
    if (!skin_group) {
      const d = this.device;
      const texture = d.createTexture({
        size: [skin.width, skin.height],
        format: "rgba8unorm-srgb",
        usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST | GPUTextureUsage.RENDER_ATTACHMENT,
      });
      d.queue.copyExternalImageToTexture({ source: skin }, { texture }, [skin.width, skin.height]);
      skin_group = d.createBindGroup({ layout: this.skin_layout, entries: [{ binding: 0, resource: texture.createView() }, { binding: 1, resource: this.sampler }] });
      this.skin_groups.set(skin, skin_group);
    }
    const made: GpuModel = { buffer, vertex_cnt: count[0], index_cnt: count[1], skin_group };
    this.models[group] = made;
    return made;
  }

  /** Writes every decor draw's uniforms, ghosts included, and draws the
   * opaque ones into the open pass: one write a frame, since a later write
   * to the same buffer before the submit would stand for every draw. */
  draw(pass: GPURenderPassEncoder, draws: Uint32Array, uniforms: Float32Array, buffers: ReadonlyMap<u32, GPUBuffer>, counts: ReadonlyMap<u32, [u32, u32]>): u32 {
    const is_decor = (p: u32) => p === PIPELINE_DECOR || p === PIPELINE_GHOST_DECOR;
    let n = 0;
    for (let k = 0; k < draws.length; k += DRAW_WORDS) if (is_decor(draws[k + 4])) n++;
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
      if (!is_decor(draws[k + 4])) continue;
      const at = (slot * this.stride) >> 2;
      const uniform = draws[k + 3] * UNIFORM_FLOATS;
      f[at] = uniforms[uniform];
      f[at + 1] = uniforms[uniform + 1];
      f[at + 2] = uniforms[uniform + 2];
      u[at + 3] = uniforms[uniform + 3];
      u[at + 4] = draws[k + 2];
      f[at + 5] = uniforms[uniform + 4];
      slot++;
    }
    this.device.queue.writeBuffer(this.draw_buffer, 0, this.draw_scratch, 0, n * this.stride);
    return this.draw_pipeline(pass, draws, buffers, counts, PIPELINE_DECOR, this.pipeline, this.frame_group);
  }

  /** The opaque decor's depth from the light, after `draw` wrote the uniforms. */
  draw_depth(pass: GPURenderPassEncoder, draws: Uint32Array, buffers: ReadonlyMap<u32, GPUBuffer>, counts: ReadonlyMap<u32, [u32, u32]>): u32 {
    return this.draw_pipeline(pass, draws, buffers, counts, PIPELINE_DECOR, this.depth_pipeline, this.light_group);
  }

  /** The ghost's decor draw, blended, after everything opaque; its uniforms
   * were written by `draw`. */
  draw_ghosts(pass: GPURenderPassEncoder, draws: Uint32Array, _uniforms: Float32Array, buffers: ReadonlyMap<u32, GPUBuffer>, counts: ReadonlyMap<u32, [u32, u32]>): u32 {
    return this.draw_pipeline(pass, draws, buffers, counts, PIPELINE_GHOST_DECOR, this.ghost_pipeline, this.frame_group);
  }

  private draw_pipeline(
    pass: GPURenderPassEncoder,
    draws: Uint32Array,
    buffers: ReadonlyMap<u32, GPUBuffer>,
    counts: ReadonlyMap<u32, [u32, u32]>,
    which: u32,
    pipeline: GPURenderPipeline,
    frame_group: GPUBindGroup,
  ): u32 {
    let slot = 0;
    let drawn = 0;
    for (let k = 0; k < draws.length; k += DRAW_WORDS) {
      const p = draws[k + 4];
      if (p !== PIPELINE_DECOR && p !== PIPELINE_GHOST_DECOR) continue;
      const offset = slot++ * this.stride;
      if (p !== which) continue;
      const model = this.model_of(draws[k + 1], buffers, counts);
      const block = buffers.get(draws[k]);
      if (!model || !block) continue;
      if (drawn === 0) {
        pass.setPipeline(pipeline);
        pass.setBindGroup(0, frame_group);
      }
      let block_group = this.block_groups.get(block);
      if (!block_group) {
        block_group = this.device.createBindGroup({ layout: this.block_layout, entries: [{ binding: 0, resource: { buffer: block } }] });
        this.block_groups.set(block, block_group);
      }
      pass.setBindGroup(1, this.draw_group, [offset]);
      pass.setBindGroup(2, model.skin_group);
      pass.setBindGroup(3, block_group);
      const vc = model.vertex_cnt;
      let at = 0;
      this.record.buffers.forEach((lane, slot) => {
        pass.setVertexBuffer(slot, model.buffer, at, vc * lane.stride);
        at += vc * lane.stride;
      });
      pass.setIndexBuffer(model.buffer, "uint32", vc * MODEL_VERTEX_WORDS * 4, model.index_cnt * 4);
      pass.drawIndexed(model.index_cnt, draws[k + 2]);
      drawn++;
    }
    return drawn;
  }
}
