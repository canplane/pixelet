// @pxl/web/kernel/render/gpu.ts

/**
 * The host's WebGPU: the device, and the calls the kernel's renderer makes
 * (crates/pxl/web/render/src/gpu/api.rs), every object named by the handle
 * the kernel gave it and kept here at that handle's slot. Nothing is decided
 * here — what is made, from what, and drawn when, is the renderer's; each
 * call is translated into WebGPU's and made. It exists because a WebAssembly
 * module reaches WebGPU no other way.
 *
 * A canvas handed to the kernel is a surface: transferred to an
 * OffscreenCanvas once, and drawn by the kernel alone from then on. The 2D
 * scratch is where the kernel paints what needs the host's fonts (the badge
 * sheet) or its decoded images (the texture pack), before copying it into a
 * texture.
 */

import type { U32Array } from "@xpute/core/abi/array.ts";
import type { f32, u32 } from "@xpute/core/abi/word.ts";
import { handle_slot } from "@xpute/runtime/abi/handle.ts";

import { address, compare, dimension, filter, format, load, paint, record, resource } from "@/kernel/render/gpu/api.spec.ts";

type GpuObject =
  | GPUBuffer
  | GPUTexture
  | GPUTextureView
  | GPUSampler
  | GPUShaderModule
  | GPUBindGroupLayout
  | GPURenderPipeline
  | GPUBindGroup
  | GPUCommandEncoder
  | GPURenderPassEncoder
  | GPUQuerySet;

interface Surface {
  canvas: OffscreenCanvas;
  context: GPUCanvasContext | null;
}

let device: GPUDevice | null = null;
let canvas_format: GPUTextureFormat = "bgra8unorm";
const objects: (GpuObject | undefined)[] = [];
const surfaces: (Surface | undefined)[] = [];
/** A canvas transfers once: an effect mounted again is handed the same one. */
const transferred = new WeakMap<HTMLCanvasElement, OffscreenCanvas>();
const images = new Map<u32, ImageBitmap>();
const utf8 = new TextDecoder();

export interface DeviceInfo {
  align: u32;
  timestamps: boolean;
}
/** Why the browser answered with no device, in the words of whoever has to
 * act on it: the ways WebGPU goes missing are fixed in different places and
 * one sentence for all of them sends the reader to the wrong one. */
export interface NoDevice {
  reason: string;
}

/** The device, once the browser answers, or why there is none. `lost` hears
 * it go. */
export async function acquire_device(lost: () => void): Promise<DeviceInfo | NoDevice> {
  // `navigator.gpu` is exposed to a secure context and nowhere else, so the
  // same browser on the same machine has WebGPU at localhost and none at the
  // dev server's LAN address over http.
  if (!("gpu" in navigator)) {
    return { reason: isSecureContext ? "this browser has no navigator.gpu" : `${location.origin} is not a secure context, which is the only place navigator.gpu is exposed` };
  }
  const adapter = await navigator.gpu.requestAdapter({ powerPreference: "high-performance" });
  if (!adapter) return { reason: "navigator.gpu is here but no adapter answered — the GPU is unavailable or blocklisted (chrome://gpu)" };
  const timestamps = adapter.features.has("timestamp-query");
  const d = await adapter.requestDevice({ requiredFeatures: timestamps ? ["timestamp-query"] : [] });
  d.lost.then((info) => {
    if (info.reason !== "destroyed") console.error(`[gpu] device lost: ${info.message}`);
    if (device !== d) return;
    device = null;
    objects.length = 0;
    stamps = null;
    lost();
  });
  d.onuncapturederror = (e) => console.error(`[gpu] ${e.error.message}`);
  device = d;
  canvas_format = navigator.gpu.getPreferredCanvasFormat();
  return { align: d.limits.minUniformBufferOffsetAlignment, timestamps };
}

/** A canvas as the kernel's surface `handle`. */
export function bind_surface(handle: u32, canvas: HTMLCanvasElement): void {
  let offscreen = transferred.get(canvas);
  if (!offscreen) {
    offscreen = canvas.transferControlToOffscreen();
    transferred.set(canvas, offscreen);
  }
  surfaces[handle_slot(handle)] = { canvas: offscreen, context: null };
}

export function unbind_surface(handle: u32): void {
  surfaces[handle_slot(handle)] = undefined;
}

/** An image a load decoded, by the handle the kernel names it by. */
export function register_image(handle: u32, image: ImageBitmap): void {
  images.set(handle, image);
}

const TOPOLOGY: GPUPrimitiveTopology[] = ["triangle-list", "triangle-strip", "line-list"];
const CULL: GPUCullMode[] = ["none", "back"];
const COMPARE: GPUCompareFunction[] = ["always", "greater", "greater-equal", "less"];
const STEP: GPUVertexStepMode[] = ["vertex", "instance"];
const VERTEX_FORMAT: GPUVertexFormat[] = ["float32", "float32x2", "float32x3", "float32x4", "uint32x4", "uint32"];
const NONE = 0xffffffff;

/** Straight alpha over what is drawn. */
const ALPHA_BLEND: GPUBlendState = {
  color: { srcFactor: "src-alpha", dstFactor: "one-minus-src-alpha", operation: "add" },
  alpha: { srcFactor: "one", dstFactor: "one-minus-src-alpha", operation: "add" },
};

const word_f32 = new Float32Array(1);
const word_u32 = new Uint32Array(word_f32.buffer);

/** A word's bits as a float. */
function float(word: u32): f32 {
  word_u32[0] = word;
  return word_f32[0];
}

function texture_format(code: u32): GPUTextureFormat {
  switch (code) {
    case format.RGBA8_SRGB:
      return "rgba8unorm-srgb";
    case format.DEPTH32:
      return "depth32float";
    case format.RGBA32F:
      return "rgba32float";
    case format.R8:
      return "r8unorm";
    case format.RGBA8:
      return "rgba8unorm";
    default:
      return canvas_format;
  }
}

function get<T extends GpuObject>(id: u32): T {
  return objects[handle_slot(id)] as T;
}

function put(id: u32, object: GpuObject): void {
  objects[handle_slot(id)] = object;
}

function gpu(): GPUDevice {
  if (device === null) throw new Error("[gpu] a call with no device");
  return device;
}

function layout_entries(words: U32Array, samples: u32): GPUBindGroupLayoutEntry[] {
  const out: GPUBindGroupLayoutEntry[] = [];
  for (let k = 0; k < words.length; k += 3) {
    const [binding, stages, kind] = [words[k], words[k + 1], words[k + 2]];
    const e: GPUBindGroupLayoutEntry = { binding, visibility: (stages & record.VERTEX ? GPUShaderStage.VERTEX : 0) | (stages & record.FRAGMENT ? GPUShaderStage.FRAGMENT : 0) };
    if (kind === record.UNIFORM) e.buffer = { type: "uniform" };
    else if (kind === record.UNIFORM_DYNAMIC) e.buffer = { type: "uniform", hasDynamicOffset: true };
    else if (kind === record.STORAGE) e.buffer = { type: "read-only-storage" };
    else if (kind === record.TEXTURE) e.texture = {};
    else if (kind === record.TEXTURE_ARRAY) e.texture = { viewDimension: "2d-array" };
    else if (kind === record.DEPTH) e.texture = { sampleType: "depth" };
    else if (kind === record.FRAME_DEPTH) e.texture = { sampleType: "depth", multisampled: samples > 1 };
    else if (kind === record.TEXTURE_UNFILTERED) e.texture = { sampleType: "unfilterable-float" };
    else if (kind === record.SAMPLER) e.sampler = {};
    else if (kind === record.SAMPLER_COMPARISON) e.sampler = { type: "comparison" };
    out.push(e);
  }
  return out;
}

/** A pipeline record's words (crates/pxl/web/render/src/shader.rs) as WebGPU's descriptor. */
function pipeline_descriptor(words: U32Array, layouts: GPUBindGroupLayout[], module: GPUShaderModule, target: GPUTextureFormat, samples: u32): GPURenderPipelineDescriptor {
  let at = 11;
  const next = () => words[at++];
  const buffers: GPUVertexBufferLayout[] = Array.from({ length: next() }, () => {
    const arrayStride = next();
    const stepMode = STEP[next()];
    const attributes = Array.from({ length: next() }, (): GPUVertexAttribute => ({ shaderLocation: next(), offset: next(), format: VERTEX_FORMAT[next()] }));
    return { arrayStride, stepMode, attributes };
  });
  const descriptor: GPURenderPipelineDescriptor = {
    layout: gpu().createPipelineLayout({ bindGroupLayouts: layouts }),
    vertex: { module, entryPoint: "vs", buffers },
    primitive: { topology: TOPOLOGY[words[3]], cullMode: CULL[words[4]], frontFace: "ccw" },
    multisample: { count: samples },
  };
  if (words[2] !== 0) descriptor.fragment = { module, entryPoint: "fs", targets: [{ format: target, blend: words[9] === record.ALPHA ? ALPHA_BLEND : undefined }] };
  if (words[6] !== record.NO_DEPTH) {
    descriptor.depthStencil = { format: "depth32float", depthWriteEnabled: words[5] !== 0, depthCompare: COMPARE[words[6]], depthBias: words[7] | 0, depthBiasSlopeScale: float(words[8]) };
  }
  return descriptor;
}

function group_entries(words: U32Array): GPUBindGroupEntry[] {
  const out: GPUBindGroupEntry[] = [];
  for (let k = 0; k < words.length; k += 5) {
    const [binding, kind, id, offset, size] = [words[k], words[k + 1], words[k + 2], words[k + 3], words[k + 4]];
    if (kind === resource.BUFFER) out.push({ binding, resource: size === 0 ? { buffer: get<GPUBuffer>(id), offset } : { buffer: get<GPUBuffer>(id), offset, size } });
    else if (kind === resource.VIEW) out.push({ binding, resource: get<GPUTextureView>(id) });
    else out.push({ binding, resource: get<GPUSampler>(id) });
  }
  return out;
}

function begin_pass(encoder: GPUCommandEncoder, w: U32Array): GPURenderPassEncoder {
  const [color, resolve, color_load, r, g, b, a, depth, depth_load, depth_clear, query_set, begin, end] = w;
  const descriptor: GPURenderPassDescriptor = { colorAttachments: [] };
  if (color !== 0) {
    descriptor.colorAttachments = [{
      view: get<GPUTextureView>(color),
      resolveTarget: resolve !== 0 ? get<GPUTextureView>(resolve) : undefined,
      loadOp: color_load === load.LOAD ? "load" : "clear",
      clearValue: { r: float(r), g: float(g), b: float(b), a: float(a) },
      storeOp: "store",
    }];
  }
  if (depth !== 0) {
    const view = get<GPUTextureView>(depth);
    descriptor.depthStencilAttachment = depth_load === load.READ_ONLY
      ? { view, depthReadOnly: true }
      : { view, depthLoadOp: depth_load === load.LOAD ? "load" : "clear", depthClearValue: float(depth_clear), depthStoreOp: "store" };
  }
  if (query_set !== 0) {
    descriptor.timestampWrites = { querySet: get<GPUQuerySet>(query_set), beginningOfPassWriteIndex: begin === NONE ? undefined : begin, endOfPassWriteIndex: end === NONE ? undefined : end };
  }
  return encoder.beginRenderPass(descriptor);
}

// ---- timestamps ----

let stamps: { resolve: GPUBuffer; read: GPUBuffer[]; pending: Set<GPUBuffer> } | null = null;
let last_gpu_ms = 0;

/** Resolves the two timestamps an encoder's passes wrote and reads them once
 * the GPU has them, a few frames behind. */
function read_timestamps(encoder: GPUCommandEncoder, set: GPUQuerySet): void {
  const d = gpu();
  stamps ??= {
    resolve: d.createBuffer({ size: 16, usage: GPUBufferUsage.QUERY_RESOLVE | GPUBufferUsage.COPY_SRC }),
    read: Array.from({ length: 3 }, () => d.createBuffer({ size: 16, usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ })),
    pending: new Set(),
  };
  const s = stamps;
  const read = s.read.find((b) => !s.pending.has(b));
  if (!read) return;
  encoder.resolveQuerySet(set, 0, 2, s.resolve, 0);
  encoder.copyBufferToBuffer(s.resolve, 0, read, 0, 16);
  s.pending.add(read);
  // Mapped once the kernel's turn has submitted the encoder: the microtask
  // runs only after the call into the module returns.
  queueMicrotask(() =>
    read.mapAsync(GPUMapMode.READ).then(() => {
      const [t0, t1] = new BigUint64Array(read.getMappedRange());
      last_gpu_ms = Number(t1 - t0) / 1e6;
      read.unmap();
      s.pending.delete(read);
    }, () => s.pending.delete(read))
  );
}

// ---- the 2D scratch ----

let scratch: OffscreenCanvas | null = null;
let scratch_ctx: OffscreenCanvasRenderingContext2D | null = null;

function scratch_canvas(): [OffscreenCanvas, OffscreenCanvasRenderingContext2D] {
  if (!scratch) {
    scratch = new OffscreenCanvas(1, 1);
    scratch_ctx = scratch.getContext("2d");
  }
  return [scratch, scratch_ctx!];
}

function run_paint(w: U32Array, memory: WebAssembly.Memory): void {
  const [canvas, ctx] = scratch_canvas();
  let k = 0;
  const f = () => float(w[k++]);
  const text = () => {
    const at = w[k++];
    const len = w[k++];
    return utf8.decode(new Uint8Array(memory.buffer, at, len));
  };
  while (k < w.length) {
    const op = w[k++];
    switch (op) {
      case paint.SIZE: {
        const width = w[k++];
        const height = w[k++];
        if (canvas.width < width || canvas.height < height) {
          canvas.width = Math.max(canvas.width, width);
          canvas.height = Math.max(canvas.height, height);
        }
        ctx.clearRect(0, 0, width, height);
        break;
      }
      case paint.SAVE:
        ctx.save();
        break;
      case paint.RESTORE:
        ctx.restore();
        break;
      case paint.TRANSLATE:
        ctx.translate(f(), f());
        break;
      case paint.SCALE:
        ctx.scale(f(), f());
        break;
      case paint.SMOOTHING:
        ctx.imageSmoothingEnabled = w[k++] !== 0;
        break;
      case paint.FILL_STYLE:
        ctx.fillStyle = text();
        break;
      case paint.STROKE_STYLE:
        ctx.strokeStyle = text();
        break;
      case paint.LINE_WIDTH:
        ctx.lineWidth = f();
        break;
      case paint.FONT:
        ctx.font = text();
        break;
      case paint.TEXT_ALIGN:
        ctx.textAlign = w[k++] === 1 ? "center" : "left";
        break;
      case paint.TEXT_BASELINE:
        k++;
        ctx.textBaseline = "middle";
        break;
      case paint.BEGIN_PATH:
        ctx.beginPath();
        break;
      case paint.ROUND_RECT:
        ctx.roundRect(f(), f(), f(), f(), f());
        break;
      case paint.ARC:
        ctx.arc(f(), f(), f(), f(), f());
        break;
      case paint.MOVE_TO:
        ctx.moveTo(f(), f());
        break;
      case paint.LINE_TO:
        ctx.lineTo(f(), f());
        break;
      case paint.CLOSE_PATH:
        ctx.closePath();
        break;
      case paint.FILL:
        ctx.fill();
        break;
      case paint.STROKE:
        ctx.stroke();
        break;
      case paint.FILL_RECT:
        ctx.fillRect(f(), f(), f(), f());
        break;
      case paint.FILL_TEXT: {
        const s = text();
        ctx.fillText(s, f(), f());
        break;
      }
      case paint.IMAGE: {
        const image = images.get(w[k++]);
        const [sx, sy, sw, sh, dx, dy, dw, dh] = [f(), f(), f(), f(), f(), f(), f(), f()];
        if (image) ctx.drawImage(image, sx, sy, sw, sh, dx, dy, dw, dh);
        break;
      }
      default:
        throw new Error(`[gpu] paint operation ${op}`);
    }
  }
}

/** The kernel's WebGPU imports, over its memory. */
export function gpu_imports(memory: WebAssembly.Memory): WebAssembly.ModuleImports {
  const words = (at: u32, n: u32) => new Uint32Array(memory.buffer, at, n);
  // setBindGroup runs a few hundred times a frame, once or more per draw, so
  // a fresh view or array per call is garbage that grows with what is drawn.
  // One view over the whole memory, rebuilt only when growth detaches it, is
  // what the (data, start, length) overload reads the offsets from instead.
  let all = new Uint32Array(memory.buffer);
  const all_words = () => all.buffer === memory.buffer ? all : (all = new Uint32Array(memory.buffer));
  const surface = (handle: u32) => surfaces[handle_slot(handle)];
  return {
    gpu_reset: () => {
      for (const o of objects) (o as { destroy?: () => void } | undefined)?.destroy?.();
      objects.length = 0;
    },
    gpu_buffer: (id: u32, size: u32, usage: u32) => put(id, gpu().createBuffer({ size, usage })),
    gpu_texture: (id: u32, width: u32, height: u32, layers: u32, fmt: u32, mips: u32, samples: u32, usage: u32) =>
      put(id, gpu().createTexture({ size: { width, height, depthOrArrayLayers: layers }, format: texture_format(fmt), mipLevelCount: mips, sampleCount: samples, usage })),
    gpu_image_texture: (id: u32, image: u32) => {
      const bitmap = images.get(image);
      if (!bitmap) return 0;
      const texture = gpu().createTexture({
        size: [bitmap.width, bitmap.height],
        format: "rgba8unorm-srgb",
        usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST | GPUTextureUsage.RENDER_ATTACHMENT,
      });
      gpu().queue.copyExternalImageToTexture({ source: bitmap }, { texture }, [bitmap.width, bitmap.height]);
      put(id, texture);
      return 1;
    },
    gpu_view: (id: u32, texture: u32, dim: u32, base_mip: u32, mips: u32, base_layer: u32, layers: u32) =>
      put(
        id,
        get<GPUTexture>(texture).createView({
          dimension: dim === dimension.D2_ARRAY ? "2d-array" : "2d",
          baseMipLevel: base_mip,
          mipLevelCount: mips,
          baseArrayLayer: base_layer,
          arrayLayerCount: layers,
        }),
      ),
    gpu_sampler: (id: u32, mag: u32, min: u32, mipmap: u32, addr: u32, cmp: u32, anisotropy: u32) => {
      const filtered = (code: u32): GPUFilterMode => code === filter.LINEAR ? "linear" : "nearest";
      const mode: GPUAddressMode = addr === address.REPEAT ? "repeat" : "clamp-to-edge";
      put(
        id,
        gpu().createSampler({
          magFilter: filtered(mag),
          minFilter: filtered(min),
          mipmapFilter: filtered(mipmap),
          addressModeU: mode,
          addressModeV: mode,
          compare: cmp === compare.LESS_EQUAL ? "less-equal" : undefined,
          maxAnisotropy: anisotropy,
        }),
      );
    },
    gpu_shader: (id: u32, at: u32, len: u32) => put(id, gpu().createShaderModule({ code: utf8.decode(new Uint8Array(memory.buffer, at, len)) })),
    gpu_layout: (id: u32, at: u32, n: u32, samples: u32) => put(id, gpu().createBindGroupLayout({ entries: layout_entries(words(at, n), samples) })),
    gpu_pipeline: (id: u32, rec: u32, rec_words: u32, lay: u32, lay_count: u32, module: u32, fmt: u32, samples: u32) => {
      const layouts = Array.from(words(lay, lay_count), (l) => get<GPUBindGroupLayout>(l));
      put(id, gpu().createRenderPipeline(pipeline_descriptor(words(rec, rec_words), layouts, get<GPUShaderModule>(module), texture_format(fmt), samples)));
    },
    gpu_bind_group: (id: u32, layout: u32, at: u32, n: u32) => put(id, gpu().createBindGroup({ layout: get<GPUBindGroupLayout>(layout), entries: group_entries(words(at, n)) })),
    gpu_destroy: (id: u32) => {
      const slot = handle_slot(id);
      (objects[slot] as { destroy?: () => void } | undefined)?.destroy?.();
      objects[slot] = undefined;
    },
    gpu_write_buffer: (id: u32, offset: u32, at: u32, len: u32) => gpu().queue.writeBuffer(get<GPUBuffer>(id), offset, memory.buffer, at, len),
    gpu_write_texture: (id: u32, layer: u32, width: u32, height: u32, at: u32, len: u32) =>
      gpu().queue.writeTexture({ texture: get<GPUTexture>(id), origin: [0, 0, layer] }, new Uint8Array(memory.buffer, at, len), { bytesPerRow: width * 4 }, [width, height, 1]),
    gpu_encoder: (id: u32) => put(id, gpu().createCommandEncoder()),
    gpu_pass: (id: u32, encoder: u32, at: u32, n: u32) => put(id, begin_pass(get<GPUCommandEncoder>(encoder), words(at, n))),
    gpu_set_pipeline: (pass: u32, pipeline: u32) => get<GPURenderPassEncoder>(pass).setPipeline(get<GPURenderPipeline>(pipeline)),
    gpu_set_bind_group: (pass: u32, index: u32, group: u32, at: u32, count: u32) => {
      const p = get<GPURenderPassEncoder>(pass);
      if (count === 0) p.setBindGroup(index, get<GPUBindGroup>(group));
      else p.setBindGroup(index, get<GPUBindGroup>(group), all_words(), at >>> 2, count);
    },
    gpu_set_vertex_buffer: (pass: u32, slot: u32, buffer: u32, offset: u32, size: u32) => {
      const p = get<GPURenderPassEncoder>(pass);
      if (size === 0) p.setVertexBuffer(slot, get<GPUBuffer>(buffer), offset);
      else p.setVertexBuffer(slot, get<GPUBuffer>(buffer), offset, size);
    },
    gpu_set_index_buffer: (pass: u32, buffer: u32, offset: u32, size: u32) => {
      const p = get<GPURenderPassEncoder>(pass);
      if (size === 0) p.setIndexBuffer(get<GPUBuffer>(buffer), "uint32", offset);
      else p.setIndexBuffer(get<GPUBuffer>(buffer), "uint32", offset, size);
    },
    gpu_draw: (pass: u32, vertices: u32, instances: u32, first_vertex: u32, first_instance: u32) => get<GPURenderPassEncoder>(pass).draw(vertices, instances, first_vertex, first_instance),
    gpu_draw_indexed: (pass: u32, indices: u32, instances: u32, first_index: u32, first_instance: u32) =>
      get<GPURenderPassEncoder>(pass).drawIndexed(indices, instances, first_index, 0, first_instance),
    gpu_end: (pass: u32) => get<GPURenderPassEncoder>(pass).end(),
    gpu_submit: (encoder: u32) => gpu().queue.submit([get<GPUCommandEncoder>(encoder).finish()]),
    gpu_configure: (handle: u32) => {
      const s = surface(handle);
      if (!s) return;
      s.context ??= s.canvas.getContext("webgpu");
      s.context?.configure({ device: gpu(), format: canvas_format, alphaMode: "premultiplied" });
    },
    gpu_resize: (handle: u32, width: u32, height: u32) => {
      const s = surface(handle);
      if (!s) return;
      s.canvas.width = width;
      s.canvas.height = height;
    },
    gpu_current: (handle: u32, view: u32) => {
      const context = surface(handle)?.context;
      if (context) put(view, context.getCurrentTexture().createView());
    },
    gpu_query_set: (id: u32, count: u32) => put(id, gpu().createQuerySet({ type: "timestamp", count })),
    gpu_timestamps: (encoder: u32, set: u32) => read_timestamps(get<GPUCommandEncoder>(encoder), get<GPUQuerySet>(set)),
    gpu_ms: () => last_gpu_ms,
    gpu_paint: (at: u32, n: u32) => run_paint(words(at, n), memory),
    gpu_paint_copy: (texture: u32, x: u32, y: u32, layer: u32, width: u32, height: u32) =>
      gpu().queue.copyExternalImageToTexture({ source: scratch_canvas()[0] }, { texture: get<GPUTexture>(texture), origin: [x, y, layer] }, [width, height]),
  };
}
