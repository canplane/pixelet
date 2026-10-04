// @pxl/web/platform/web/webgpu.ts

/**
 * The host's WebGPU: runs the calls the kernel's renderer recorded in a turn,
 * each object kept at the slot of the handle the kernel gave it. A wasm
 * module reaches WebGPU no other way.
 */

import type { U32Array } from "@xpute/core/abi/array.ts";
import type { f32, f64, u32 } from "@xpute/core/abi/word.ts";
import { handle_slot } from "@xpute/runtime/abi/handle.ts";
import { bytes_of, each_record, text_of } from "@xpute/runtime/ipc/stream.ts";

import { DEV_BUNDLE } from "@/config/bundle.ts";
import { address, compare, dimension, filter, format, load, op, record, resource, stamp, stream } from "@/kernel/render/gpu/api.spec.ts";

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
let canvasFormat: GPUTextureFormat = "bgra8unorm";
const objects: (GpuObject | undefined)[] = [];
const surfaces: (Surface | undefined)[] = [];
/** A canvas transfers once: an effect mounted again is handed the same one. */
const transferred = new WeakMap<HTMLCanvasElement, OffscreenCanvas>();
const images = new Map<u32, ImageBitmap>();

interface DeviceInfo {
  align: u32;
  timestamps: boolean;
}
export const enum Missing {
  WebGpu = "webgpu",
  Insecure = "insecure",
  Gpu = "gpu",
}

/** `navigator.gpu` exists only in a secure context, so the LAN address over
 * http has none where localhost has it. */
export async function whatIsMissing(): Promise<Missing | null> {
  if (!("gpu" in navigator)) return isSecureContext ? Missing.WebGpu : Missing.Insecure;
  const adapter = await navigator.gpu.requestAdapter({ powerPreference: "high-performance" });
  return adapter ? null : Missing.Gpu;
}

export async function acquireDevice(lost: () => void): Promise<DeviceInfo | null> {
  const adapter = await navigator.gpu.requestAdapter({ powerPreference: "high-performance" });
  if (!adapter) return null;
  const timestamps = adapter.features.has("timestamp-query");
  const d = await adapter.requestDevice({ requiredFeatures: timestamps ? ["timestamp-query"] : [] });
  // A compatibility-mode WebGPU (GLES 3.1 hardware) may offer no storage
  // buffers in the vertex stage, which the terrain needs; it shows here only.
  if (DEV_BUNDLE) {
    const l = d.limits as unknown as Record<string, number>;
    const of = (k: string) => `${k}=${l[k] ?? "-"}`;
    const info = (adapter as unknown as { info?: Record<string, string> }).info;
    console.info(
      `[gpu] adapter ${info ? `${info.vendor ?? "?"}/${info.architecture ?? "?"}/${info.device ?? "?"}` : "?"} features [${[...d.features].join(",")}] ` +
        [
          "maxStorageBuffersPerShaderStage",
          "maxStorageBuffersInVertexStage",
          "maxStorageBufferBindingSize",
          "maxBufferSize",
          "minStorageBufferOffsetAlignment",
          "minUniformBufferOffsetAlignment",
          "maxBindGroups",
        ].map(of).join(" "),
    );
  }
  d.lost.then((info) => {
    // Nothing here destroys the device, so a loss is always the browser's.
    console.warn(`[gpu] device lost (${info.reason}): ${info.message}`);
    if (device !== d) return;
    device = null;
    objects.length = 0;
    stamps = null;
    lost();
  });
  d.onuncapturederror = (e) => console.error(`[gpu] ${e.error.message}`);
  device = d;
  canvasFormat = navigator.gpu.getPreferredCanvasFormat();
  return { align: d.limits.minUniformBufferOffsetAlignment, timestamps };
}

export function bindSurface(handle: u32, canvas: HTMLCanvasElement): void {
  let offscreen = transferred.get(canvas);
  if (!offscreen) {
    offscreen = canvas.transferControlToOffscreen();
    transferred.set(canvas, offscreen);
  }
  surfaces[handle_slot(handle)] = { canvas: offscreen, context: null };
}

export function unbindSurface(handle: u32): void {
  surfaces[handle_slot(handle)] = undefined;
}

export function registerImage(handle: u32, image: ImageBitmap): void {
  images.set(handle, image);
}

const TOPOLOGY: Record<number, GPUPrimitiveTopology> = { [record.TRIANGLES]: "triangle-list", [record.STRIP]: "triangle-strip", [record.LINES]: "line-list" };
const CULL: Record<number, GPUCullMode> = { [record.CULL_NONE]: "none", [record.CULL_BACK]: "back" };
const COMPARE: Record<number, GPUCompareFunction> = { [record.ALWAYS]: "always", [record.GREATER]: "greater", [record.GREATER_EQUAL]: "greater-equal", [record.LESS]: "less" };
const STEP: Record<number, GPUVertexStepMode> = { [record.PER_VERTEX]: "vertex", [record.PER_INSTANCE]: "instance" };
const VERTEX_FORMAT: Record<number, GPUVertexFormat> = {
  [record.F32]: "float32",
  [record.F32X2]: "float32x2",
  [record.F32X3]: "float32x3",
  [record.F32X4]: "float32x4",
  [record.U32X4]: "uint32x4",
  [record.U32]: "uint32",
  [record.U32X2]: "uint32x2",
};
const VERTEX_ENTRY: Record<number, string> = { [record.VS]: "vs", [record.VS_COARSE]: "vs_coarse", [record.VS_DEPTH]: "vs_depth", [record.VS_COARSE_DEPTH]: "vs_coarse_depth" };

const ALPHA_BLEND: GPUBlendState = {
  color: { srcFactor: "src-alpha", dstFactor: "one-minus-src-alpha", operation: "add" },
  alpha: { srcFactor: "one", dstFactor: "one-minus-src-alpha", operation: "add" },
};
const ADD_BLEND: GPUBlendState = {
  color: { srcFactor: "one", dstFactor: "one", operation: "add" },
  alpha: { srcFactor: "zero", dstFactor: "one", operation: "add" },
};
const BLEND: Record<number, GPUBlendState | undefined> = { [record.OPAQUE]: undefined, [record.ALPHA]: ALPHA_BLEND, [record.ADD]: ADD_BLEND };

const wordF32 = new Float32Array(1);
const wordU32 = new Uint32Array(wordF32.buffer);

function float(word: u32): f32 {
  wordU32[0] = word;
  return wordF32[0];
}

function textureFormat(code: u32): GPUTextureFormat {
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
    case format.RGBA16F:
      return "rgba16float";
    default:
      return canvasFormat;
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

function layoutEntries(words: U32Array): GPUBindGroupLayoutEntry[] {
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
    else if (kind === record.TEXTURE_UNFILTERED) e.texture = { sampleType: "unfilterable-float" };
    else if (kind === record.SAMPLER) e.sampler = {};
    else if (kind === record.SAMPLER_COMPARISON) e.sampler = { type: "comparison" };
    out.push(e);
  }
  return out;
}

function pipelineDescriptor(words: U32Array, layouts: GPUBindGroupLayout[], module: GPUShaderModule, target: GPUTextureFormat): GPURenderPipelineDescriptor {
  let at = record.HEAD;
  const next = () => words[at++];
  const buffers: GPUVertexBufferLayout[] = Array.from({ length: next() }, () => {
    const arrayStride = next();
    const stepMode = STEP[next()];
    const attributes = Array.from({ length: next() }, (): GPUVertexAttribute => ({ shaderLocation: next(), offset: next(), format: VERTEX_FORMAT[next()] }));
    return { arrayStride, stepMode, attributes };
  });
  const descriptor: GPURenderPipelineDescriptor = {
    layout: gpu().createPipelineLayout({ bindGroupLayouts: layouts }),
    vertex: { module, entryPoint: VERTEX_ENTRY[words[record.HEAD - 1]], buffers },
    primitive: { topology: TOPOLOGY[words[2]], cullMode: CULL[words[3]], frontFace: "ccw" },
  };
  if (words[1] !== 0) descriptor.fragment = { module, entryPoint: "fs", targets: [{ format: target, blend: BLEND[words[8]] }] };
  if (words[5] !== record.NO_DEPTH) {
    descriptor.depthStencil = { format: "depth32float", depthWriteEnabled: words[4] !== 0, depthCompare: COMPARE[words[5]], depthBias: words[6] | 0, depthBiasSlopeScale: float(words[7]) };
  }
  return descriptor;
}

function groupEntries(words: U32Array): GPUBindGroupEntry[] {
  const out: GPUBindGroupEntry[] = [];
  for (let k = 0; k < words.length; k += 5) {
    const [binding, kind, id, offset, size] = [words[k], words[k + 1], words[k + 2], words[k + 3], words[k + 4]];
    if (kind === resource.BUFFER) out.push({ binding, resource: size === 0 ? { buffer: get<GPUBuffer>(id), offset } : { buffer: get<GPUBuffer>(id), offset, size } });
    else if (kind === resource.VIEW) out.push({ binding, resource: get<GPUTextureView>(id) });
    else out.push({ binding, resource: get<GPUSampler>(id) });
  }
  return out;
}

function beginPass(encoder: GPUCommandEncoder, w: U32Array): GPURenderPassEncoder {
  const [color, color_load, r, g, b, a, depth, depth_load, depth_clear, query_set, begin, end] = w;
  const descriptor: GPURenderPassDescriptor = { colorAttachments: [] };
  if (color !== 0) {
    descriptor.colorAttachments = [{
      view: get<GPUTextureView>(color),
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
    descriptor.timestampWrites = {
      querySet: get<GPUQuerySet>(query_set),
      beginningOfPassWriteIndex: begin === stamp.NONE ? undefined : begin,
      endOfPassWriteIndex: end === stamp.NONE ? undefined : end,
    };
  }
  return encoder.beginRenderPass(descriptor);
}

let stamps: { count: u32; resolve: GPUBuffer; read: GPUBuffer[]; pending: Set<GPUBuffer> } | null = null;
let gpuMsAt: { memory: ArrayBuffer; at: u32 } | null = null;

/** A query no pass wrote this frame keeps an old value, so only `written`
 * spans are read. */
function readTimestamps(encoder: GPUCommandEncoder, set: GPUQuerySet, written: u32): void {
  const d = gpu();
  const count = set.count;
  if (stamps?.count !== count) {
    const bytes = count * 8;
    stamps = {
      count,
      resolve: d.createBuffer({ size: bytes, usage: GPUBufferUsage.QUERY_RESOLVE | GPUBufferUsage.COPY_SRC }),
      read: Array.from({ length: 3 }, () => d.createBuffer({ size: bytes, usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ })),
      pending: new Set(),
    };
  }
  const s = stamps;
  const read = s.read.find((b) => !s.pending.has(b));
  if (!read) return;
  encoder.resolveQuerySet(set, 0, count, s.resolve, 0);
  encoder.copyBufferToBuffer(s.resolve, 0, read, 0, count * 8);
  s.pending.add(read);
  // The microtask runs only after the whole stream has submitted.
  queueMicrotask(() =>
    read.mapAsync(GPUMapMode.READ).then(() => {
      const t = new BigUint64Array(read.getMappedRange());
      const spans: f64[] = [];
      let [first, last] = [0n, 0n];
      for (let k = 0; 2 * k < count; k++) {
        const [b, e] = [t[2 * k], t[2 * k + 1]];
        const ran = (written & (1 << k)) !== 0 && e >= b;
        spans.push(ran ? Number(e - b) / 1e6 : 0);
        if (!ran) continue;
        if (first === 0n || b < first) first = b;
        if (e > last) last = e;
      }
      spans.push(Number(last - first) / 1e6);
      if (gpuMsAt) new Float64Array(gpuMsAt.memory, gpuMsAt.at, spans.length).set(spans);
      read.unmap();
      s.pending.delete(read);
    }, () => s.pending.delete(read))
  );
}

/** With no device the records are dropped: the kernel remakes everything on the next one. */
export function runGpuStream(memory: ArrayBuffer, at: u32): void {
  gpuMsAt ??= { memory, at: at + stream.GPU_MS * 4 };
  if (device === null) return;
  each_record(memory, at, stream.HEAD, runGpuRecord);
}

/** One record of the stream, on the device runGpuStream found. */
function runGpuRecord(code: u32, w: U32Array, a: u32, n: u32): void {
  const d = device!;
  const k = a + n;
  switch (code) {
    case op.RESET:
      for (const o of objects) (o as { destroy?: () => void } | undefined)?.destroy?.();
      objects.length = 0;
      break;
    case op.BUFFER: {
      const [id, size, usage] = [w[a], w[a + 1], w[a + 2]];
      // Wrapped in an out-of-memory scope because a refused buffer is not
      // an exception: WebGPU hands back an object that is invalid, and
      // nothing says so until something uses it — by which point the kernel
      // has long since recorded the id as live. The mesh arena asks for six
      // buffers of 36–40 MiB the moment the first mesh is written, and a
      // device that refuses one draws no terrain at all while every gauge
      // reads normal, since `arena_bytes` counts a group by its id and the
      // draw that finds no bind group is skipped without a word (pxl-render
      // gpu/arena.rs, gpu/renderer.rs).
      //
      // Guarded on what comes back rather than on what is there, because a
      // check stands in its own device here and answers the calls a kernel
      // makes, not the ones an instrument does.
      d.pushErrorScope?.("out-of-memory");
      put(id, d.createBuffer({ size, usage }));
      const scope = d.popErrorScope?.() as Promise<GPUError | null> | undefined;
      if (typeof scope?.then === "function") {
        scope.then((e) => {
          if (e) console.error(`[gpu] buffer ${id} of ${(size / 1048576).toFixed(1)}MB refused: ${e.message}`);
        }).catch(() => {});
      }
      break;
    }
    case op.TEXTURE:
      put(
        w[a],
        d.createTexture({ size: { width: w[a + 1], height: w[a + 2], depthOrArrayLayers: w[a + 3] }, format: textureFormat(w[a + 4]), mipLevelCount: w[a + 5], usage: w[a + 6] }),
      );
      break;
    case op.IMAGE_TEXTURE: {
      const bitmap = images.get(w[a + 1])!;
      const texture = d.createTexture({
        size: [bitmap.width, bitmap.height],
        format: "rgba8unorm-srgb",
        usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_SRC | GPUTextureUsage.COPY_DST | GPUTextureUsage.RENDER_ATTACHMENT,
      });
      d.queue.copyExternalImageToTexture({ source: bitmap }, { texture }, [bitmap.width, bitmap.height]);
      put(w[a], texture);
      break;
    }
    case op.VIEW:
      put(
        w[a],
        get<GPUTexture>(w[a + 1]).createView({
          dimension: w[a + 2] === dimension.D2_ARRAY ? "2d-array" : "2d",
          baseMipLevel: w[a + 3],
          mipLevelCount: w[a + 4],
          baseArrayLayer: w[a + 5],
          arrayLayerCount: w[a + 6],
        }),
      );
      break;
    case op.SAMPLER: {
      const filtered = (code: u32): GPUFilterMode => code === filter.LINEAR ? "linear" : "nearest";
      const mode: GPUAddressMode = w[a + 4] === address.REPEAT ? "repeat" : "clamp-to-edge";
      put(
        w[a],
        d.createSampler({
          magFilter: filtered(w[a + 1]),
          minFilter: filtered(w[a + 2]),
          mipmapFilter: filtered(w[a + 3]),
          addressModeU: mode,
          addressModeV: mode,
          compare: w[a + 5] === compare.LESS_EQUAL ? "less-equal" : undefined,
          maxAnisotropy: w[a + 6],
        }),
      );
      break;
    }
    case op.SHADER:
      put(w[a], d.createShaderModule({ code: text_of(w, a + 1) }));
      break;
    case op.LAYOUT:
      put(w[a], d.createBindGroupLayout({ entries: layoutEntries(w.subarray(a + 1, k)) }));
      break;
    case op.PIPELINE: {
      const [id, module, fmt, len] = [w[a], w[a + 1], w[a + 2], w[a + 3]];
      const layouts = Array.from(w.subarray(a + 4 + len, k), (l) => get<GPUBindGroupLayout>(l));
      put(id, d.createRenderPipeline(pipelineDescriptor(w.subarray(a + 4, a + 4 + len), layouts, get<GPUShaderModule>(module), textureFormat(fmt))));
      break;
    }
    case op.BIND_GROUP:
      put(w[a], d.createBindGroup({ layout: get<GPUBindGroupLayout>(w[a + 1]), entries: groupEntries(w.subarray(a + 2, k)) }));
      break;
    case op.DESTROY: {
      const slot = handle_slot(w[a]);
      (objects[slot] as { destroy?: () => void } | undefined)?.destroy?.();
      objects[slot] = undefined;
      break;
    }
    case op.WRITE_BUFFER:
      d.queue.writeBuffer(get<GPUBuffer>(w[a]), w[a + 1], w.buffer, w.byteOffset + 4 * (a + 3), w[a + 2]);
      break;
    case op.WRITE_TEXTURE:
      d.queue.writeTexture({ texture: get<GPUTexture>(w[a]), origin: [0, 0, w[a + 1]] }, bytes_of(w, a + 4), { bytesPerRow: w[a + 2] * 4 }, [w[a + 2], w[a + 3], 1]);
      break;
    case op.ENCODER:
      put(w[a], d.createCommandEncoder());
      break;
    case op.COPY_TEXTURE:
      get<GPUCommandEncoder>(w[a]).copyTextureToTexture(
        { texture: get<GPUTexture>(w[a + 1]), origin: [w[a + 2], w[a + 3], 0] },
        { texture: get<GPUTexture>(w[a + 4]), origin: [w[a + 5], w[a + 6], w[a + 7]] },
        [w[a + 8], w[a + 9], 1],
      );
      break;
    case op.PASS:
      put(w[a], beginPass(get<GPUCommandEncoder>(w[a + 1]), w.subarray(a + 2, k)));
      break;
    case op.SET_PIPELINE:
      get<GPURenderPassEncoder>(w[a]).setPipeline(get<GPURenderPipeline>(w[a + 1]));
      break;
    // setBindGroup runs a few hundred times a frame, once or more per draw,
    // so a fresh array per call is garbage that grows with what is drawn:
    // the (data, start, length) overload reads the offsets where they lie.
    case op.SET_BIND_GROUP:
      if (n === 3) get<GPURenderPassEncoder>(w[a]).setBindGroup(w[a + 1], get<GPUBindGroup>(w[a + 2]));
      else get<GPURenderPassEncoder>(w[a]).setBindGroup(w[a + 1], get<GPUBindGroup>(w[a + 2]), w, a + 3, n - 3);
      break;
    case op.SET_VERTEX_BUFFER: {
      const p = get<GPURenderPassEncoder>(w[a]);
      if (w[a + 4] === 0) p.setVertexBuffer(w[a + 1], get<GPUBuffer>(w[a + 2]), w[a + 3]);
      else p.setVertexBuffer(w[a + 1], get<GPUBuffer>(w[a + 2]), w[a + 3], w[a + 4]);
      break;
    }
    case op.SET_INDEX_BUFFER: {
      const p = get<GPURenderPassEncoder>(w[a]);
      if (w[a + 3] === 0) p.setIndexBuffer(get<GPUBuffer>(w[a + 1]), "uint32", w[a + 2]);
      else p.setIndexBuffer(get<GPUBuffer>(w[a + 1]), "uint32", w[a + 2], w[a + 3]);
      break;
    }
    case op.DRAW:
      get<GPURenderPassEncoder>(w[a]).draw(w[a + 1], w[a + 2], w[a + 3], w[a + 4]);
      break;
    case op.DRAW_INDEXED:
      get<GPURenderPassEncoder>(w[a]).drawIndexed(w[a + 1], w[a + 2], w[a + 3], 0, w[a + 4]);
      break;
    case op.END:
      get<GPURenderPassEncoder>(w[a]).end();
      break;
    case op.SUBMIT:
      d.queue.submit([get<GPUCommandEncoder>(w[a]).finish()]);
      break;
    case op.CONFIGURE: {
      const s = surfaces[handle_slot(w[a])];
      if (!s) break;
      s.context ??= s.canvas.getContext("webgpu");
      s.context?.configure({ device: d, format: canvasFormat, alphaMode: "premultiplied" });
      break;
    }
    case op.RESIZE: {
      const s = surfaces[handle_slot(w[a])];
      if (!s) break;
      s.canvas.width = w[a + 1];
      s.canvas.height = w[a + 2];
      break;
    }
    case op.CURRENT: {
      const context = surfaces[handle_slot(w[a])]?.context;
      if (context) put(w[a + 1], context.getCurrentTexture().createView());
      break;
    }
    case op.QUERY_SET:
      put(w[a], d.createQuerySet({ type: "timestamp", count: w[a + 1] }));
      break;
    case op.TIMESTAMPS:
      readTimestamps(get<GPUCommandEncoder>(w[a]), get<GPUQuerySet>(w[a + 1]), w[a + 2]);
      break;
    default:
      throw new Error(`[gpu] record ${code}`);
  }
}
