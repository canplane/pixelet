// @xpute/gpu-web-host/host.ts

/**
 * This is the host's end of a guest's GPU calls on the web, as
 * gpu/native/host/src/run.rs is on a native device. It runs a stream's records
 * (gpu/link/spec/api.json) on a WebGPU device, and keeps each object the guest
 * made at its handle's slot. An enumeration's number is its place in the
 * spec's list, which gives WebGPU's string. The host is told the guest's
 * memory, the device and the surfaces the program binds, and nothing of what
 * the guest draws.
 */

import type { U32Array } from "@xpute/kit/abi/array.ts";
import type { u32 } from "@xpute/kit/abi/word.ts";
import { bytes_of, each_rec, text_of } from "./stream.ts";

import {
  NONE,
  op,
  RESULT,
  STATUS,
  SURFACE_PREFERRED,
  DEVICE,
} from "@xpute/gpu-link/api.spec.ts";
import { type GpuConfig } from "@xpute/gpu-link";
import { handle_slot } from "@xpute/gpu-link/handle.ts";
import { Belt } from "./belt.ts";
import { timed } from "./timed.ts";
import {
  ADDRESS_MODES,
  BLEND_FACTORS,
  BLEND_OPERATIONS,
  BUFFER_BINDING_TYPES,
  CANVAS_ALPHA_MODES,
  COMPARE_FUNCTIONS,
  CULL_MODES,
  FEATURE_NAMES,
  FILTER_MODES,
  FRONT_FACES,
  INDEX_FORMATS,
  LIMIT_NAMES,
  LOAD_OPS,
  MIPMAP_FILTER_MODES,
  PRIMITIVE_TOPOLOGIES,
  QUERY_TYPES,
  SAMPLER_BINDING_TYPES,
  STENCIL_OPERATIONS,
  STORAGE_TEXTURE_ACCESSES,
  STORE_OPS,
  TEXTURE_ASPECTS,
  TEXTURE_DIMENSIONS,
  TEXTURE_FORMATS,
  TEXTURE_SAMPLE_TYPES,
  TEXTURE_VIEW_DIMENSIONS,
  VERTEX_FORMATS,
  VERTEX_STEP_MODES,
} from "@xpute/gpu-link/enum.spec.ts";

type GpuObject =
  | GPUBuffer
  | GPUTexture
  | View
  | GPUSampler
  | GPUShaderModule
  | GPUBindGroupLayout
  | GPUPipelineLayout
  | GPUBindGroup
  | GPURenderPipeline
  | GPUComputePipeline
  | GPUQuerySet
  | GPUCommandEncoder
  | GPURenderPassEncoder
  | GPUComputePassEncoder;

/** This is a view with its format, which a pass's depth attachment needs to
 * know. */
type View = {
  readonly view: GPUTextureView;
  readonly format: GPUTextureFormat;
};

/** This is a texture's place in a copy, whose fields are refilled record by
 * record. */
type Place = {
  texture: GPUTexture;
  mipLevel: number;
  origin: number[];
  aspect: GPUTextureAspect;
};

function place(): Place {
  return {
    texture: undefined as unknown as GPUTexture,
    mipLevel: 0,
    origin: [0, 0, 0],
    aspect: "all",
  };
}

/** This is what the guest asked of a surface, whether or not a canvas is bound
 * to it, so a canvas bound later is configured and sized as asked: CONFIGURE's
 * configuration, RESIZE's size, and the handle and canvas context bound. */
type Surface = {
  cfg?: GPUCanvasConfiguration;
  size?: [u32, u32];
  bound?: { handle: u32; context: GPUCanvasContext };
};

/** This is a `GPUQueue` with `copyExternalImageToTexture`, which the lib's type
 * does not list. */
type ExternalImageQueue = GPUQueue & {
  copyExternalImageToTexture(
    source: { source: ImageBitmap },
    destination: GPUTexelCopyTextureInfo,
    size: GPUExtent3D,
  ): void;
};

/** These are a record's words, read in order as the guest wrote them. One cursor
 * runs over the whole stream and is set at each record's first word, so reading
 * a record allocates nothing. */
class WordCursor {
  private readonly _f: Float32Array;
  at: u32 = 0;
  readonly w: U32Array;
  constructor(w: U32Array) {
    this.w = w;
    this._f = new Float32Array(w.buffer, w.byteOffset, w.length);
  }
  u(): u32 {
    return this.w[this.at++];
  }
  i(): number {
    return this.u() | 0;
  }
  f32(): number {
    return this._f[this.at++];
  }
  flag(): boolean {
    return this.u() !== 0;
  }
  /** Steps past `n` words that the caller reads in place. */
  skip(n: u32): void {
    this.at += n;
  }
  /** This is undefined where the guest left the word unset (NONE). */
  opt(): u32 | undefined {
    const v = this.u();
    return v === NONE ? undefined : v;
  }
  bytes(): Uint8Array {
    const b = bytes_of(this.w, this.at);
    this.at += 1 + Math.ceil(b.length / 4);
    return b;
  }
  text(): string {
    const t = text_of(this.w, this.at);
    this.bytes();
    return t;
  }
}

export class GpuHost {
  private readonly _objects: (GpuObject | undefined)[] = [];
  /** This holds each async pipeline still being made: its handle, by its slot.
   * A slot let go since, or taken by another handle, no longer waits for the
   * pipeline. */
  private readonly _making = new Map<u32, u32>();
  /** These are the dictionaries a record's call takes, refilled for each call
   * rather than made anew, since WebGPU reads a dictionary only during the call
   * it is given to. There are two places because a texture-to-texture copy
   * names two. */
  private readonly _places: [Place, Place] = [place(), place()];
  private readonly _buffer_info: GPUTexelCopyBufferInfo = {
    buffer: undefined as unknown as GPUBuffer,
    offset: 0,
    bytesPerRow: 0,
    rowsPerImage: 0,
  };
  private readonly _layout: GPUTexelCopyBufferLayout = {
    offset: 0,
    bytesPerRow: 0,
    rowsPerImage: 0,
  };
  private readonly _size: number[] = [0, 0, 0];
  /** These are a pass's dictionaries, refilled the same way: its descriptors,
   * its color attachments (as many as any pass has had), its depth attachment
   * and its timestamps; and a submit's command buffers. */
  private readonly _pass: GPURenderPassDescriptor = { colorAttachments: [] };
  private readonly _colors: GPURenderPassColorAttachment[] = [];
  private readonly _depth: GPURenderPassDepthStencilAttachment = {
    view: undefined as unknown as GPUTextureView,
  };
  private readonly _compute: GPUComputePassDescriptor = {};
  private readonly _stamps: GPURenderPassTimestampWrites = {
    querySet: undefined as unknown as GPUQuerySet,
  };
  private readonly _cmd_bufs: GPUCommandBuffer[] = [];
  /** This holds each surface by the handle the guest issued it (gpu/wit
   * surface.wit), which the link never issues again. */
  private readonly _surfaces = new Map<u32, Surface>();
  /** This says where the guest lent the device's block, and is kept to write
   * its counts and its status later. */
  private _block: u32 | undefined;
  /** This counts the uncaptured errors the device raised (spec DEVICE). */
  private _error_cnt = 0;
  /** This counts the frames drawn whose work the queue has not yet done (spec
   * DEVICE). A run whose stream took a surface's texture draws a frame, which
   * stays unfinished until the queue's `onSubmittedWorkDone`, asked after the
   * run, resolves. */
  private _unfinished_cnt = 0;
  /** This run's stream has taken a surface's texture. */
  private _drew = false;

  /** Every buffer write is staged through the belt and copied by `_uploads`,
   * which is submitted ahead of what follows, as a device's host stages its
   * writes. */
  private readonly _belt: Belt;
  private _uploads: GPUCommandEncoder | null = null;

  private readonly _device: GPUDevice;
  private readonly _mem: () => ArrayBufferLike;
  private readonly _wake: () => void;
  private readonly _slow_ms: u32 | null;

  /** `wake` tells the guest a completion landed (gpu/wit/gpu.wit
   * `device.settled`), and that the device was lost. `cfg` is the program's
   * (@xpute/gpu-link `GpuConfig`). */
  constructor(
    device: GPUDevice,
    mem: () => ArrayBufferLike,
    wake: () => void,
    cfg: GpuConfig,
  ) {
    this._device = device;
    this._mem = mem;
    this._wake = wake;
    this._slow_ms = cfg.slow_ms;
    this._belt = new Belt(device, cfg.upload_chunk_bytes, cfg.slow_ms);
    // Nothing here destroys the device, so every loss is the browser's. The
    // guest is told of it in its block.
    device.lost.then(() => {
      this._on_lost();
      this._wake();
    });
    // An error no scope caught is logged and counted.
    device.addEventListener("uncapturederror", (e) => {
      console.warn(`[gpu] ${e.error.message}`);
      this._error_cnt++;
      this._tell_counts();
    });
  }

  /** Binds the canvas's `context` to the guest's surface `handle`, which
   * CONFIGURE, RESIZE and CURRENT name, and sizes and configures it as the
   * guest asked. */
  bind(handle: u32, context: GPUCanvasContext): void {
    const s = this._surface(handle);
    if (s.bound && s.bound.context !== context) s.bound.context.unconfigure();
    s.bound = { handle, context };
    if (s.size) [context.canvas.width, context.canvas.height] = s.size;
    if (s.cfg) context.configure(s.cfg);
  }

  /** Lets the surface `handle` go, with its context. */
  unbind(handle: u32): void {
    this._surfaces.get(handle)?.bound?.context.unconfigure();
    this._surfaces.delete(handle);
  }

  private _surface(handle: u32): Surface {
    let s = this._surfaces.get(handle);
    if (!s) this._surfaces.set(handle, (s = {}));
    return s;
  }

  /** Forgets every object of the device that went, and tells the guest in its
   * block. The block is then let go: the next device's host writes it, and
   * what this device answers late writes nothing. */
  private _on_lost(): void {
    this._objects.length = 0;
    this._making.clear();
    if (this._block !== undefined) {
      const block = new Uint32Array(this._mem(), this._block, DEVICE.WORDS);
      Atomics.store(block, DEVICE.STATUS_AT, STATUS.LOST);
      this._block = undefined;
    }
  }

  /** Runs the records `w` the guest lent. A frame they drew counts as
   * unfinished until the queue has done the work submitted with it. */
  run(w: U32Array): void {
    timed(this._slow_ms, "run", () => this._run(w));
  }

  /** `run` without the run's own time: each record is timed where the
   * program asked (timed.ts). */
  private _run(words: U32Array): void {
    let r: WordCursor | undefined;
    this._drew = false;
    const slow = this._slow_ms;
    each_rec(words, (code, w, a) => {
      r ??= new WordCursor(w);
      r.at = a;
      const rec = r;
      if (slow === null) return this._record(code, rec);
      timed(slow, `op ${code}`, () => this._record(code, rec));
    });
    // The run's staged writes reach the queue with the run, as
    // writeBuffer's would.
    if (this._uploads) this._submit([]);
    if (!this._drew) return;
    this._unfinished_cnt++;
    this._tell_counts();
    this._device.queue.onSubmittedWorkDone().then(() => {
      this._unfinished_cnt--;
      this._tell_counts();
    });
  }

  /** Stages `n` bytes of `src` from `src_at` through the belt, a chunk's room
   * at a time, and copies them into `buffer` at `offset` ahead of what
   * follows. No bytes write nothing, as in WebGPU. */
  private _write(
    buffer: GPUBuffer,
    offset: u32,
    src: ArrayBufferLike,
    src_at: u32,
    n: u32,
  ): void {
    for (let done = 0; done < n;) {
      const room = this._belt.take(n - done);
      room.bytes.set(new Uint8Array(src, src_at + done, room.n), room.at);
      this._uploads ??= this._device.createCommandEncoder();
      this._uploads.copyBufferToBuffer(
        room.chunk,
        room.at,
        buffer,
        offset + done,
        room.n,
      );
      done += room.n;
    }
  }

  /** Submits `then`, after the writes staged so far, and frees the belt's
   * chunks those writes took once the queue is done with them. */
  private _submit(then: GPUCommandBuffer[]): void {
    const uploads = this._uploads;
    this._uploads = null;
    if (!uploads) return this._device.queue.submit(then);
    this._belt.finish();
    this._device.queue.submit([uploads.finish(), ...then]);
    this._belt.recall();
  }

  /** Lets the block go: the guest has it back, and nothing more is written
   * there. */
  detach(): void {
    this._block = undefined;
  }

  /** Writes the device's block (spec DEVICE) into the guest's memory at
   * `at`, which the guest lent (gpu/wit/gpu.wit `device.lost`), STATUS last,
   * READY, with release; its counts are written there from then on. */
  attach(at: u32): void {
    const d = this._device;
    this._block = at;
    const out = new Uint32Array(this._mem(), at, DEVICE.WORDS);
    // A block lent again holds the last device's words. The guest reads those before
    // the counts only once STATUS is READY, and the counts and STATUS, which it loads
    // at any time, are stored atomically.
    out.fill(0, 0, DEVICE.ERRORS_AT);
    LIMIT_NAMES.forEach(
      (name, k) => (out[k] = Math.min(d.limits[name], 0xffffffff)),
    );
    FEATURE_NAMES.forEach((name, k) => {
      if (d.features.has(name as GPUFeatureName)) {
        out[DEVICE.FEATS_AT + (k >> 5)] |= 1 << (k & 31);
      }
    });
    // WebGPU resolves timestamps in nanoseconds, so the period is 1.
    new Float32Array(this._mem(), at, DEVICE.WORDS)[DEVICE.PERIOD_AT] = 1;
    out[DEVICE.FORMAT_AT] = (TEXTURE_FORMATS as readonly string[]).indexOf(
      navigator.gpu.getPreferredCanvasFormat(),
    );
    this._tell_counts();
    Atomics.store(out, DEVICE.STATUS_AT, STATUS.READY);
  }

  /** Stores the uncaptured errors and the unfinished frames with release
   * into the device's block (spec DEVICE). */
  private _tell_counts(): void {
    if (this._block === undefined) return;
    const block = new Uint32Array(this._mem(), this._block, DEVICE.WORDS);
    Atomics.store(block, DEVICE.ERRORS_AT, this._error_cnt);
    Atomics.store(block, DEVICE.UNFINISHED_AT, this._unfinished_cnt);
  }

  private _put(id: u32, o: GpuObject): void {
    this._objects[handle_slot(id)] = o;
  }

  private _get<T extends GpuObject>(id: u32): T {
    return this._objects[handle_slot(id)] as T;
  }

  private _forget(id: u32): void {
    this._objects[handle_slot(id)] = undefined;
    this._making.delete(handle_slot(id));
  }

  private _bytesAt(addr: u32, size: u32): Uint8Array {
    return new Uint8Array(this._mem(), addr, size);
  }

  /** Reads an async pipeline's fence and ticket, which follow the sync op's
   * words (RENDER_PIPELINE_ASYNC, COMPUTE_PIPELINE_ASYNC). Once `pipeline` gives
   * the pipeline, it is kept at `id` and its ticket stored, provided the
   * slot still waits for `id`. A pipeline the device refuses is made again
   * by the sync call, so its error is reported as the sync op's is. */
  private _make_async<P extends GPURenderPipeline | GPUComputePipeline>(
    id: u32,
    r: WordCursor,
    pipeline: Promise<P>,
    sync: () => P,
    label: string,
  ): void {
    const [word, ticket] = [r.u(), r.u()];
    const slot = handle_slot(id);
    this._making.set(slot, id);
    const waits = () => this._making.get(slot) === id;
    pipeline
      .catch((e: unknown) => {
        // Logged with its label, since the sync call's own error does not
        // name the pipeline on every device.
        console.warn(
          `[gpu] pipeline ${label} refused: ${e instanceof Error ? e.message : String(e)}`,
        );
        return waits() ? sync() : undefined;
      })
      .then((p) => {
        if (p === undefined || !waits()) return;
        this._making.delete(slot);
        this._put(id, p);
        this._signal(word, ticket);
      });
  }

  /** Answers the request asked with `ticket` at the fence `word` (spec
   * RESULT). Where the request has a result, it goes into the word after the
   * fence first; the ticket is then stored with release, so a guest that sees
   * the ticket sees the result. An entry is then asked for, to read it. */
  private _signal(word: u32, ticket: u32, result?: u32): void {
    const w = new Uint32Array(this._mem(), word, result === undefined ? 1 : 2);
    if (result !== undefined) w[1] = result;
    Atomics.store(w, 0, ticket);
    this._wake();
  }

  /** Reads a texture's place into `p`. `p` is one of `_places` where WebGPU
   * reads it only during the call, and a fresh one where it is kept past the
   * call. */
  private _texturePlace(r: WordCursor, p: Place): Place {
    p.texture = this._get<GPUTexture>(r.u());
    p.mipLevel = r.u();
    p.origin[0] = r.u();
    p.origin[1] = r.u();
    p.origin[2] = r.u();
    p.aspect = TEXTURE_ASPECTS[r.u()];
    return p;
  }

  private _bufferLayout(r: WordCursor): GPUTexelCopyBufferInfo {
    const b = this._buffer_info;
    b.buffer = this._get<GPUBuffer>(r.u());
    b.offset = r.u();
    b.bytesPerRow = r.u();
    b.rowsPerImage = r.u();
    return b;
  }

  private _extent(r: WordCursor): number[] {
    const e = this._size;
    e[0] = r.u();
    e[1] = r.u();
    e[2] = r.u();
    return e;
  }

  private _timestamps(r: WordCursor): GPURenderPassTimestampWrites | undefined {
    const set = r.opt();
    const t = this._stamps;
    t.beginningOfPassWriteIndex = r.opt();
    t.endOfPassWriteIndex = r.opt();
    if (set === undefined) return undefined;
    t.querySet = this._get<GPUQuerySet>(set);
    return t;
  }

  private _stencilFace(r: WordCursor): GPUStencilFaceState {
    return {
      compare: COMPARE_FUNCTIONS[r.u()],
      failOp: STENCIL_OPERATIONS[r.u()],
      depthFailOp: STENCIL_OPERATIONS[r.u()],
      passOp: STENCIL_OPERATIONS[r.u()],
    };
  }

  private _record(code: u32, r: WordCursor): void {
    const d = this._device;
    switch (code) {
      case op.BUFFER: {
        const id = r.u();
        this._put(
          id,
          d.createBuffer({
            size: r.u(),
            usage: r.u(),
            mappedAtCreation: r.flag(),
          }),
        );
        return;
      }
      case op.TEXTURE: {
        const id = r.u();
        const size = [r.u(), r.u(), r.u()];
        const [mipLevelCount, sampleCount, dimension, format, usage] = [
          r.u(),
          r.u(),
          TEXTURE_DIMENSIONS[r.u()],
          TEXTURE_FORMATS[r.u()],
          r.u(),
        ];
        const view_formats = Array.from(
          { length: r.u() },
          () => TEXTURE_FORMATS[r.u()],
        );
        this._put(
          id,
          d.createTexture({
            size,
            mipLevelCount,
            sampleCount,
            dimension,
            format,
            usage,
            viewFormats: view_formats,
          }),
        );
        return;
      }
      case op.VIEW: {
        const id = r.u();
        const texture = this._get<GPUTexture>(r.u());
        const [f, dim] = [r.opt(), r.opt()];
        const aspect = TEXTURE_ASPECTS[r.u()];
        const [baseMipLevel, mipLevelCount, baseArrayLayer, arrayLayerCount] = [
          r.u(),
          r.opt(),
          r.u(),
          r.opt(),
        ];
        const format = f === undefined ? undefined : TEXTURE_FORMATS[f];
        this._put(id, {
          view: texture.createView({
            format,
            dimension:
              dim === undefined ? undefined : TEXTURE_VIEW_DIMENSIONS[dim],
            aspect,
            baseMipLevel,
            mipLevelCount,
            baseArrayLayer,
            arrayLayerCount,
          }),
          format: format ?? texture.format,
        });
        return;
      }
      case op.SAMPLER: {
        const id = r.u();
        const [u, v, w, mag, min, mip, lodMinClamp, lodMaxClamp] = [
          r.u(),
          r.u(),
          r.u(),
          r.u(),
          r.u(),
          r.u(),
          r.f32(),
          r.f32(),
        ];
        const compare = r.opt();
        this._put(
          id,
          d.createSampler({
            addressModeU: ADDRESS_MODES[u],
            addressModeV: ADDRESS_MODES[v],
            addressModeW: ADDRESS_MODES[w],
            magFilter: FILTER_MODES[mag],
            minFilter: FILTER_MODES[min],
            mipmapFilter: MIPMAP_FILTER_MODES[mip],
            lodMinClamp,
            lodMaxClamp,
            compare:
              compare === undefined ? undefined : COMPARE_FUNCTIONS[compare],
            maxAnisotropy: r.u(),
          }),
        );
        return;
      }
      case op.SHADER_MODULE: {
        const id = r.u();
        this._put(id, d.createShaderModule({ code: r.text() }));
        return;
      }
      case op.BIND_GROUP_LAYOUT: {
        const id = r.u();
        const entries = Array.from(
          { length: r.u() },
          (): GPUBindGroupLayoutEntry => {
            const [binding, visibility, kind, a, b, c] = [
              r.u(),
              r.u(),
              r.u(),
              r.u(),
              r.u(),
              r.u(),
            ];
            switch (kind) {
              case 0:
                return {
                  binding,
                  visibility,
                  buffer: {
                    type: BUFFER_BINDING_TYPES[a],
                    hasDynamicOffset: b !== 0,
                    minBindingSize: c,
                  },
                };
              case 1:
                return {
                  binding,
                  visibility,
                  sampler: { type: SAMPLER_BINDING_TYPES[a] },
                };
              case 2:
                return {
                  binding,
                  visibility,
                  texture: {
                    sampleType: TEXTURE_SAMPLE_TYPES[a],
                    viewDimension: TEXTURE_VIEW_DIMENSIONS[b],
                    multisampled: c !== 0,
                  },
                };
              default:
                return {
                  binding,
                  visibility,
                  storageTexture: {
                    access: STORAGE_TEXTURE_ACCESSES[a],
                    format: TEXTURE_FORMATS[b],
                    viewDimension: TEXTURE_VIEW_DIMENSIONS[c],
                  },
                };
            }
          },
        );
        this._put(id, d.createBindGroupLayout({ entries }));
        return;
      }
      case op.PIPELINE_LAYOUT: {
        const id = r.u();
        this._put(
          id,
          d.createPipelineLayout({
            bindGroupLayouts: Array.from({ length: r.u() }, () =>
              this._get<GPUBindGroupLayout>(r.u()),
            ),
          }),
        );
        return;
      }
      case op.BIND_GROUP: {
        const id = r.u();
        const layout = this._get<GPUBindGroupLayout>(r.u());
        const entries = Array.from({ length: r.u() }, (): GPUBindGroupEntry => {
          const [binding, kind, object, offset, size] = [
            r.u(),
            r.u(),
            r.u(),
            r.u(),
            r.u(),
          ];
          if (kind === 0) {
            return {
              binding,
              resource: {
                buffer: this._get<GPUBuffer>(object),
                offset,
                size: size === NONE ? undefined : size,
              },
            };
          }
          return {
            binding,
            resource:
              kind === 1
                ? this._get<GPUSampler>(object)
                : this._get<View>(object).view,
          };
        });
        this._put(id, d.createBindGroup({ layout, entries }));
        return;
      }
      case op.RENDER_PIPELINE:
      case op.RENDER_PIPELINE_ASYNC: {
        const id = r.u();
        const layout = r.opt();
        const vertex: GPUVertexState = {
          module: this._get<GPUShaderModule>(r.u()),
          entryPoint: r.text(),
          buffers: Array.from({ length: r.u() }, (): GPUVertexBufferLayout => ({
            arrayStride: r.u(),
            stepMode: VERTEX_STEP_MODES[r.u()],
            // WebGPU's newer vertex formats are missing from the lib's type,
            // hence the cast.
            attributes: Array.from(
              { length: r.u() },
              (): GPUVertexAttribute => ({
                format: VERTEX_FORMATS[r.u()] as GPUVertexFormat,
                offset: r.u(),
                shaderLocation: r.u(),
              }),
            ),
          })),
        };
        const strip = (topology: u32, s: u32 | undefined) => ({
          topology: PRIMITIVE_TOPOLOGIES[topology],
          stripIndexFormat: s === undefined ? undefined : INDEX_FORMATS[s],
        });
        const primitive: GPUPrimitiveState = {
          ...strip(r.u(), r.opt()),
          frontFace: FRONT_FACES[r.u()],
          cullMode: CULL_MODES[r.u()],
          unclippedDepth: r.flag(),
        };
        const depth_format = r.opt();
        const depth: GPUDepthStencilState = {
          format: TEXTURE_FORMATS[depth_format ?? 0],
          depthWriteEnabled: r.flag(),
          depthCompare: COMPARE_FUNCTIONS[r.u()],
          stencilFront: this._stencilFace(r),
          stencilBack: this._stencilFace(r),
          stencilReadMask: r.u(),
          stencilWriteMask: r.u(),
          depthBias: r.i(),
          depthBiasSlopeScale: r.f32(),
          depthBiasClamp: r.f32(),
        };
        const multisample: GPUMultisampleState = {
          count: r.u(),
          mask: r.u(),
          alphaToCoverageEnabled: r.flag(),
        };
        const module = r.opt();
        const entry_point = r.text();
        const targets = Array.from(
          { length: r.u() },
          (): GPUColorTargetState => {
            const format = TEXTURE_FORMATS[r.u()];
            const [cop, csrc, cdst, aop, asrc, adst] = [
              r.u(),
              r.u(),
              r.u(),
              r.u(),
              r.u(),
              r.u(),
            ];
            // WebGPU's newer blend factors are missing from the lib's type
            // too, hence the cast.
            const factor = (k: u32) => BLEND_FACTORS[k] as GPUBlendFactor;
            const blend =
              cop === NONE
                ? undefined
                : {
                    color: {
                      operation: BLEND_OPERATIONS[cop],
                      srcFactor: factor(csrc),
                      dstFactor: factor(cdst),
                    },
                    alpha: {
                      operation: BLEND_OPERATIONS[aop],
                      srcFactor: factor(asrc),
                      dstFactor: factor(adst),
                    },
                  };
            return { format, blend, writeMask: r.u() };
          },
        );
        // The label is the pipeline's entry points, so a device's error
        // about the pipeline says which one it is.
        const desc: GPURenderPipelineDescriptor = {
          label:
            module === undefined
              ? vertex.entryPoint
              : `${vertex.entryPoint}+${entry_point}`,
          layout:
            layout === undefined
              ? "auto"
              : this._get<GPUPipelineLayout>(layout),
          vertex,
          primitive,
          depthStencil: depth_format === undefined ? undefined : depth,
          multisample,
          fragment:
            module === undefined
              ? undefined
              : {
                  module: this._get<GPUShaderModule>(module),
                  entryPoint: entry_point,
                  targets,
                },
        };
        if (code === op.RENDER_PIPELINE) {
          this._put(id, d.createRenderPipeline(desc));
        } else {
          this._make_async(
            id,
            r,
            d.createRenderPipelineAsync(desc),
            () => d.createRenderPipeline(desc),
            desc.label ?? "",
          );
        }
        return;
      }
      case op.COMPUTE_PIPELINE:
      case op.COMPUTE_PIPELINE_ASYNC: {
        const id = r.u();
        const layout = r.opt();
        const module = this._get<GPUShaderModule>(r.u());
        const entry_point = r.text();
        // The label is the pipeline's entry point, so a device's error about
        // the pipeline says which one it is.
        const desc: GPUComputePipelineDescriptor = {
          label: entry_point,
          layout:
            layout === undefined
              ? "auto"
              : this._get<GPUPipelineLayout>(layout),
          compute: { module, entryPoint: entry_point },
        };
        if (code === op.COMPUTE_PIPELINE) {
          this._put(id, d.createComputePipeline(desc));
        } else {
          this._make_async(
            id,
            r,
            d.createComputePipelineAsync(desc),
            () => d.createComputePipeline(desc),
            entry_point,
          );
        }
        return;
      }
      case op.QUERY_SET: {
        const id = r.u();
        this._put(
          id,
          d.createQuerySet({ type: QUERY_TYPES[r.u()], count: r.u() }),
        );
        return;
      }
      case op.DESTROY: {
        const id = r.u();
        const o = this._get<GpuObject>(id);
        // The writes staged so far reach the queue before what they write
        // goes, as a device's host submits them.
        if (
          (o instanceof GPUBuffer || o instanceof GPUTexture) &&
          this._uploads
        )
          this._submit([]);
        if (
          o instanceof GPUBuffer ||
          o instanceof GPUTexture ||
          o instanceof GPUQuerySet
        )
          o.destroy();
        this._forget(id);
        return;
      }
      case op.WRITE_BUFFER: {
        const buffer = this._get<GPUBuffer>(r.u());
        const offset = r.u();
        const n = r.u();
        this._write(buffer, offset, r.w.buffer, r.w.byteOffset + 4 * r.at, n);
        r.skip(Math.ceil(n / 4));
        return;
      }
      case op.WRITE_BUFFER_AT: {
        const buffer = this._get<GPUBuffer>(r.u());
        const offset = r.u();
        this._write(buffer, offset, this._mem(), r.u(), r.u());
        return;
      }
      case op.WRITE_TEXTURE:
      case op.WRITE_TEXTURE_AT: {
        const place = this._texturePlace(r, this._places[0]);
        const layout = this._layout;
        layout.bytesPerRow = r.u();
        layout.rowsPerImage = r.u();
        const size = this._extent(r);
        // The bytes are read where they lie: in the record after their count
        // for WRITE_TEXTURE, or at an address in the guest's memory for
        // WRITE_TEXTURE_AT. The layout's offset says where.
        let data: ArrayBuffer;
        if (code === op.WRITE_TEXTURE) {
          const n = r.u();
          data = r.w.buffer as ArrayBuffer;
          layout.offset = r.w.byteOffset + 4 * r.at;
          r.skip(Math.ceil(n / 4));
        } else {
          data = this._mem() as ArrayBuffer;
          layout.offset = r.u();
          r.skip(1);
        }
        d.queue.writeTexture(place, data, layout, size);
        return;
      }
      case op.COPY_EXTERNAL_IMAGE: {
        const to = this._texturePlace(r, place());
        const [flipY, premultipliedAlpha] = [r.flag(), r.flag()];
        // The bytes are copied now, since they are the guest's again once the
        // run is answered (gpu/wit/gpu.wit `run`).
        const image = new Blob([this._bytesAt(r.u(), r.u()).slice()]);
        const [word, ticket] = [r.u(), r.u()];
        // An image whose texture is gone, with the device or by the guest, is
        // not answered. One the browser cannot decode is answered REFUSED.
        createImageBitmap(image, {
          imageOrientation: flipY ? "flipY" : "from-image",
          premultiplyAlpha: premultipliedAlpha ? "premultiply" : "none",
        }).then(
          (source) => {
            if (this._objects.includes(to.texture)) {
              timed(this._slow_ms, "image", () =>
                (d.queue as ExternalImageQueue).copyExternalImageToTexture(
                  { source },
                  to,
                  [source.width, source.height],
                ),
              );
              this._signal(word, ticket, RESULT.OK);
            }
            source.close();
          },
          () => {
            if (this._objects.includes(to.texture)) {
              this._signal(word, ticket, RESULT.REFUSED);
            }
          },
        );
        return;
      }
      case op.ENCODER:
        this._put(r.u(), d.createCommandEncoder());
        return;
      case op.COPY_BUFFER_TO_BUFFER: {
        const e = this._get<GPUCommandEncoder>(r.u());
        e.copyBufferToBuffer(
          this._get<GPUBuffer>(r.u()),
          r.u(),
          this._get<GPUBuffer>(r.u()),
          r.u(),
          r.u(),
        );
        return;
      }
      case op.COPY_BUFFER_TO_TEXTURE: {
        const e = this._get<GPUCommandEncoder>(r.u());
        e.copyBufferToTexture(
          this._bufferLayout(r),
          this._texturePlace(r, this._places[0]),
          this._extent(r),
        );
        return;
      }
      case op.COPY_TEXTURE_TO_BUFFER: {
        const e = this._get<GPUCommandEncoder>(r.u());
        e.copyTextureToBuffer(
          this._texturePlace(r, this._places[0]),
          this._bufferLayout(r),
          this._extent(r),
        );
        return;
      }
      case op.COPY_TEXTURE_TO_TEXTURE: {
        const e = this._get<GPUCommandEncoder>(r.u());
        e.copyTextureToTexture(
          this._texturePlace(r, this._places[0]),
          this._texturePlace(r, this._places[1]),
          this._extent(r),
        );
        return;
      }
      case op.CLEAR_BUFFER: {
        const e = this._get<GPUCommandEncoder>(r.u());
        const [buffer, offset, size] = [
          this._get<GPUBuffer>(r.u()),
          r.u(),
          r.opt(),
        ];
        e.clearBuffer(buffer, offset, size);
        return;
      }
      case op.RESOLVE_QUERY_SET: {
        const e = this._get<GPUCommandEncoder>(r.u());
        e.resolveQuerySet(
          this._get<GPUQuerySet>(r.u()),
          r.u(),
          r.u(),
          this._get<GPUBuffer>(r.u()),
          r.u(),
        );
        return;
      }
      case op.RENDER_PASS: {
        const id = r.u();
        const e = this._get<GPUCommandEncoder>(r.u());
        const n = r.u();
        const colors = this._pass
          .colorAttachments as GPURenderPassColorAttachment[];
        colors.length = 0;
        for (let k = 0; k < n; k++) {
          const c = (this._colors[k] ??= {
            view: undefined as unknown as GPUTextureView,
            loadOp: "load",
            storeOp: "store",
            clearValue: [0, 0, 0, 0],
          });
          c.view = this._get<View>(r.u()).view;
          const resolve = r.opt();
          c.resolveTarget =
            resolve === undefined ? undefined : this._get<View>(resolve).view;
          c.loadOp = LOAD_OPS[r.u()];
          c.storeOp = STORE_OPS[r.u()];
          const clear = c.clearValue as number[];
          for (let i = 0; i < 4; i++) clear[i] = r.f32();
          colors.push(c);
        }
        const depth = r.opt();
        const dload = r.u(),
          dstore = r.u(),
          dclear = r.f32(),
          dro = r.flag(),
          sload = r.u(),
          sstore = r.u(),
          sclear = r.u(),
          sro = r.flag();
        this._pass.depthStencilAttachment = undefined;
        if (depth !== undefined) {
          // WebGPU refuses load and store ops on an aspect the format lacks,
          // or on one read only, so those are left unset.
          const { view, format } = this._get<View>(depth);
          const a = this._depth;
          a.view = view;
          const ops = format.startsWith("depth") && !dro;
          a.depthLoadOp = ops ? LOAD_OPS[dload] : undefined;
          a.depthStoreOp = ops ? STORE_OPS[dstore] : undefined;
          a.depthClearValue = ops ? dclear : undefined;
          a.depthReadOnly = ops ? undefined : dro;
          const stencil = format.includes("stencil") && !sro;
          a.stencilLoadOp = stencil ? LOAD_OPS[sload] : undefined;
          a.stencilStoreOp = stencil ? STORE_OPS[sstore] : undefined;
          a.stencilClearValue = stencil ? sclear : undefined;
          a.stencilReadOnly = stencil ? undefined : sro;
          this._pass.depthStencilAttachment = a;
        }
        this._pass.timestampWrites = this._timestamps(r);
        const occlusion = r.opt();
        this._pass.occlusionQuerySet =
          occlusion === undefined
            ? undefined
            : this._get<GPUQuerySet>(occlusion);
        this._put(id, e.beginRenderPass(this._pass));
        return;
      }
      case op.COMPUTE_PASS: {
        const id = r.u();
        const e = this._get<GPUCommandEncoder>(r.u());
        this._compute.timestampWrites = this._timestamps(r);
        this._put(id, e.beginComputePass(this._compute));
        return;
      }
      case op.SET_PIPELINE: {
        const pass = this._get<GPURenderPassEncoder | GPUComputePassEncoder>(
          r.u(),
        );
        const pipeline = this._get<GPURenderPipeline | GPUComputePipeline>(
          r.u(),
        );
        if (pass instanceof GPUComputePassEncoder) {
          pass.setPipeline(pipeline as GPUComputePipeline);
        } else pass.setPipeline(pipeline as GPURenderPipeline);
        return;
      }
      case op.SET_BIND_GROUP: {
        const pass = this._get<GPURenderPassEncoder | GPUComputePassEncoder>(
          r.u(),
        );
        const index = r.u();
        const group = this._get<GPUBindGroup>(r.u());
        const offsets = r.u();
        // A frame sets hundreds of bind groups, so the offsets are read where
        // they lie and no array is made.
        pass.setBindGroup(index, group, r.w, r.at, offsets);
        r.skip(offsets);
        return;
      }
      case op.SET_VERTEX_BUFFER: {
        const pass = this._get<GPURenderPassEncoder>(r.u());
        pass.setVertexBuffer(
          r.u(),
          this._get<GPUBuffer>(r.u()),
          r.u(),
          r.opt(),
        );
        return;
      }
      case op.SET_INDEX_BUFFER: {
        const pass = this._get<GPURenderPassEncoder>(r.u());
        pass.setIndexBuffer(
          this._get<GPUBuffer>(r.u()),
          INDEX_FORMATS[r.u()],
          r.u(),
          r.opt(),
        );
        return;
      }
      case op.SET_VIEWPORT:
        this._get<GPURenderPassEncoder>(r.u()).setViewport(
          r.f32(),
          r.f32(),
          r.f32(),
          r.f32(),
          r.f32(),
          r.f32(),
        );
        return;
      case op.SET_SCISSOR:
        this._get<GPURenderPassEncoder>(r.u()).setScissorRect(
          r.u(),
          r.u(),
          r.u(),
          r.u(),
        );
        return;
      case op.SET_BLEND_CONSTANT:
        this._get<GPURenderPassEncoder>(r.u()).setBlendConstant([
          r.f32(),
          r.f32(),
          r.f32(),
          r.f32(),
        ]);
        return;
      case op.SET_STENCIL_REFERENCE:
        this._get<GPURenderPassEncoder>(r.u()).setStencilReference(r.u());
        return;
      case op.DRAW:
        this._get<GPURenderPassEncoder>(r.u()).draw(r.u(), r.u(), r.u(), r.u());
        return;
      case op.DRAW_INDEXED:
        this._get<GPURenderPassEncoder>(r.u()).drawIndexed(
          r.u(),
          r.u(),
          r.u(),
          r.i(),
          r.u(),
        );
        return;
      case op.DRAW_INDIRECT:
        this._get<GPURenderPassEncoder>(r.u()).drawIndirect(
          this._get<GPUBuffer>(r.u()),
          r.u(),
        );
        return;
      case op.DRAW_INDEXED_INDIRECT:
        this._get<GPURenderPassEncoder>(r.u()).drawIndexedIndirect(
          this._get<GPUBuffer>(r.u()),
          r.u(),
        );
        return;
      case op.DISPATCH:
        this._get<GPUComputePassEncoder>(r.u()).dispatchWorkgroups(
          r.u(),
          r.u(),
          r.u(),
        );
        return;
      case op.DISPATCH_INDIRECT:
        this._get<GPUComputePassEncoder>(r.u()).dispatchWorkgroupsIndirect(
          this._get<GPUBuffer>(r.u()),
          r.u(),
        );
        return;
      case op.LABEL: {
        const o = this._get<GpuObject>(r.u());
        const label = r.text();
        if ("view" in o) o.view.label = label;
        else o.label = label;
        return;
      }
      case op.END: {
        const id = r.u();
        this._get<GPURenderPassEncoder | GPUComputePassEncoder>(id).end();
        this._forget(id);
        return;
      }
      case op.SUBMIT: {
        const n = r.u();
        const cmd_bufs = this._cmd_bufs;
        for (let k = 0; k < n; k++) {
          const id = r.u();
          cmd_bufs.push(this._get<GPUCommandEncoder>(id).finish());
          this._forget(id);
        }
        this._submit(cmd_bufs);
        cmd_bufs.length = 0;
        return;
      }
      case op.MAP_ASYNC: {
        const id = r.u();
        const buffer = this._get<GPUBuffer>(id);
        const offset = r.u();
        const size = r.u();
        const addr = r.u();
        const word = r.u();
        const ticket = r.u();
        // A read whose buffer is gone since, with the device (`lost`) or by
        // the guest, lands nowhere, as the native host drops its pending
        // reads. A read the device refused is answered REFUSED.
        buffer.mapAsync(GPUMapMode.READ, offset, size).then(
          () => {
            if (this._get<GPUBuffer>(id) !== buffer) return;
            timed(this._slow_ms, "map", () => {
              this._bytesAt(addr, size).set(
                new Uint8Array(buffer.getMappedRange(offset, size)),
              );
              buffer.unmap();
            });
            this._signal(word, ticket, RESULT.OK);
          },
          () => {
            if (this._get<GPUBuffer>(id) !== buffer) return;
            this._signal(word, ticket, RESULT.REFUSED);
          },
        );
        return;
      }
      case op.QUEUE_DONE: {
        const [word, ticket] = [r.u(), r.u()];
        d.queue.onSubmittedWorkDone().then(() => this._signal(word, ticket));
        return;
      }
      case op.CONFIGURE: {
        const s = this._surface(r.u());
        const format = r.u();
        const [usage, alphaMode] = [r.u(), CANVAS_ALPHA_MODES[r.u()]];
        const cfg: GPUCanvasConfiguration = {
          device: d,
          format:
            format === SURFACE_PREFERRED
              ? navigator.gpu.getPreferredCanvasFormat()
              : TEXTURE_FORMATS[format],
          usage,
          alphaMode,
        };
        s.cfg = cfg;
        s.bound?.context.configure(cfg);
        return;
      }
      case op.RESIZE: {
        const s = this._surface(r.u());
        const [width, height] = [r.u(), r.u()];
        s.size = [width, height];
        const canvas = s.bound?.context.canvas;
        if (canvas && (canvas.width !== width || canvas.height !== height)) {
          [canvas.width, canvas.height] = [width, height];
        }
        return;
      }
      case op.CURRENT: {
        const id = r.u();
        const texture = this._surfaces
          .get(r.u())
          ?.bound?.context.getCurrentTexture();
        if (!texture) return;
        this._put(id, { view: texture.createView(), format: texture.format });
        this._drew = true;
        return;
      }
      default:
        throw new Error(`[gpu] op ${code} is no op of gpu/link/spec/api.json`);
    }
  }
}
