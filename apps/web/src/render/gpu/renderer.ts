// @pxl/web/render/gpu/renderer.ts

/**
 * The WebGPU renderer: executes what the module lists
 * (crates/pxl-core/src/draw.rs) against a device, and decides nothing —
 * which block goes to the GPU, which draw is made at what weight and
 * where, are the module's. What lives here is the API: the device and the
 * canvas, the pipelines and their layouts, the buffers behind the module's
 * handles, the texture pack's arrays, and the frame's uniforms — the
 * camera and the lights, written by the glue from what three still owns
 * until three goes.
 *
 * A mesh block is uploaded whole, as the mesher laid it out, and drawn
 * with one vertex slot per lane at the lane's offset from the vertex
 * count (chunk_mesher.ts's carve_baked_mesh); the index lane sits past the
 * visibility lane the three path used, which this path does not read —
 * a draw's weight is its uniform.
 */

import type { u32 } from "@xpute/core/kit/word.ts";

import type { PxlExports } from "@/wasm/pxl.ts";
import { CHUNK_SHADER } from "./chunk_shader.ts";
import { SKY_SHADER } from "./sky_shader.ts";
import { DecorPass } from "./decor.ts";
import { create_layer_textures, type LayerTextures } from "./layers.ts";

/** draw.rs's records, in words and floats. */
const UPLOAD_WORDS = 5;
const DRAW_WORDS = 5;
const UNIFORM_FLOATS = 8;
const PIPELINE_CHUNK = 0;

/** chunk_mesher.ts's lanes, floats a vertex: position, normal, local_pos,
 * box_scale, rgb, layer_taps, faces; then the visibility lane; then the
 * index lane. */
const LANE_FLOATS = [3, 3, 3, 3, 3, 4, 1] as const;
const F32_PER_VERTEX = 20;
const VIS_F32_PER_VERTEX = 1;

/** Floats the frame uniform holds (chunk_shader.ts's Frame). */
export const FRAME_UNIFORM_FLOATS = 16 + 4 * 8;
const FRAME_UNIFORM_BYTES = FRAME_UNIFORM_FLOATS * 4;
/** Floats the sky uniform holds (sky_shader.ts's Sky). */
export const SKY_UNIFORM_FLOATS = 16 + 4 * 7;

const DEPTH_FORMAT: GPUTextureFormat = "depth24plus";

export type FrameUniforms = Float32Array<ArrayBuffer>;

export class GpuRenderer {
  private readonly context: GPUCanvasContext;
  private readonly format: GPUTextureFormat;
  private readonly pipeline: GPURenderPipeline;
  private readonly sky_pipeline: GPURenderPipeline;
  private readonly sky_buffer: GPUBuffer;
  private readonly sky_bind_group: GPUBindGroup;
  private readonly bind_layout: GPUBindGroupLayout;
  private readonly layers: LayerTextures;
  /** Decor's instanced draws (decor.ts). */
  readonly decor: DecorPass;
  private readonly frame_buffer: GPUBuffer;
  private draws_buffer: GPUBuffer;
  private bind_group: GPUBindGroup;
  private depth: GPUTexture | null = null;
  private readonly buffers = new Map<u32, GPUBuffer>();
  private width = 0;
  private height = 0;
  /** The frame's uniforms, written by the glue before `frame`. */
  readonly uniforms: FrameUniforms = new Float32Array(new ArrayBuffer(FRAME_UNIFORM_FLOATS * 4));
  /** The sky's uniforms, likewise. */
  readonly sky: FrameUniforms = new Float32Array(new ArrayBuffer(SKY_UNIFORM_FLOATS * 4));
  /** What the last frame drew, for the ledger. */
  stat = { draws: 0, decor_draws: 0, uploads: 0, frees: 0, buffers: 0, bytes: 0 };

  private constructor(readonly device: GPUDevice, readonly canvas: HTMLCanvasElement, private readonly pxl: PxlExports) {
    const context = canvas.getContext("webgpu");
    if (!context) throw new Error("gpu: the canvas has no webgpu context");
    this.context = context;
    this.format = navigator.gpu.getPreferredCanvasFormat();
    this.context.configure({ device, format: this.format, alphaMode: "premultiplied" });

    this.bind_layout = device.createBindGroupLayout({
      label: "chunk/bind",
      entries: [
        { binding: 0, visibility: GPUShaderStage.VERTEX | GPUShaderStage.FRAGMENT, buffer: { type: "uniform" } },
        { binding: 1, visibility: GPUShaderStage.VERTEX | GPUShaderStage.FRAGMENT, buffer: { type: "read-only-storage" } },
        { binding: 2, visibility: GPUShaderStage.FRAGMENT, texture: { viewDimension: "2d-array" } },
        { binding: 3, visibility: GPUShaderStage.FRAGMENT, texture: { viewDimension: "2d-array" } },
        { binding: 4, visibility: GPUShaderStage.FRAGMENT, sampler: {} },
      ],
    });
    const module = device.createShaderModule({ label: "chunk", code: CHUNK_SHADER });
    const vertex_buffers: GPUVertexBufferLayout[] = LANE_FLOATS.map((floats, location) => ({
      arrayStride: floats * 4,
      attributes: [{ shaderLocation: location, offset: 0, format: (floats === 1 ? "float32" : `float32x${floats}`) as GPUVertexFormat }],
    }));
    this.pipeline = device.createRenderPipeline({
      label: "chunk",
      layout: device.createPipelineLayout({ bindGroupLayouts: [this.bind_layout] }),
      vertex: { module, entryPoint: "vs", buffers: vertex_buffers },
      fragment: { module, entryPoint: "fs", targets: [{ format: this.format }] },
      primitive: { topology: "triangle-list", cullMode: "back", frontFace: "ccw" },
      depthStencil: { format: DEPTH_FORMAT, depthWriteEnabled: true, depthCompare: "less" },
    });
    // The sky: one triangle over the screen, first, writing no depth.
    const sky_module = device.createShaderModule({ label: "sky", code: SKY_SHADER });
    this.sky_pipeline = device.createRenderPipeline({
      label: "sky",
      layout: "auto",
      vertex: { module: sky_module, entryPoint: "vs" },
      fragment: { module: sky_module, entryPoint: "fs", targets: [{ format: this.format }] },
      primitive: { topology: "triangle-list" },
      depthStencil: { format: DEPTH_FORMAT, depthWriteEnabled: false, depthCompare: "always" },
    });
    this.sky_buffer = device.createBuffer({ label: "sky", size: SKY_UNIFORM_FLOATS * 4, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST });
    this.sky_bind_group = device.createBindGroup({ layout: this.sky_pipeline.getBindGroupLayout(0), entries: [{ binding: 0, resource: { buffer: this.sky_buffer } }] });
    this.layers = create_layer_textures(device);
    this.frame_buffer = device.createBuffer({ label: "frame", size: FRAME_UNIFORM_BYTES, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST });
    this.draws_buffer = device.createBuffer({ label: "draws", size: 256 * UNIFORM_FLOATS * 4, usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST });
    this.bind_group = this.make_bind_group();
    this.decor = new DecorPass(device, this.format, DEPTH_FORMAT, this.frame_buffer);
  }

  /** A renderer over a device, or null where there is no WebGPU: the app
   * has no other path, so the caller says so. */
  static async create(canvas: HTMLCanvasElement, pxl: PxlExports): Promise<GpuRenderer | null> {
    if (!("gpu" in navigator)) return null;
    const adapter = await navigator.gpu.requestAdapter({ powerPreference: "high-performance" });
    if (!adapter) return null;
    const device = await adapter.requestDevice();
    device.lost.then((info) => console.error(`[gpu] device lost: ${info.message}`));
    device.onuncapturederror = (e) => console.error(`[gpu] ${e.error.message}`);
    return new GpuRenderer(device, canvas, pxl);
  }

  private make_bind_group(): GPUBindGroup {
    return this.device.createBindGroup({
      layout: this.bind_layout,
      entries: [
        { binding: 0, resource: { buffer: this.frame_buffer } },
        { binding: 1, resource: { buffer: this.draws_buffer } },
        { binding: 2, resource: this.layers.top.createView({ dimension: "2d-array" }) },
        { binding: 3, resource: this.layers.side.createView({ dimension: "2d-array" }) },
        { binding: 4, resource: this.layers.sampler },
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
    this.depth = this.device.createTexture({ label: "depth", size: [width, height], format: DEPTH_FORMAT, usage: GPUTextureUsage.RENDER_ATTACHMENT });
  }

  /** Takes the module's lists and draws the frame: the blocks it names
   * uploaded and freed, then every draw with its uniforms. */
  frame(): void {
    if (!this.depth) return;
    const pxl = this.pxl;
    const mem = pxl.memory.buffer;
    const row = new Uint32Array(mem, pxl.pxl_draw_take(), 8);
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
      this.stat.bytes += size;
    }
    for (const handle of frees) {
      const buffer = this.buffers.get(handle);
      if (!buffer) continue;
      this.stat.bytes -= buffer.size;
      buffer.destroy();
      this.buffers.delete(handle);
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
      this.bind_group = this.make_bind_group();
    }
    if (uniforms.length > 0) this.device.queue.writeBuffer(this.draws_buffer, 0, mem as ArrayBuffer, row[6], uniforms.byteLength);

    const encoder = this.device.createCommandEncoder();
    const pass = encoder.beginRenderPass({
      colorAttachments: [{ view: this.context.getCurrentTexture().createView(), loadOp: "clear", clearValue: { r: 0, g: 0, b: 0, a: 0 }, storeOp: "store" }],
      depthStencilAttachment: { view: this.depth.createView(), depthLoadOp: "clear", depthClearValue: 1, depthStoreOp: "store" },
    });
    pass.setPipeline(this.sky_pipeline);
    pass.setBindGroup(0, this.sky_bind_group);
    pass.draw(3);
    pass.setPipeline(this.pipeline);
    pass.setBindGroup(0, this.bind_group);
    for (let k = 0; k < draws.length; k += DRAW_WORDS) {
      const [handle, vertex_cnt, index_cnt, uniform, pipeline] = [draws[k], draws[k + 1], draws[k + 2], draws[k + 3], draws[k + 4]];
      if (pipeline !== PIPELINE_CHUNK) continue;
      const buffer = this.buffers.get(handle);
      if (!buffer) continue;
      let offset = 0;
      LANE_FLOATS.forEach((floats, slot) => {
        pass.setVertexBuffer(slot, buffer, offset);
        offset += vertex_cnt * floats * 4;
      });
      pass.setIndexBuffer(buffer, "uint32", vertex_cnt * (F32_PER_VERTEX + VIS_F32_PER_VERTEX) * 4, index_cnt * 4);
      pass.drawIndexed(index_cnt, 1, 0, 0, uniform);
    }
    this.stat.decor_draws = this.decor.draw(pass, draws, uniforms, this.buffers);
    pass.end();
    this.device.queue.submit([encoder.finish()]);
  }
}
