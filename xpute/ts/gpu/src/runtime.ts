// @xpute/gpu/runtime.ts

/**
 * This is the runtime's end of a core's GPU calls on the web, as
 * rust/gpu/src/runtime.rs is on a native device. It runs a stream's records
 * (spec/gpu/api.json) on a WebGPU device, and keeps each object the core made
 * at its handle's slot. An enumeration's number is its place in the spec's
 * list, which gives WebGPU's string. The runtime is told the core's memory,
 * the device and the surfaces the program binds, and nothing of what the
 * core draws.
 */

import type { U32Array } from "@xpute/kit/abi/array.ts";
import type { u32 } from "@xpute/kit/abi/word.ts";
import {
  GENERATION_BITS,
  handle_slot,
  SLOT_BITS,
} from "@xpute/runtime/abi/handle.ts";
import { bytes_of, each_rec, text_of } from "@xpute/runtime/ipc/stream.ts";

import { NONE, op, RESULT, STATUS, SURFACE_PREFERRED } from "./api.spec.ts";
import {
  ADDRESS_MODE,
  BLEND_FACTOR,
  BLEND_OPERATION,
  BUFFER_BINDING_TYPE,
  CANVAS_ALPHA_MODE,
  COMPARE_FUNCTION,
  CULL_MODE,
  FEATURE_NAME,
  FILTER_MODE,
  FRONT_FACE,
  INDEX_FORMAT,
  LIMIT_NAME,
  LOAD_OP,
  MIPMAP_FILTER_MODE,
  PRIMITIVE_TOPOLOGY,
  QUERY_TYPE,
  SAMPLER_BINDING_TYPE,
  STENCIL_OPERATION,
  STORAGE_TEXTURE_ACCESS,
  STORE_OP,
  TEXTURE_ASPECT,
  TEXTURE_DIMENSION,
  TEXTURE_FORMAT,
  TEXTURE_SAMPLE_TYPE,
  TEXTURE_VIEW_DIMENSION,
  VERTEX_FORMAT,
  VERTEX_STEP_MODE,
} from "./enum.spec.ts";

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

/** This is a `GPUQueue` with `copyExternalImageToTexture`, which the lib's type
 * does not list. */
type ExternalImageQueue = GPUQueue & {
  copyExternalImageToTexture(
    source: { source: ImageBitmap },
    destination: GPUTexelCopyTextureInfo,
    size: GPUExtent3D,
  ): void;
};

/** These are a record's words, read in order as the core wrote them. One cursor
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
  /** This is undefined where the core left the word unset (NONE). */
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

/** This says whether handle `a`'s generation is behind `b`'s. Generations wrap
 * in GENERATION_BITS. */
function older(a: u32, b: u32): boolean {
  const behind =
    ((a >>> SLOT_BITS) - (b >>> SLOT_BITS)) & ((1 << GENERATION_BITS) - 1);
  return behind >= 1 << (GENERATION_BITS - 1);
}

export class GpuRuntime {
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
  private readonly _command_bufs: GPUCommandBuffer[] = [];
  /** This holds each canvas context, at the slot the core names its surface by,
   * with the handle bound there. */
  private readonly _surfaces = new Map<
    u32,
    { handle: u32; context: GPUCanvasContext }
  >();
  /** What the core said of a slot before its canvas was bound, which the bind
   * applies: a bind and a resize reach a worker on two ports, in no order. */
  private readonly _before_bind = new Map<
    u32,
    { cfg?: GPUCanvasConfiguration; size?: [u32, u32] }
  >();
  /** This holds each slot's newest handle, kept after it is let go, so that an
   * older handle's bind is refused. */
  private readonly _seen = new Map<u32, u32>();
  /** This says where the core asked for the device's block, and is kept to
   * write its unfinished frames and its status later. */
  private _block: { addr: u32; words: u32 } | undefined;
  /** This counts the frames drawn whose work the queue has not yet done (spec
   * DEVICE). A turn whose stream took a surface's texture draws a frame, which
   * stays unfinished until the queue's `onSubmittedWorkDone`, asked after the
   * turn, resolves. */
  private _unfinished_cnt = 0;
  /** This turn's stream has taken a surface's texture. */
  private _drew = false;

  private readonly _device: GPUDevice;
  private readonly _mem: () => ArrayBufferLike;
  private readonly _wake: () => void;

  /** `wake` asks the runtime's loop for a turn. A completion written between
   * turns is read only by a turn, so each completion calls it. */
  constructor(device: GPUDevice, mem: () => ArrayBufferLike, wake: () => void) {
    this._device = device;
    this._mem = mem;
    this._wake = wake;
    // Nothing here destroys the device, so every loss is the browser's. The
    // core is told of it in its block.
    device.lost.then(() => {
      this._on_lost();
      this._wake();
    });
  }

  /** Binds the canvas's `context` to the core's surface `handle`, which
   * CONFIGURE, RESIZE and CURRENT name by its slot. A context the slot held
   * before is unconfigured. It answers false, binding nothing, where a newer
   * handle has bound the slot since. */
  bind(handle: u32, context: GPUCanvasContext): boolean {
    const slot = handle_slot(handle);
    const seen = this._seen.get(slot);
    if (seen !== undefined && older(handle, seen)) return false;
    this._seen.set(slot, handle);
    const was = this._surfaces.get(slot);
    if (was && was.context !== context) was.context.unconfigure();
    this._surfaces.set(slot, { handle, context });
    const before = this._before_bind.get(slot);
    if (before) {
      this._before_bind.delete(slot);
      if (before.size) {
        [context.canvas.width, context.canvas.height] = before.size;
      }
      if (before.cfg) context.configure(before.cfg);
    }
    return true;
  }

  /** Lets `handle`'s context go, if `handle` is still the one bound in its
   * slot, and says whether it was. An older handle unbinds no newer one. */
  unbind(handle: u32): boolean {
    const slot = handle_slot(handle);
    const bound = this._surfaces.get(slot);
    if (handle === 0 || bound?.handle !== handle) return false;
    bound.context.unconfigure();
    this._surfaces.delete(slot);
    return true;
  }

  private _unbound(slot: u32): {
    cfg?: GPUCanvasConfiguration;
    size?: [u32, u32];
  } {
    let held = this._before_bind.get(slot);
    if (!held) this._before_bind.set(slot, (held = {}));
    return held;
  }

  /** Forgets every object of the device that went, and tells the core in its
   * block. */
  private _on_lost(): void {
    this._objects.length = 0;
    this._making.clear();
    if (this._block) {
      const block = new Uint32Array(
        this._mem(),
        this._block.addr,
        this._block.words,
      );
      Atomics.store(block, this._block.words - 1, STATUS.LOST);
    }
  }

  /** Runs the stream at `at` as the turn left it, and hands it back. A frame
   * the stream drew counts as unfinished until the queue has done the work
   * submitted with it. */
  run(at: u32): void {
    let r: WordCursor | undefined;
    this._drew = false;
    each_rec(this._mem(), at, (code, w, a) => {
      r ??= new WordCursor(w);
      r.at = a;
      this._record(code, r);
    });
    if (!this._drew) return;
    this._tell_unfinished(this._unfinished_cnt + 1);
    this._device.queue
      .onSubmittedWorkDone()
      .then(() => this._tell_unfinished(this._unfinished_cnt - 1));
  }

  /** Keeps the count of unfinished frames, and stores it with release into
   * the device's block (spec DEVICE). */
  private _tell_unfinished(cnt: number): void {
    this._unfinished_cnt = cnt;
    if (!this._block) return;
    const block = new Uint32Array(
      this._mem(),
      this._block.addr,
      this._block.words,
    );
    Atomics.store(block, this._block.words - 2, cnt);
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
        console.error(
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
   * fence first; the ticket is then stored with release, so a core that sees
   * the ticket sees the result. A turn is then asked for, to read it. */
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
    p.aspect = TEXTURE_ASPECT[r.u()];
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
      compare: COMPARE_FUNCTION[r.u()],
      failOp: STENCIL_OPERATION[r.u()],
      depthFailOp: STENCIL_OPERATION[r.u()],
      passOp: STENCIL_OPERATION[r.u()],
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
          TEXTURE_DIMENSION[r.u()],
          TEXTURE_FORMAT[r.u()],
          r.u(),
        ];
        const viewFormats = Array.from(
          { length: r.u() },
          () => TEXTURE_FORMAT[r.u()],
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
            viewFormats,
          }),
        );
        return;
      }
      case op.VIEW: {
        const id = r.u();
        const texture = this._get<GPUTexture>(r.u());
        const [f, dim] = [r.opt(), r.opt()];
        const aspect = TEXTURE_ASPECT[r.u()];
        const [baseMipLevel, mipLevelCount, baseArrayLayer, arrayLayerCount] = [
          r.u(),
          r.opt(),
          r.u(),
          r.opt(),
        ];
        const format = f === undefined ? undefined : TEXTURE_FORMAT[f];
        this._put(id, {
          view: texture.createView({
            format,
            dimension:
              dim === undefined ? undefined : TEXTURE_VIEW_DIMENSION[dim],
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
            addressModeU: ADDRESS_MODE[u],
            addressModeV: ADDRESS_MODE[v],
            addressModeW: ADDRESS_MODE[w],
            magFilter: FILTER_MODE[mag],
            minFilter: FILTER_MODE[min],
            mipmapFilter: MIPMAP_FILTER_MODE[mip],
            lodMinClamp,
            lodMaxClamp,
            compare:
              compare === undefined ? undefined : COMPARE_FUNCTION[compare],
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
                    type: BUFFER_BINDING_TYPE[a],
                    hasDynamicOffset: b !== 0,
                    minBindingSize: c,
                  },
                };
              case 1:
                return {
                  binding,
                  visibility,
                  sampler: { type: SAMPLER_BINDING_TYPE[a] },
                };
              case 2:
                return {
                  binding,
                  visibility,
                  texture: {
                    sampleType: TEXTURE_SAMPLE_TYPE[a],
                    viewDimension: TEXTURE_VIEW_DIMENSION[b],
                    multisampled: c !== 0,
                  },
                };
              default:
                return {
                  binding,
                  visibility,
                  storageTexture: {
                    access: STORAGE_TEXTURE_ACCESS[a],
                    format: TEXTURE_FORMAT[b],
                    viewDimension: TEXTURE_VIEW_DIMENSION[c],
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
            stepMode: VERTEX_STEP_MODE[r.u()],
            // WebGPU's newer vertex formats are missing from the lib's type,
            // hence the cast.
            attributes: Array.from(
              { length: r.u() },
              (): GPUVertexAttribute => ({
                format: VERTEX_FORMAT[r.u()] as GPUVertexFormat,
                offset: r.u(),
                shaderLocation: r.u(),
              }),
            ),
          })),
        };
        const strip = (topology: u32, s: u32 | undefined) => ({
          topology: PRIMITIVE_TOPOLOGY[topology],
          stripIndexFormat: s === undefined ? undefined : INDEX_FORMAT[s],
        });
        const primitive: GPUPrimitiveState = {
          ...strip(r.u(), r.opt()),
          frontFace: FRONT_FACE[r.u()],
          cullMode: CULL_MODE[r.u()],
          unclippedDepth: r.flag(),
        };
        const depthFormat = r.opt();
        const depth: GPUDepthStencilState = {
          format: TEXTURE_FORMAT[depthFormat ?? 0],
          depthWriteEnabled: r.flag(),
          depthCompare: COMPARE_FUNCTION[r.u()],
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
        const entryPoint = r.text();
        const targets = Array.from(
          { length: r.u() },
          (): GPUColorTargetState => {
            const format = TEXTURE_FORMAT[r.u()];
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
            const factor = (k: u32) => BLEND_FACTOR[k] as GPUBlendFactor;
            const blend =
              cop === NONE
                ? undefined
                : {
                    color: {
                      operation: BLEND_OPERATION[cop],
                      srcFactor: factor(csrc),
                      dstFactor: factor(cdst),
                    },
                    alpha: {
                      operation: BLEND_OPERATION[aop],
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
              : `${vertex.entryPoint}+${entryPoint}`,
          layout:
            layout === undefined
              ? "auto"
              : this._get<GPUPipelineLayout>(layout),
          vertex,
          primitive,
          depthStencil: depthFormat === undefined ? undefined : depth,
          multisample,
          fragment:
            module === undefined
              ? undefined
              : {
                  module: this._get<GPUShaderModule>(module),
                  entryPoint,
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
        const entryPoint = r.text();
        // The label is the pipeline's entry point, so a device's error about
        // the pipeline says which one it is.
        const desc: GPUComputePipelineDescriptor = {
          label: entryPoint,
          layout:
            layout === undefined
              ? "auto"
              : this._get<GPUPipelineLayout>(layout),
          compute: { module, entryPoint },
        };
        if (code === op.COMPUTE_PIPELINE) {
          this._put(id, d.createComputePipeline(desc));
        } else {
          this._make_async(
            id,
            r,
            d.createComputePipelineAsync(desc),
            () => d.createComputePipeline(desc),
            entryPoint,
          );
        }
        return;
      }
      case op.QUERY_SET: {
        const id = r.u();
        this._put(
          id,
          d.createQuerySet({ type: QUERY_TYPE[r.u()], count: r.u() }),
        );
        return;
      }
      case op.DESTROY: {
        const id = r.u();
        const o = this._get<GpuObject>(id);
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
        d.queue.writeBuffer(
          buffer,
          offset,
          r.w.buffer as ArrayBuffer,
          r.w.byteOffset + 4 * r.at,
          n,
        );
        r.skip(Math.ceil(n / 4));
        return;
      }
      case op.WRITE_BUFFER_AT: {
        const buffer = this._get<GPUBuffer>(r.u());
        const offset = r.u();
        d.queue.writeBuffer(
          buffer,
          offset,
          this._mem() as ArrayBuffer,
          r.u(),
          r.u(),
        );
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
        // for WRITE_TEXTURE, or at an address in the core's memory for
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
        // The bytes are copied now, since they are the core's again once the
        // stream is handed back (xpute-core ipc/stream.rs R3).
        const image = new Blob([this._bytesAt(r.u(), r.u()).slice()]);
        const [word, ticket] = [r.u(), r.u()];
        // An image whose texture is gone, with the device or by the core, is
        // not answered. One the browser cannot decode is answered REFUSED.
        createImageBitmap(image, {
          imageOrientation: flipY ? "flipY" : "from-image",
          premultiplyAlpha: premultipliedAlpha ? "premultiply" : "none",
        }).then(
          (source) => {
            if (this._objects.includes(to.texture)) {
              (d.queue as ExternalImageQueue).copyExternalImageToTexture(
                { source },
                to,
                [source.width, source.height],
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
          c.loadOp = LOAD_OP[r.u()];
          c.storeOp = STORE_OP[r.u()];
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
          a.depthLoadOp = ops ? LOAD_OP[dload] : undefined;
          a.depthStoreOp = ops ? STORE_OP[dstore] : undefined;
          a.depthClearValue = ops ? dclear : undefined;
          a.depthReadOnly = ops ? undefined : dro;
          const stencil = format.includes("stencil") && !sro;
          a.stencilLoadOp = stencil ? LOAD_OP[sload] : undefined;
          a.stencilStoreOp = stencil ? STORE_OP[sstore] : undefined;
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
          INDEX_FORMAT[r.u()],
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
        const command_bufs = this._command_bufs;
        for (let k = 0; k < n; k++) {
          const id = r.u();
          command_bufs.push(this._get<GPUCommandEncoder>(id).finish());
          this._forget(id);
        }
        d.queue.submit(command_bufs);
        command_bufs.length = 0;
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
        // the core, lands nowhere, as the native runtime drops its pending
        // reads. A read the device refused is answered REFUSED.
        buffer.mapAsync(GPUMapMode.READ, offset, size).then(
          () => {
            if (this._get<GPUBuffer>(id) !== buffer) return;
            this._bytesAt(addr, size).set(
              new Uint8Array(buffer.getMappedRange(offset, size)),
            );
            buffer.unmap();
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
        const slot = r.u();
        const context = this._surfaces.get(slot)?.context;
        const format = r.u();
        const [usage, alphaMode] = [r.u(), CANVAS_ALPHA_MODE[r.u()]];
        const cfg: GPUCanvasConfiguration = {
          device: d,
          format:
            format === SURFACE_PREFERRED
              ? navigator.gpu.getPreferredCanvasFormat()
              : TEXTURE_FORMAT[format],
          usage,
          alphaMode,
        };
        if (context) context.configure(cfg);
        else this._unbound(slot).cfg = cfg;
        return;
      }
      case op.RESIZE: {
        const slot = r.u();
        const canvas = this._surfaces.get(slot)?.context.canvas;
        const [width, height] = [r.u(), r.u()];
        if (!canvas) this._unbound(slot).size = [width, height];
        else if (canvas.width !== width || canvas.height !== height) {
          [canvas.width, canvas.height] = [width, height];
        }
        return;
      }
      case op.CURRENT: {
        const id = r.u();
        const texture = this._surfaces.get(r.u())?.context.getCurrentTexture();
        if (!texture) return;
        this._put(id, { view: texture.createView(), format: texture.format });
        this._drew = true;
        return;
      }
      case op.DEVICE: {
        const [addr, words] = [r.u(), r.u()];
        this._block = { addr, words };
        const out = new Uint32Array(this._mem(), addr, words);
        LIMIT_NAME.forEach(
          (name, k) => (out[k] = Math.min(d.limits[name], 0xffffffff)),
        );
        FEATURE_NAME.forEach((name, k) => {
          if (d.features.has(name as GPUFeatureName)) {
            out[LIMIT_NAME.length + (k >> 5)] |= 1 << (k & 31);
          }
        });
        // WebGPU resolves timestamps in nanoseconds, so the period is 1.
        const period = LIMIT_NAME.length + Math.ceil(FEATURE_NAME.length / 32);
        new Float32Array(this._mem(), addr, words)[period] = 1;
        out[period + 1] = (TEXTURE_FORMAT as readonly string[]).indexOf(
          navigator.gpu.getPreferredCanvasFormat(),
        );
        out[words - 2] = this._unfinished_cnt;
        Atomics.store(out, words - 1, STATUS.READY);
        return;
      }
      default:
        throw new Error(`[gpu] op ${code} is no op of spec/gpu/api.json`);
    }
  }
}
