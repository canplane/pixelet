// @pxl/web/kernel/render/gpu.ts

/**
 * The host's WebGPU: the device, and the calls the kernel's renderer made
 * in a turn (crates/pxl/web/render/src/gpu/api.rs), run from its stream once
 * the turn has returned. Every object is named by the handle the kernel gave
 * it and kept here at that handle's slot. Nothing is decided here — what is
 * made, from what, and drawn when, is the renderer's; each record is
 * translated into WebGPU's call and made. It exists because a WebAssembly
 * module reaches WebGPU no other way.
 *
 * A canvas handed to the kernel is a surface: transferred to an
 * OffscreenCanvas once, and drawn by the kernel alone from then on. The 2D
 * scratch is where the kernel paints what needs the host's fonts (the badge
 * sheet) or its decoded images (the texture pack), before copying it into a
 * texture.
 */

import type { U32Array } from "@xpute/core/abi/array.ts";
import type { f32, f64, u32 } from "@xpute/core/abi/word.ts";
import { handle_slot } from "@xpute/runtime/abi/handle.ts";
import { bytes_of, each_record } from "@xpute/runtime/ipc/stream.ts";

import type { UnsupportedReason } from "@/app/store/unsupported-store.ts";
import { DEV_BUNDLE } from "@/config/bundle.ts";
import { address, compare, dimension, filter, format, load, op, paint, record, resource, stream } from "@/kernel/render/gpu/api.spec.ts";

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
/** Why the browser answered with no device. The ways WebGPU goes missing are
 * fixed in different places, so `reason` picks the sentence the reader is
 * shown (app/store/unsupported-store.ts) rather than being one; `detail` is
 * the same fact for whoever is reading a console, which names what a sentence
 * for a reader should not. */
export interface NoDevice {
  reason: UnsupportedReason;
  detail: string;
}

/** What the browser is missing, where that is knowable without asking it for
 * anything — no adapter request, no module, no kernel. A browser with no
 * WebGPU has no use for any of those, so the page becomes the notice rather
 * than being covered by it (app/bootstrap.ts).
 *
 * Whether an adapter answers is not knowable here: it is a question for the
 * device request, and by then the module is loaded either way. */
export function missing_before_boot(): UnsupportedReason | null {
  if ("gpu" in navigator) return null;
  return isSecureContext ? "webgpu" : "insecure";
}

/** The device, once the browser answers, or why there is none. `lost` hears
 * it go. */
export async function acquire_device(lost: () => void): Promise<DeviceInfo | NoDevice> {
  // `navigator.gpu` is exposed to a secure context and nowhere else, so the
  // same browser on the same machine has WebGPU at localhost and none at the
  // dev server's LAN address over http.
  if (!("gpu" in navigator)) {
    return isSecureContext
      ? { reason: "webgpu", detail: "this browser has no navigator.gpu" }
      : { reason: "insecure", detail: `${location.origin} is not a secure context, which is the only place navigator.gpu is exposed` };
  }
  const adapter = await navigator.gpu.requestAdapter({ powerPreference: "high-performance" });
  if (!adapter) return { reason: "gpu", detail: "navigator.gpu is here but no adapter answered — the GPU is unavailable or blocklisted (chrome://gpu)" };
  const timestamps = adapter.features.has("timestamp-query");
  const d = await adapter.requestDevice({ requiredFeatures: timestamps ? ["timestamp-query"] : [] });
  // What the adapter says it can do, on a device nobody can attach a debugger
  // to. The terrain's vertex stage reads its boxes out of a storage buffer
  // (pxl-render frame/shader/chunk.wgsl), which is the one thing here a device
  // is allowed to refuse outright: OpenGL ES 3.1 hardware may offer none in
  // that stage at all, and a WebGPU running in compatibility mode says so in
  // these numbers rather than in an error.
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
const VERTEX_FORMAT: GPUVertexFormat[] = ["float32", "float32x2", "float32x3", "float32x4", "uint32x4", "uint32", "uint32x2"];
const NONE = 0xffffffff;

/** Straight alpha over what is drawn. */
const ALPHA_BLEND: GPUBlendState = {
  color: { srcFactor: "src-alpha", dstFactor: "one-minus-src-alpha", operation: "add" },
  alpha: { srcFactor: "one", dstFactor: "one-minus-src-alpha", operation: "add" },
};
/** Light added to what is drawn: the stars over the sky. */
const ADD_BLEND: GPUBlendState = {
  color: { srcFactor: "one", dstFactor: "one", operation: "add" },
  alpha: { srcFactor: "zero", dstFactor: "one", operation: "add" },
};
/** A record's blend code's state; none over what is opaque. */
const BLEND: Record<number, GPUBlendState | undefined> = { [record.OPAQUE]: undefined, [record.ALPHA]: ALPHA_BLEND, [record.ADD]: ADD_BLEND };

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
    case format.RGBA16F:
      return "rgba16float";
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

function layout_entries(words: U32Array): GPUBindGroupLayoutEntry[] {
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

/** The vertex entry points a pipeline record can name, by record.VS and kin. */
const VERTEX_ENTRY = ["vs", "vs_coarse"];

/** A pipeline record's words (crates/pxl/web/render/src/frame/shader.rs) as WebGPU's descriptor. */
function pipeline_descriptor(words: U32Array, layouts: GPUBindGroupLayout[], module: GPUShaderModule, target: GPUTextureFormat): GPURenderPipelineDescriptor {
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
    descriptor.timestampWrites = { querySet: get<GPUQuerySet>(query_set), beginningOfPassWriteIndex: begin === NONE ? undefined : begin, endOfPassWriteIndex: end === NONE ? undefined : end };
  }
  return encoder.beginRenderPass(descriptor);
}

// ---- timestamps ----

let stamps: { count: u32; resolve: GPUBuffer; read: GPUBuffer[]; pending: Set<GPUBuffer> } | null = null;
/** Where the last frame read goes: the stream's head, from GPU_MS — each
 * span's time, then the frame's whole — where the kernel reads it in a turn. */
let gpu_ms_at: { memory: ArrayBuffer; at: u32 } | null = null;

/** Resolves the timestamps an encoder's passes wrote into `set`, a begin and
 * an end a span, and reads them once the GPU has them, a few frames behind.
 * `written` says which spans' pairs this frame wrote: a query no pass wrote
 * keeps what an earlier frame left in it, so the rest are not read. */
function read_timestamps(encoder: GPUCommandEncoder, set: GPUQuerySet, written: u32): void {
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
  // Mapped once the stream has submitted the encoder: the microtask runs
  // only after the whole stream has.
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
      if (gpu_ms_at) new Float64Array(gpu_ms_at.memory, gpu_ms_at.at, spans.length).set(spans);
      read.unmap();
      s.pending.delete(read);
    }, () => s.pending.delete(read))
  );
}

// ---- the 2D scratch ----

/** A color the kernel packs as `0xRRGGBBAA`. */
function css_color(rgba: u32): string {
  return `rgba(${rgba >>> 24},${(rgba >>> 16) & 0xff},${(rgba >>> 8) & 0xff},${(rgba & 0xff) / 255})`;
}

let family: string | null = null;

/** The kernel's font — a weight and a size — in the page's own family, read
 * off the page once it is styled, so what the kernel paints and measures is
 * the face the page sets around it. */
export function css_font(weight: u32, px: f32): string {
  family ??= globalThis.document ? getComputedStyle(document.body).fontFamily : "sans-serif";
  return `${weight} ${px}px ${family}`;
}

let scratch: OffscreenCanvas | null = null;
let scratch_ctx: OffscreenCanvasRenderingContext2D | null = null;

function scratch_canvas(): [OffscreenCanvas, OffscreenCanvasRenderingContext2D] {
  if (!scratch) {
    scratch = new OffscreenCanvas(1, 1);
    scratch_ctx = scratch.getContext("2d");
  }
  return [scratch, scratch_ctx!];
}

function run_paint(w: U32Array): void {
  const [canvas, ctx] = scratch_canvas();
  let k = 0;
  const f = () => float(w[k++]);
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
        ctx.fillStyle = css_color(w[k++]);
        break;
      case paint.STROKE_STYLE:
        ctx.strokeStyle = css_color(w[k++]);
        break;
      case paint.LINE_WIDTH:
        ctx.lineWidth = f();
        break;
      case paint.FONT: {
        const weight = w[k++];
        ctx.font = css_font(weight, f());
        break;
      }
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
        const [x, y, len] = [f(), f(), w[k++]];
        ctx.fillText(utf8.decode(new Uint8Array(w.buffer, w.byteOffset + 4 * k, len)), x, y);
        k += (len + 3) >>> 2;
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

/** Runs the records the turn just returned left in the stream at `at`. With
 * no device they are dropped whole: the kernel hears the device went and
 * makes everything again on the next one. */
export function run_gpu_stream(memory: ArrayBuffer, at: u32): void {
  gpu_ms_at ??= { memory, at: at + stream.GPU_MS * 4 };
  if (device === null) return;
  const d = device;
  each_record(memory, at, stream.HEAD, (code, w, a, n) => {
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
          d.createTexture({ size: { width: w[a + 1], height: w[a + 2], depthOrArrayLayers: w[a + 3] }, format: texture_format(w[a + 4]), mipLevelCount: w[a + 5], usage: w[a + 6] }),
        );
        break;
      case op.IMAGE_TEXTURE: {
        const bitmap = images.get(w[a + 1])!;
        const texture = d.createTexture({
          size: [bitmap.width, bitmap.height],
          format: "rgba8unorm-srgb",
          usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST | GPUTextureUsage.RENDER_ATTACHMENT,
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
        put(w[a], d.createShaderModule({ code: utf8.decode(bytes_of(w, a + 1)) }));
        break;
      case op.LAYOUT:
        put(w[a], d.createBindGroupLayout({ entries: layout_entries(w.subarray(a + 1, k)) }));
        break;
      case op.PIPELINE: {
        const [id, module, fmt, len] = [w[a], w[a + 1], w[a + 2], w[a + 3]];
        const layouts = Array.from(w.subarray(a + 4 + len, k), (l) => get<GPUBindGroupLayout>(l));
        put(id, d.createRenderPipeline(pipeline_descriptor(w.subarray(a + 4, a + 4 + len), layouts, get<GPUShaderModule>(module), texture_format(fmt))));
        break;
      }
      case op.BIND_GROUP:
        put(w[a], d.createBindGroup({ layout: get<GPUBindGroupLayout>(w[a + 1]), entries: group_entries(w.subarray(a + 2, k)) }));
        break;
      case op.DESTROY: {
        const slot = handle_slot(w[a]);
        (objects[slot] as { destroy?: () => void } | undefined)?.destroy?.();
        objects[slot] = undefined;
        break;
      }
      case op.WRITE_BUFFER:
        d.queue.writeBuffer(get<GPUBuffer>(w[a]), w[a + 1], memory, w.byteOffset + 4 * (a + 3), w[a + 2]);
        break;
      case op.WRITE_TEXTURE:
        d.queue.writeTexture({ texture: get<GPUTexture>(w[a]), origin: [0, 0, w[a + 1]] }, bytes_of(w, a + 4), { bytesPerRow: w[a + 2] * 4 }, [w[a + 2], w[a + 3], 1]);
        break;
      case op.ENCODER:
        put(w[a], d.createCommandEncoder());
        break;
      case op.PASS:
        put(w[a], begin_pass(get<GPUCommandEncoder>(w[a + 1]), w.subarray(a + 2, k)));
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
        s.context?.configure({ device: d, format: canvas_format, alphaMode: "premultiplied" });
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
        read_timestamps(get<GPUCommandEncoder>(w[a]), get<GPUQuerySet>(w[a + 1]), w[a + 2]);
        break;
      case op.PAINT:
        run_paint(w.subarray(a, k));
        break;
      case op.PAINT_COPY:
        d.queue.copyExternalImageToTexture({ source: scratch_canvas()[0] }, { texture: get<GPUTexture>(w[a]), origin: [w[a + 1], w[a + 2], w[a + 3]] }, [w[a + 4], w[a + 5]]);
        break;
      default:
        throw new Error(`[gpu] record ${code}`);
    }
  });
}
