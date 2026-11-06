// @xpute/gpu/runtime.ts

/**
 * The runtime's end of a core's GPU calls: runs a stream's records
 * (spec/gpu/api.json) on a WebGPU device, each object kept at its handle's
 * slot, each enumeration's number turned into WebGPU's string by its place.
 * It is told the core's memory, the device and the surfaces the program
 * binds, and nothing of what the core draws.
 */

import type { U32Array } from "@xpute/kit/abi/array.ts";
import type { u32 } from "@xpute/kit/abi/word.ts";
import {
  GENERATION_BITS,
  handle_slot,
  SLOT_BITS,
} from "@xpute/runtime/abi/handle.ts";
import { bytes_of, each_record, text_of } from "@xpute/runtime/ipc/stream.ts";

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

/** A view with its format, which a pass's depth attachment asks of it. */
type View = {
  readonly view: GPUTextureView;
  readonly format: GPUTextureFormat;
};

/** A texture's place in a copy, its fields refilled record by record. */
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

/** `copyExternalImageToTexture`, which the lib's `GPUQueue` does not list. */
type ExternalImageQueue = GPUQueue & {
  copyExternalImageToTexture(
    source: { source: ImageBitmap },
    destination: GPUTexelCopyTextureInfo,
    size: GPUExtent3D,
  ): void;
};

/** A run's words read in order, as the core wrote them: one cursor over
 * the stream, set at each record's first word, so a record makes nothing. */
class Words {
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
  /** Past `n` words, read in place by the caller. */
  skip(n: u32): void {
    this.at += n;
  }
  /** None where the core left it unset. */
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

/** Whether `a`'s generation is behind `b`'s, generations wrapping in their bits. */
function older(a: u32, b: u32): boolean {
  const behind =
    ((a >>> SLOT_BITS) - (b >>> SLOT_BITS)) & ((1 << GENERATION_BITS) - 1);
  return behind >= 1 << (GENERATION_BITS - 1);
}

export class GpuRuntime {
  private readonly _objects: (GpuObject | undefined)[] = [];
  /** Each async pipeline still being made, by its handle's slot: its handle,
   * so a slot let go since, or taken by another, waits for it no more. */
  private readonly _making = new Map<u32, u32>();
  /** What a record's call takes, refilled each time rather than made: WebGPU
   * reads a dictionary only during the call it is given to. Two places, as
   * a texture-to-texture copy names two. */
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
  /** A pass's: its descriptors, its color attachments (as many as one has
   * had), its depth attachment, its timestamps; a submit's buffers. */
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
  /** At the slot the core names each by, with the core's handle bound there. */
  private readonly _surfaces = new Map<
    u32,
    { handle: u32; context: GPUCanvasContext }
  >();
  /** Each slot's newest handle, kept after it is let go: an older one's bind is refused. */
  private readonly _seen = new Map<u32, u32>();
  /** Where the core asked the device's block written, for its frames
   * unfinished and its status. */
  private _block: { address: u32; words: u32 } | undefined;
  /** The frames drawn whose work the queue has not yet done (spec DEVICE):
   * a turn's whose stream took a surface's texture, until the queue's
   * `onSubmittedWorkDone` asked after it resolves. */
  private _unfinished_cnt = 0;
  /** This turn's stream has taken a surface's texture. */
  private _drew = false;

  private readonly _device: GPUDevice;
  private readonly _memory: () => ArrayBufferLike;
  private readonly _wake: () => void;

  /** `wake` asks the runtime's loop for a turn: a completion written between
   * turns is read only by a turn. */
  constructor(
    device: GPUDevice,
    memory: () => ArrayBufferLike,
    wake: () => void,
  ) {
    this._device = device;
    this._memory = memory;
    this._wake = wake;
    // The core is told of the loss in its block; nothing here destroys the device.
    device.lost.then(() => {
      this._on_lost();
      this._wake();
    });
  }

  /** The canvas's `context` for the core's surface `handle`, named by its
   * slot in CONFIGURE, RESIZE and CURRENT; refused where a newer handle has
   * bound the slot since. */
  bind(handle: u32, context: GPUCanvasContext): boolean {
    const slot = handle_slot(handle);
    const seen = this._seen.get(slot);
    if (seen !== undefined && older(handle, seen)) return false;
    this._seen.set(slot, handle);
    const was = this._surfaces.get(slot);
    if (was && was.context !== context) was.context.unconfigure();
    this._surfaces.set(slot, { handle, context });
    return true;
  }

  /** Lets `handle`'s context go where it is still the one bound under its
   * slot: an older handle unbinds no newer one. */
  unbind(handle: u32): boolean {
    const slot = handle_slot(handle);
    const bound = this._surfaces.get(slot);
    if (handle === 0 || bound?.handle !== handle) return false;
    bound.context.unconfigure();
    this._surfaces.delete(slot);
    return true;
  }

  /** The device went: every object with it, and the core told in its block. */
  private _on_lost(): void {
    this._objects.length = 0;
    this._making.clear();
    if (this._block) {
      const block = new Uint32Array(
        this._memory(),
        this._block.address,
        this._block.words,
      );
      Atomics.store(block, this._block.words - 1, STATUS.LOST);
    }
  }

  /** Runs the stream at `at` as the turn left it, from past the head it
   * says; a frame it drew counted unfinished until the queue has done the
   * work submitted with it. */
  run(at: u32): void {
    let r: Words | undefined;
    this._drew = false;
    each_record(this._memory(), at, (code, w, a) => {
      r ??= new Words(w);
      r.at = a;
      this._record(code, r);
    });
    if (!this._drew) return;
    this._tell_unfinished(this._unfinished_cnt + 1);
    this._device.queue
      .onSubmittedWorkDone()
      .then(() => this._tell_unfinished(this._unfinished_cnt - 1));
  }

  /** The frames unfinished, kept and told the core in its block with
   * release (spec DEVICE). */
  private _tell_unfinished(cnt: number): void {
    this._unfinished_cnt = cnt;
    if (!this._block) return;
    const block = new Uint32Array(
      this._memory(),
      this._block.address,
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

  private _bytesAt(address: u32, size: u32): Uint8Array {
    return new Uint8Array(this._memory(), address, size);
  }

  /** An async pipeline's word and ticket, read past the sync op's words
   * (RENDER_PIPELINE_ASYNC, COMPUTE_PIPELINE_ASYNC): the pipeline kept at
   * `id` once `made` gives it, where the slot still waits for `id`, and its
   * word written. One the device refuses is made with the sync call, which
   * reports its error as the sync op's does. */
  private _make_async<P extends GPURenderPipeline | GPUComputePipeline>(
    id: u32,
    r: Words,
    made: Promise<P>,
    sync: () => P,
    label: string,
  ): void {
    const [word, ticket] = [r.u(), r.u()];
    const slot = handle_slot(id);
    this._making.set(slot, id);
    const waits = () => this._making.get(slot) === id;
    made
      .catch((e: unknown) => {
        // Named, as the sync call's own error names no pipeline on every device.
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

  /** The request asked with `ticket` answered at the fence `word`
   * (RESULT): `result` into the word after it first, where the request has
   * one, then the ticket, with release; and a turn asked to read it. */
  private _signal(word: u32, ticket: u32, result?: u32): void {
    const w = new Uint32Array(
      this._memory(),
      word,
      result === undefined ? 1 : 2,
    );
    if (result !== undefined) w[1] = result;
    Atomics.store(w, 0, ticket);
    this._wake();
  }

  /** A texture's place read into `p`: one of `places` where WebGPU reads
   * it only during the call, a fresh one where it is kept past it. */
  private _texturePlace(r: Words, p: Place): Place {
    p.texture = this._get<GPUTexture>(r.u());
    p.mipLevel = r.u();
    p.origin[0] = r.u();
    p.origin[1] = r.u();
    p.origin[2] = r.u();
    p.aspect = TEXTURE_ASPECT[r.u()];
    return p;
  }

  private _bufferLayout(r: Words): GPUTexelCopyBufferInfo {
    const b = this._buffer_info;
    b.buffer = this._get<GPUBuffer>(r.u());
    b.offset = r.u();
    b.bytesPerRow = r.u();
    b.rowsPerImage = r.u();
    return b;
  }

  private _extent(r: Words): number[] {
    const e = this._size;
    e[0] = r.u();
    e[1] = r.u();
    e[2] = r.u();
    return e;
  }

  private _timestamps(r: Words): GPURenderPassTimestampWrites | undefined {
    const set = r.opt();
    const t = this._stamps;
    t.beginningOfPassWriteIndex = r.opt();
    t.endOfPassWriteIndex = r.opt();
    if (set === undefined) return undefined;
    t.querySet = this._get<GPUQuerySet>(set);
    return t;
  }

  private _stencilFace(r: Words): GPUStencilFaceState {
    return {
      compare: COMPARE_FUNCTION[r.u()],
      failOp: STENCIL_OPERATION[r.u()],
      depthFailOp: STENCIL_OPERATION[r.u()],
      passOp: STENCIL_OPERATION[r.u()],
    };
  }

  private _record(code: u32, r: Words): void {
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
            // WebGPU's newer formats are past what the lib's type lists.
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
            // As the formats, WebGPU's newer factors are past what a DOM lib's type lists.
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
        // Named by its entries, which a device's error about it names.
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
        // Named by its entry, which a device's error about it names.
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
          this._memory() as ArrayBuffer,
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
        // The bytes where they lie: in the record, a count then them, or at
        // an address and size in the core's memory; the layout says where.
        let data: ArrayBuffer;
        if (code === op.WRITE_TEXTURE) {
          const n = r.u();
          data = r.w.buffer as ArrayBuffer;
          layout.offset = r.w.byteOffset + 4 * r.at;
          r.skip(Math.ceil(n / 4));
        } else {
          data = this._memory() as ArrayBuffer;
          layout.offset = r.u();
          r.skip(1);
        }
        d.queue.writeTexture(place, data, layout, size);
        return;
      }
      case op.COPY_EXTERNAL_IMAGE: {
        const to = this._texturePlace(r, place());
        const [flipY, premultipliedAlpha] = [r.flag(), r.flag()];
        // Copied now: the bytes are the core's again once the stream is
        // handed back (xpute-core ipc/stream.rs R3).
        const image = new Blob([this._bytesAt(r.u(), r.u()).slice()]);
        const [word, ticket] = [r.u(), r.u()];
        // One whose texture went, with the device or by the core, is
        // answered nothing; one the browser cannot decode, REFUSED.
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
          // WebGPU refuses ops on an aspect the format lacks, or one read only.
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
        // Hundreds a frame: the offsets are read where they lie, no array made.
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
        const address = r.u();
        const word = r.u();
        const ticket = r.u();
        // Where the device went since (`lost`) it lands nowhere, as the native
        // runtime's pending reads are dropped; one the device refused, REFUSED.
        buffer.mapAsync(GPUMapMode.READ, offset, size).then(
          () => {
            if (this._get<GPUBuffer>(id) !== buffer) return;
            this._bytesAt(address, size).set(
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
        const context = this._surfaces.get(r.u())?.context;
        const format = r.u();
        const [usage, alphaMode] = [r.u(), CANVAS_ALPHA_MODE[r.u()]];
        context?.configure({
          device: d,
          format:
            format === SURFACE_PREFERRED
              ? navigator.gpu.getPreferredCanvasFormat()
              : TEXTURE_FORMAT[format],
          usage,
          alphaMode,
        });
        return;
      }
      case op.RESIZE: {
        const canvas = this._surfaces.get(r.u())?.context.canvas;
        const [width, height] = [r.u(), r.u()];
        if (canvas && (canvas.width !== width || canvas.height !== height)) {
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
        const [address, words] = [r.u(), r.u()];
        this._block = { address, words };
        const out = new Uint32Array(this._memory(), address, words);
        LIMIT_NAME.forEach(
          (name, k) => (out[k] = Math.min(d.limits[name], 0xffffffff)),
        );
        FEATURE_NAME.forEach((name, k) => {
          if (d.features.has(name as GPUFeatureName)) {
            out[LIMIT_NAME.length + (k >> 5)] |= 1 << (k & 31);
          }
        });
        // WebGPU resolves timestamps in nanoseconds.
        const period = LIMIT_NAME.length + Math.ceil(FEATURE_NAME.length / 32);
        new Float32Array(this._memory(), address, words)[period] = 1;
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
