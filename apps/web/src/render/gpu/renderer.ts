// @pxl/web/render/gpu/renderer.ts

/**
 * The WebGPU renderer: executes what the module lists
 * (crates/pxl/core/src/render/draw.rs) against a device, and decides nothing —
 * which block goes to the GPU, which draw is made at what weight and
 * where, are the module's. What lives here is the API: the device and the
 * canvas, the pipelines and their layouts, the buffers behind the module's
 * handles, the texture pack's arrays, and the frame's uniforms, written by
 * the glue from the view and the module's environment.
 *
 * What the frame is drawn into is the renderer's to size, and what it
 * costs is counted (`stat.texture_bytes`): the depth buffer, the
 * multisampled colour and depth where MSAA is on — four samples a pixel,
 * the one count WebGPU offers, the largest allocation the page makes —
 * the sun's shadow map, the texture pack's layers with their mip chains,
 * the badge sheet. Reversed float depth throughout (depth.ts).
 *
 * A mesh block is uploaded whole, as the mesher laid it out, and drawn
 * with one vertex slot per lane at the lane's offset from the vertex
 * count, the lanes as the chunk pipeline's record lays them out
 * (crates/pxl/core/src/render/shader.rs); the index lane sits past the
 * visibility lane the three path used, which this path does not read —
 * a draw's weight is its uniform. Every pipeline and shader is the
 * module's, built through pipeline.ts.
 */

import type { u32 } from "@xpute/core/kit/word.ts";
import type { U32Array } from "@xpute/core/kit/array.ts";

import type { PxlExports } from "@/wasm/pxl.ts";
import { DecorPass } from "./decor.ts";
import { SkinnedPass } from "./skinned.ts";
import { PuckPass } from "./puck.ts";
import { BadgePass } from "./badges.ts";
import { PreviewCase } from "./preview.ts";
import { DebugPass } from "./debug.ts";
import { create_layer_textures, type LayerTextures } from "./layers.ts";
import { DEPTH_CLEAR, DEPTH_FORMAT } from "./depth.ts";
import { ShadowPass } from "./shadow.ts";
import { MipBuilder } from "./mips.ts";
import { type FrameLayout, PIPELINE_CHUNK_DEPTH, PIPELINE_SKY, PipelineBuilder, type PipelineRecord, read_frame_layout } from "./pipeline.ts";
import { CORE_BUILD } from "@/wasm/core_build.ts";
import { Cmd } from "@pxl/shared/spec/cmd.ts";

/** draw.rs's records, in words and floats. */
const UPLOAD_WORDS = 5;
const REWRITE_WORDS = 3;
const DRAW_WORDS = 5;
const UNIFORM_FLOATS = 8;
const PIPELINE_CHUNK = 0;
const PIPELINE_GHOST_BOX = 2;
const PIPELINE_GHOST_BOX_COPLANAR = 3;
const PIPELINE_DECOR = 1;
const PIPELINE_GHOST_DECOR = 4;
const PIPELINE_SKINNED = 5;
const PIPELINE_PUCK_ZONE = 6;
const PIPELINE_PUCK_BEACON = 7;
const PIPELINE_BADGE = 8;
const PIPELINE_DEBUG = 9;
/** draw.rs's pipelines, by id, as the ledger names them. */
const PIPELINE_NAMES: Record<number, string> = {
  [PIPELINE_CHUNK]: "chunk",
  [PIPELINE_DECOR]: "decor",
  [PIPELINE_GHOST_BOX]: "ghost",
  [PIPELINE_GHOST_BOX_COPLANAR]: "ghost",
  [PIPELINE_GHOST_DECOR]: "ghost",
  [PIPELINE_SKINNED]: "actor",
  [PIPELINE_PUCK_ZONE]: "puck",
  [PIPELINE_PUCK_BEACON]: "puck",
  [PIPELINE_BADGE]: "billboard",
  [PIPELINE_DEBUG]: "debug",
};

/** The visibility lane past the vertex lanes, floats a vertex, which the
 * index lane follows. */
const VIS_F32_PER_VERTEX = 1;

/** Floats the sky uniform holds (shader/sky.wgsl's Sky). */
export const SKY_UNIFORM_FLOATS = 16 + 4 * 7;

/** What the renderer is made with; a change means a new renderer. */
export type RendererOptions = {
  /** Four samples a pixel, or one. */
  msaa: boolean;
};

/** Bytes of what the frame is drawn into, by what it is for. */
export type TextureBytes = { depth: number; msaa: number; shadow: number; layers: number };

export type FrameUniforms = Float32Array<ArrayBuffer>;

export class GpuRenderer {
  private readonly context: GPUCanvasContext;
  private readonly format: GPUTextureFormat;
  private readonly pipeline: GPURenderPipeline;
  /** The ghost box through the chunk shader: blended, writing no depth,
   * biased toward the eye as hover_cursor.tsx's polygon offsets were —
   * slightly clear of a surface, strongly in one. */
  private readonly ghost_pipelines: Record<number, GPURenderPipeline>;
  private readonly sky_pipeline: GPURenderPipeline;
  private readonly sky_buffer: GPUBuffer;
  private readonly sky_bind_group: GPUBindGroup;
  private readonly bind_layout: GPUBindGroupLayout;
  /** The chunk pipeline's record: the lanes' strides, for the offsets. */
  private readonly chunk_record: PipelineRecord;
  private readonly chunk_bytes_per_vertex: u32;
  private readonly layers: LayerTextures;
  /** The chunks' depth from the light, for the shadow map. */
  private readonly depth_pipeline: GPURenderPipeline;
  private light_bind_group: GPUBindGroup;
  readonly shadow: ShadowPass;
  readonly mips: MipBuilder;
  readonly pipelines: PipelineBuilder;
  /** The frame uniform's layout, the module's (shader.rs's FRAME_AT). */
  readonly frame_at: FrameLayout;
  readonly sample_count: u32;
  /** Multisampled colour and depth, the frame's targets under MSAA. */
  private msaa_color: GPUTexture | null = null;
  /** GPU time per frame from timestamp queries, where the device has them. */
  private readonly timestamps: { set: GPUQuerySet; resolve: GPUBuffer; read: GPUBuffer[]; pending: Set<GPUBuffer> } | null = null;
  /** Decor's instanced draws (decor.ts). */
  readonly decor: DecorPass;
  /** Actors' skinned draws (skinned.ts). */
  readonly skinned: SkinnedPass;
  /** The location puck (puck.ts). */
  private readonly puck: PuckPass;
  /** The badges (badges.ts), a pass of their own that reads the depth. */
  readonly badges: BadgePass;
  /** The dev gizmos (debug.ts), over everything. */
  private readonly debug: DebugPass;
  /** The display cases open (preview.ts), drawn after the world each frame. */
  private readonly previews = new Set<PreviewCase>();
  /** Each block's vertex and index counts, as its upload said. */
  private readonly counts = new Map<u32, [u32, u32]>();
  private readonly frame_buffer: GPUBuffer;
  private draws_buffer: GPUBuffer;
  private bind_group: GPUBindGroup;
  private depth: GPUTexture | null = null;
  private readonly buffers = new Map<u32, GPUBuffer>();
  /** The module's draw row, at a fixed address: one view, made on the
   * first take rather than by a take of its own, which would drop the
   * uploads waiting for the renderer. */
  private width = 0;
  private height = 0;
  /** The frame's uniforms, written by the glue before `frame`. */
  readonly uniforms: FrameUniforms;
  /** The sky's uniforms, likewise. */
  readonly sky: FrameUniforms = new Float32Array(new ArrayBuffer(SKY_UNIFORM_FLOATS * 4));
  /** What the last frame drew, for the ledger. `gpu_ms` is the GPU's own
   * time over the frame's passes from timestamp queries, a few frames
   * behind; 0 where the device has none. */
  stat = { draws: 0, triangles: 0, decor_draws: 0, badges: 0, uploads: 0, frees: 0, buffers: 0, bytes: 0, gpu_ms: 0, texture_bytes: { depth: 0, msaa: 0, shadow: 0, layers: 0 } as TextureBytes };
  /** Whether the sun's shadow is drawn this frame: the setting, read by the glue. */
  shadows = false;
  /** Draws and triangles by pipeline, for the ledger; filled in dev only. */
  census: Record<string, [number, number]> = {};

  private constructor(readonly device: GPUDevice, readonly canvas: HTMLCanvasElement, private readonly pxl: PxlExports, options: RendererOptions) {
    const context = canvas.getContext("webgpu");
    if (!context) throw new Error("gpu: the canvas has no webgpu context");
    this.context = context;
    this.format = navigator.gpu.getPreferredCanvasFormat();
    this.context.configure({ device, format: this.format, alphaMode: "premultiplied" });
    this.sample_count = options.msaa ? 4 : 1;
    const samples = this.sample_count;
    this.pipelines = new PipelineBuilder(device, pxl);
    this.frame_at = read_frame_layout();
    this.uniforms = new Float32Array(new ArrayBuffer(this.frame_at.floats * 4));
    this.mips = new MipBuilder(device, this.pipelines);
    this.shadow = new ShadowPass(device, this.frame_at);
    this.stat.texture_bytes.shadow = this.shadow.bytes;

    const chunk = this.pipelines.build(PIPELINE_CHUNK, "chunk", this.format, samples);
    this.pipeline = chunk.pipeline;
    this.bind_layout = chunk.layouts[0];
    this.chunk_record = chunk.record;
    this.chunk_bytes_per_vertex = chunk.record.buffers.reduce((sum, b) => sum + b.stride, 0) + VIS_F32_PER_VERTEX * 4;
    this.depth_pipeline = this.pipelines.build(PIPELINE_CHUNK_DEPTH, "chunk/depth", this.format, samples).pipeline;
    this.ghost_pipelines = {
      [PIPELINE_GHOST_BOX]: this.pipelines.build(PIPELINE_GHOST_BOX, "ghost/box", this.format, samples).pipeline,
      [PIPELINE_GHOST_BOX_COPLANAR]: this.pipelines.build(PIPELINE_GHOST_BOX_COPLANAR, "ghost/box/coplanar", this.format, samples).pipeline,
    };
    this.frame_buffer = device.createBuffer({ label: "frame", size: this.frame_at.floats * 4, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST });
    const sky = this.pipelines.build(PIPELINE_SKY, "sky", this.format, samples);
    this.sky_pipeline = sky.pipeline;
    this.sky_buffer = device.createBuffer({ label: "sky", size: SKY_UNIFORM_FLOATS * 4, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST });
    this.sky_bind_group = device.createBindGroup({ layout: sky.layouts[0], entries: [{ binding: 0, resource: { buffer: this.frame_buffer } }, { binding: 1, resource: { buffer: this.sky_buffer } }] });
    this.layers = create_layer_textures(device, this.mips);
    this.stat.texture_bytes.layers = this.layers.bytes;
    this.draws_buffer = device.createBuffer({ label: "draws", size: 256 * UNIFORM_FLOATS * 4, usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST });
    this.bind_group = this.make_bind_group(this.frame_buffer);
    this.light_bind_group = this.make_bind_group(this.shadow.light_frame, true);
    this.decor = new DecorPass(device, this.pipelines, this.format, this.frame_buffer, this.shadow, samples);
    this.skinned = new SkinnedPass(device, this.pipelines, this.format, this.frame_buffer, this.shadow, samples);
    this.puck = new PuckPass(device, this.pipelines, this.format, this.frame_buffer, samples);
    this.badges = new BadgePass(device, this.pipelines, this.format, samples, this.mips);
    this.debug = new DebugPass(device, this.pipelines, this.format, this.frame_buffer, samples);
    if (device.features.has("timestamp-query")) {
      this.timestamps = {
        set: device.createQuerySet({ type: "timestamp", count: 2 }),
        resolve: device.createBuffer({ label: "timestamps", size: 16, usage: GPUBufferUsage.QUERY_RESOLVE | GPUBufferUsage.COPY_SRC }),
        read: Array.from({ length: 3 }, () => device.createBuffer({ label: "timestamps/read", size: 16, usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ })),
        pending: new Set(),
      };
    }
  }

  /** A renderer over a device, or null where there is no WebGPU: the app
   * has no other path, so the caller says so. */
  static async create(canvas: HTMLCanvasElement, pxl: PxlExports, options: RendererOptions): Promise<GpuRenderer | null> {
    if (!("gpu" in navigator)) return null;
    const adapter = await navigator.gpu.requestAdapter({ powerPreference: "high-performance" });
    if (!adapter) return null;
    const device = await adapter.requestDevice({ requiredFeatures: adapter.features.has("timestamp-query") ? ["timestamp-query"] : [] });
    // A device this renderer let go (destroy) is lost on purpose; only
    // one the browser took away is worth a word.
    device.lost.then((info) => {
      if (info.reason !== "destroyed") console.error(`[gpu] device lost: ${info.message}`);
    });
    device.onuncapturederror = (e) => console.error(`[gpu] ${e.error.message}`);
    return new GpuRenderer(device, canvas, pxl, options);
  }

  /** The chunk pass's group over `frame`; the light's reads no map, since
   * the depth pass writes it. */
  private make_bind_group(frame: GPUBuffer, light = false): GPUBindGroup {
    return this.device.createBindGroup({
      layout: this.bind_layout,
      entries: [
        { binding: 0, resource: { buffer: frame } },
        ...(light ? this.shadow.light_entries(1) : this.shadow.group_entries(1)),
        { binding: 3, resource: { buffer: this.draws_buffer } },
        { binding: 4, resource: this.layers.top.createView({ dimension: "2d-array" }) },
        { binding: 5, resource: this.layers.side.createView({ dimension: "2d-array" }) },
        { binding: 6, resource: this.layers.sampler },
      ],
    });
  }

  /** The canvas's size in device pixels; the depth buffer follows it. */
  resize(width: u32, height: u32): void {
    width = Math.max(1, Math.floor(width));
    height = Math.max(1, Math.floor(height));
    if (width === this.width && height === this.height) return;
    this.width = width;
    this.height = height;
    this.canvas.width = width;
    this.canvas.height = height;
    this.depth?.destroy();
    this.msaa_color?.destroy();
    const samples = this.sample_count;
    this.depth = this.device.createTexture({
      label: "depth",
      size: [width, height],
      format: DEPTH_FORMAT,
      sampleCount: samples,
      usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.TEXTURE_BINDING,
    });
    this.stat.texture_bytes.depth = width * height * 4 * samples;
    if (samples > 1) {
      this.msaa_color = this.device.createTexture({ label: "msaa", size: [width, height], format: this.format, sampleCount: samples, usage: GPUTextureUsage.RENDER_ATTACHMENT });
      this.stat.texture_bytes.msaa = width * height * 4 * samples;
    } else {
      this.msaa_color = null;
      this.stat.texture_bytes.msaa = 0;
    }
  }

  /** Lets the device go, with everything on it. */
  destroy(): void {
    this.device.destroy();
  }

  /** A display case over `canvas`, drawn each frame until closed. */
  open_preview(canvas: HTMLCanvasElement): PreviewCase {
    const preview = new PreviewCase(this.device, this.pipelines, this.frame_at, canvas, this.format);
    this.previews.add(preview);
    return preview;
  }

  close_preview(preview: PreviewCase): void {
    this.previews.delete(preview);
  }

  /** Takes the module's lists and draws the frame: the blocks it names
   * uploaded and freed, then every draw with its uniforms. */
  frame(): void {
    if (!this.depth) return;
    const pxl = this.pxl;
    const mem = pxl.memory.buffer;
    // The take is a query, so every command sent this frame is applied
    // before the lists are taken: what a phase after the module's tick sent
    // (a billboard, the dev gizmos) is drawn in the frame that sent it.
    const row = CORE_BUILD.call(Cmd.DRAW_READ, null, (_, reply) => Uint32Array.from(reply!.at(0).get<U32Array>()));
    const rewrites = new Uint32Array(mem, row[8], row[9] * REWRITE_WORDS);
    const uploads = new Uint32Array(mem, row[0], row[1] * UPLOAD_WORDS);
    const frees = new Uint32Array(mem, row[2], row[3]);
    const draws = new Uint32Array(mem, row[4], row[5] * DRAW_WORDS);
    const uniforms = new Float32Array(mem, row[6], row[7] * UNIFORM_FLOATS);

    for (let k = 0; k < uploads.length; k += UPLOAD_WORDS) {
      const [handle, at, bytes] = [uploads[k], uploads[k + 1], uploads[k + 2]];
      this.buffers.get(handle)?.destroy();
      const size = Math.max(4, (bytes + 3) & ~3);
      // Every block is usable every way a draw reads one: a mesh's lanes as
      // vertices and indices, a decor block's as storage.
      const buffer = this.device.createBuffer({ size, usage: GPUBufferUsage.VERTEX | GPUBufferUsage.INDEX | GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST });
      this.device.queue.writeBuffer(buffer, 0, mem as ArrayBuffer, at, bytes & ~3);
      this.buffers.set(handle, buffer);
      this.counts.set(handle, [uploads[k + 3], uploads[k + 4]]);
      this.stat.bytes += size;
    }
    for (let k = 0; k < rewrites.length; k += REWRITE_WORDS) {
      const buffer = this.buffers.get(rewrites[k]);
      if (buffer) this.device.queue.writeBuffer(buffer, 0, mem as ArrayBuffer, rewrites[k + 1], rewrites[k + 2] & ~3);
    }
    for (const handle of frees) {
      const buffer = this.buffers.get(handle);
      if (!buffer) continue;
      this.stat.bytes -= buffer.size;
      buffer.destroy();
      this.buffers.delete(handle);
      this.counts.delete(handle);
    }
    this.stat.uploads = uploads.length / UPLOAD_WORDS;
    this.stat.frees = frees.length;
    this.stat.buffers = this.buffers.size;
    this.stat.draws = draws.length / DRAW_WORDS;

    this.device.queue.writeBuffer(this.frame_buffer, 0, this.uniforms);
    this.device.queue.writeBuffer(this.sky_buffer, 0, this.sky);
    if (uniforms.byteLength > this.draws_buffer.size) {
      this.draws_buffer.destroy();
      this.draws_buffer = this.device.createBuffer({ label: "draws", size: uniforms.byteLength * 2, usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST });
      this.bind_group = this.make_bind_group(this.frame_buffer);
      this.light_bind_group = this.make_bind_group(this.shadow.light_frame, true);
    }
    if (uniforms.length > 0) this.device.queue.writeBuffer(this.draws_buffer, 0, mem as ArrayBuffer, row[6], uniforms.byteLength);

    const encoder = this.device.createCommandEncoder();
    const ts = this.timestamps;
    // The sun's shadow map first, from the frame's lists, where it is on.
    if (this.shadows) {
      const shadow = this.shadow.begin(encoder);
      shadow.setPipeline(this.depth_pipeline);
      shadow.setBindGroup(0, this.light_bind_group);
      this.draw_meshes(shadow, draws, (p) => p === PIPELINE_CHUNK);
      this.decor.draw_depth(shadow, draws, this.buffers, this.counts);
      this.skinned.draw_depth(shadow, draws, this.buffers, this.counts);
      shadow.end();
    }
    const canvas_view = this.context.getCurrentTexture().createView();
    const color = this.msaa_color ? this.msaa_color.createView() : canvas_view;
    const resolve = this.msaa_color ? canvas_view : undefined;
    const pass = encoder.beginRenderPass({
      colorAttachments: [{ view: color, resolveTarget: resolve, loadOp: "clear", clearValue: { r: 0, g: 0, b: 0, a: 0 }, storeOp: "store" }],
      depthStencilAttachment: { view: this.depth.createView(), depthLoadOp: "clear", depthClearValue: DEPTH_CLEAR, depthStoreOp: "store" },
      timestampWrites: ts ? { querySet: ts.set, beginningOfPassWriteIndex: 0 } : undefined,
    });
    pass.setPipeline(this.sky_pipeline);
    pass.setBindGroup(0, this.sky_bind_group);
    pass.draw(3);
    pass.setPipeline(this.pipeline);
    pass.setBindGroup(0, this.bind_group);
    this.draw_meshes(pass, draws, (p) => p === PIPELINE_CHUNK);
    this.stat.decor_draws = this.decor.draw(pass, draws, uniforms, this.buffers, this.counts);
    this.skinned.draw(pass, draws, uniforms, this.buffers, this.counts);
    this.puck.draw_beacon(pass, draws, uniforms);
    this.puck.draw_zone(pass, draws, uniforms, this.buffers);
    // What is blended goes last, over everything opaque.
    for (const p of [PIPELINE_GHOST_BOX, PIPELINE_GHOST_BOX_COPLANAR]) {
      if (!this.draw_meshes(pass, draws, (q) => q === p, true)) continue;
    }
    this.decor.draw_ghosts(pass, draws, uniforms, this.buffers, this.counts);
    this.debug.draw(pass, draws, uniforms, this.buffers);
    pass.end();
    this.stat.badges = this.badges.draw(encoder, color, resolve, this.depth, [this.width, this.height], draws, uniforms, this.buffers);
    for (const preview of this.previews) preview.draw(encoder, this.uniforms, draws, uniforms, this.buffers, this.counts);
    // The frame's end, on a pass of its own that draws nothing, so the two
    // timestamps bracket every pass of the frame.
    let read: GPUBuffer | null = null;
    if (ts) {
      const end = encoder.beginRenderPass({
        colorAttachments: [{ view: color, resolveTarget: resolve, loadOp: "load", storeOp: "store" }],
        timestampWrites: { querySet: ts.set, endOfPassWriteIndex: 1 },
      });
      end.end();
      read = ts.read.find((b) => !ts.pending.has(b)) ?? null;
      if (read) {
        encoder.resolveQuerySet(ts.set, 0, 2, ts.resolve, 0);
        encoder.copyBufferToBuffer(ts.resolve, 0, read, 0, 16);
      }
    }
    this.device.queue.submit([encoder.finish()]);
    if (ts && read) {
      const buffer = read;
      ts.pending.add(buffer);
      buffer.mapAsync(GPUMapMode.READ).then(() => {
        const [t0, t1] = new BigUint64Array(buffer.getMappedRange());
        this.stat.gpu_ms = Number(t1 - t0) / 1e6;
        buffer.unmap();
        ts.pending.delete(buffer);
      }, () => ts.pending.delete(buffer));
    }
    if (import.meta.env.DEV) this.take_census(draws);
  }

  /** Draws and triangles by pipeline, this frame. */
  private take_census(draws: Uint32Array): void {
    const census: Record<string, [number, number]> = {};
    let triangles = 0;
    for (let k = 0; k < draws.length; k += DRAW_WORDS) {
      const [a, b, c, pipeline] = [draws[k], draws[k + 1], draws[k + 2], draws[k + 4]];
      let tris = 0;
      switch (pipeline) {
        case PIPELINE_CHUNK:
        case PIPELINE_GHOST_BOX:
        case PIPELINE_GHOST_BOX_COPLANAR:
          tris = c / 3;
          break;
        case PIPELINE_DECOR:
        case PIPELINE_GHOST_DECOR:
          tris = this.decor.triangles_of(b) * c;
          break;
        case PIPELINE_SKINNED:
          tris = (this.counts.get(a)?.[1] ?? 0) / 3;
          break;
        case PIPELINE_PUCK_ZONE:
          tris = 12 * b;
          break;
        case PIPELINE_PUCK_BEACON:
          tris = 8;
          break;
        case PIPELINE_BADGE:
          tris = 2 * b;
          break;
      }
      const name = PIPELINE_NAMES[pipeline] ?? `pipeline${pipeline}`;
      const row = census[name] ??= [0, 0];
      row[0]++;
      row[1] += tris;
      triangles += tris;
    }
    this.census = census;
    this.stat.triangles = triangles;
  }

  /** Every mesh draw whose pipeline `which` accepts; with `own_pipeline`,
   * each switches to its ghost pipeline first. Whether any was drawn. */
  private draw_meshes(pass: GPURenderPassEncoder, draws: Uint32Array, which: (p: u32) => boolean, own_pipeline = false): boolean {
    let any = false;
    for (let k = 0; k < draws.length; k += DRAW_WORDS) {
      const [handle, vertex_cnt, index_cnt, uniform, pipeline] = [draws[k], draws[k + 1], draws[k + 2], draws[k + 3], draws[k + 4]];
      if (!which(pipeline)) continue;
      const buffer = this.buffers.get(handle);
      if (!buffer) continue;
      if (own_pipeline && !any) {
        pass.setPipeline(this.ghost_pipelines[pipeline]);
        pass.setBindGroup(0, this.bind_group);
      }
      any = true;
      let offset = 0;
      this.chunk_record.buffers.forEach((lane, slot) => {
        pass.setVertexBuffer(slot, buffer, offset);
        offset += vertex_cnt * lane.stride;
      });
      pass.setIndexBuffer(buffer, "uint32", vertex_cnt * this.chunk_bytes_per_vertex, index_cnt * 4);
      pass.drawIndexed(index_cnt, 1, 0, 0, uniform);
    }
    return any;
  }
}
