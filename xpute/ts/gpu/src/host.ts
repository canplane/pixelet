// @xpute/gpu/host.ts

/**
 * The host's end of a guest's GPU calls: runs a stream's records
 * (spec/gpu/api.json) on a WebGPU device, each object kept at its handle's
 * slot, each enumeration's number turned into WebGPU's string by its place.
 * It is told the guest's memory, the device and the surfaces the program
 * binds, and nothing of what the guest draws.
 */

import type { U32Array } from "@xpute/kit/abi/array.ts";
import type { u32 } from "@xpute/kit/abi/word.ts";
import { handle_slot } from "@xpute/host/abi/handle.ts";
import { bytes_of, each_record, text_of } from "@xpute/host/ipc/stream.ts";

import { DONE, NONE, op, STATUS, SURFACE_PREFERRED } from "./api.spec.ts";
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

/** A run's words read in order, as the guest wrote them: one cursor over
 * the stream, set at each record's first word, so a record makes nothing. */
class Words {
  private readonly f: Float32Array;
  at: u32 = 0;
  constructor(readonly w: U32Array) {
    this.f = new Float32Array(w.buffer, w.byteOffset, w.length);
  }
  u(): u32 {
    return this.w[this.at++];
  }
  i(): number {
    return this.u() | 0;
  }
  f32(): number {
    return this.f[this.at++];
  }
  flag(): boolean {
    return this.u() !== 0;
  }
  /** Past `n` words, read in place by the caller. */
  skip(n: u32): void {
    this.at += n;
  }
  /** None where the guest left it unset. */
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

/** Whether `a`'s generation is behind `b`'s, generations wrapping in 16 bits. */
function older(a: u32, b: u32): boolean {
  return (((a >>> 16) - (b >>> 16)) & 0xffff) >= 0x8000;
}

export class GpuHost {
  private readonly objects: (GpuObject | undefined)[] = [];
  /** What a record's call takes, refilled each time rather than made: WebGPU
   * reads a dictionary only during the call it is given to. Two places, as
   * a texture-to-texture copy names two. */
  private readonly places: [Place, Place] = [place(), place()];
  private readonly buffered: GPUTexelCopyBufferInfo = {
    buffer: undefined as unknown as GPUBuffer,
    offset: 0,
    bytesPerRow: 0,
    rowsPerImage: 0,
  };
  private readonly layout: GPUTexelCopyBufferLayout = {
    offset: 0,
    bytesPerRow: 0,
    rowsPerImage: 0,
  };
  private readonly size: number[] = [0, 0, 0];
  /** A pass's: its descriptors, its color attachments (as many as one has
   * had), its depth attachment, its timestamps; a submit's buffers. */
  private readonly pass: GPURenderPassDescriptor = { colorAttachments: [] };
  private readonly colors: GPURenderPassColorAttachment[] = [];
  private readonly depth: GPURenderPassDepthStencilAttachment = {
    view: undefined as unknown as GPUTextureView,
  };
  private readonly compute: GPUComputePassDescriptor = {};
  private readonly stamps: GPURenderPassTimestampWrites = {
    querySet: undefined as unknown as GPUQuerySet,
  };
  private readonly finished: GPUCommandBuffer[] = [];
  /** At the slot the guest names each by, with the guest's handle bound there. */
  private readonly surfaces = new Map<
    u32,
    { handle: u32; context: GPUCanvasContext }
  >();
  /** Each slot's newest handle, kept after it is let go: an older one's bind is refused. */
  private readonly seen = new Map<u32, u32>();
  /** Where the guest asked the device's block written, for its status. */
  private block: { address: u32; words: u32 } | undefined;

  /** `wake` asks the host's loop for a turn: a completion written between
   * turns is read only by a turn. */
  constructor(
    private readonly device: GPUDevice,
    private readonly memory: () => ArrayBufferLike,
    private readonly wake: () => void,
  ) {
    // The guest is told of the loss in its block; nothing here destroys the device.
    device.lost.then(() => {
      this.lost();
      this.wake();
    });
  }

  /** The canvas's `context` for the guest's surface `handle`, named by its
   * slot in CONFIGURE, RESIZE and CURRENT; refused where a newer handle has
   * bound the slot since. */
  bind(handle: u32, context: GPUCanvasContext): boolean {
    const slot = handle_slot(handle);
    const seen = this.seen.get(slot);
    if (seen !== undefined && older(handle, seen)) return false;
    this.seen.set(slot, handle);
    const was = this.surfaces.get(slot);
    if (was && was.context !== context) was.context.unconfigure();
    this.surfaces.set(slot, { handle, context });
    return true;
  }

  /** Lets `handle`'s context go where it is still the one bound under its
   * slot: an older handle unbinds no newer one. */
  unbind(handle: u32): boolean {
    const slot = handle_slot(handle);
    const bound = this.surfaces.get(slot);
    if (handle === 0 || bound?.handle !== handle) return false;
    bound.context.unconfigure();
    this.surfaces.delete(slot);
    return true;
  }

  /** The device went: every object with it, and the guest told in its block. */
  private lost(): void {
    this.objects.length = 0;
    if (this.block) {
      new Uint32Array(
        this.memory(),
        this.block.address,
        this.block.words,
      )[this.block.words - 1] = STATUS.LOST;
    }
  }

  /** Runs the stream at `at`, `head` words of head, as the turn left it. */
  run(at: u32, head: u32): void {
    let r: Words | undefined;
    each_record(this.memory(), at, head, (code, w, a) => {
      r ??= new Words(w);
      r.at = a;
      this.record(code, r);
    });
  }

  private put(id: u32, o: GpuObject): void {
    this.objects[handle_slot(id)] = o;
  }

  private get<T extends GpuObject>(id: u32): T {
    return this.objects[handle_slot(id)] as T;
  }

  private forget(id: u32): void {
    this.objects[handle_slot(id)] = undefined;
  }

  private bytesAt(address: u32, size: u32): Uint8Array {
    return new Uint8Array(this.memory(), address, size);
  }

  /** `ticket | DONE` at `word`, where the ticket still stands, and a turn asked to read it. */
  private done(word: u32, ticket: u32): void {
    const w = new Uint32Array(this.memory(), word, 1);
    if (w[0] !== ticket) return;
    w[0] = (ticket | DONE) >>> 0;
    this.wake();
  }

  /** A texture's place read into `p`: one of `places` where WebGPU reads
   * it only during the call, a fresh one where it is kept past it. */
  private texturePlace(r: Words, p: Place): Place {
    p.texture = this.get<GPUTexture>(r.u());
    p.mipLevel = r.u();
    p.origin[0] = r.u();
    p.origin[1] = r.u();
    p.origin[2] = r.u();
    p.aspect = TEXTURE_ASPECT[r.u()];
    return p;
  }

  private bufferLayout(r: Words): GPUTexelCopyBufferInfo {
    const b = this.buffered;
    b.buffer = this.get<GPUBuffer>(r.u());
    b.offset = r.u();
    b.bytesPerRow = r.u();
    b.rowsPerImage = r.u();
    return b;
  }

  private extent(r: Words): number[] {
    const e = this.size;
    e[0] = r.u();
    e[1] = r.u();
    e[2] = r.u();
    return e;
  }

  private timestamps(r: Words): GPURenderPassTimestampWrites | undefined {
    const set = r.opt();
    const t = this.stamps;
    t.beginningOfPassWriteIndex = r.opt();
    t.endOfPassWriteIndex = r.opt();
    if (set === undefined) return undefined;
    t.querySet = this.get<GPUQuerySet>(set);
    return t;
  }

  private stencilFace(r: Words): GPUStencilFaceState {
    return {
      compare: COMPARE_FUNCTION[r.u()],
      failOp: STENCIL_OPERATION[r.u()],
      depthFailOp: STENCIL_OPERATION[r.u()],
      passOp: STENCIL_OPERATION[r.u()],
    };
  }

  private record(code: u32, r: Words): void {
    const d = this.device;
    switch (code) {
      case op.BUFFER: {
        const id = r.u();
        this.put(
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
        this.put(
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
        const texture = this.get<GPUTexture>(r.u());
        const [f, dim] = [r.opt(), r.opt()];
        const aspect = TEXTURE_ASPECT[r.u()];
        const [baseMipLevel, mipLevelCount, baseArrayLayer, arrayLayerCount] = [
          r.u(),
          r.opt(),
          r.u(),
          r.opt(),
        ];
        const format = f === undefined ? undefined : TEXTURE_FORMAT[f];
        this.put(id, {
          view: texture.createView({
            format,
            dimension: dim === undefined
              ? undefined
              : TEXTURE_VIEW_DIMENSION[dim],
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
        this.put(
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
            compare: compare === undefined
              ? undefined
              : COMPARE_FUNCTION[compare],
            maxAnisotropy: r.u(),
          }),
        );
        return;
      }
      case op.SHADER_MODULE: {
        const id = r.u();
        this.put(id, d.createShaderModule({ code: r.text() }));
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
        this.put(id, d.createBindGroupLayout({ entries }));
        return;
      }
      case op.PIPELINE_LAYOUT: {
        const id = r.u();
        this.put(
          id,
          d.createPipelineLayout({
            bindGroupLayouts: Array.from(
              { length: r.u() },
              () => this.get<GPUBindGroupLayout>(r.u()),
            ),
          }),
        );
        return;
      }
      case op.BIND_GROUP: {
        const id = r.u();
        const layout = this.get<GPUBindGroupLayout>(r.u());
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
                buffer: this.get<GPUBuffer>(object),
                offset,
                size: size === NONE ? undefined : size,
              },
            };
          }
          return {
            binding,
            resource: kind === 1
              ? this.get<GPUSampler>(object)
              : this.get<View>(object).view,
          };
        });
        this.put(id, d.createBindGroup({ layout, entries }));
        return;
      }
      case op.RENDER_PIPELINE: {
        const id = r.u();
        const layout = r.opt();
        const vertex: GPUVertexState = {
          module: this.get<GPUShaderModule>(r.u()),
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
          stencilFront: this.stencilFace(r),
          stencilBack: this.stencilFace(r),
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
            const blend = cop === NONE ? undefined : {
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
        this.put(
          id,
          d.createRenderPipeline({
            layout: layout === undefined
              ? "auto"
              : this.get<GPUPipelineLayout>(layout),
            vertex,
            primitive,
            depthStencil: depthFormat === undefined ? undefined : depth,
            multisample,
            fragment: module === undefined ? undefined : {
              module: this.get<GPUShaderModule>(module),
              entryPoint,
              targets,
            },
          }),
        );
        return;
      }
      case op.COMPUTE_PIPELINE: {
        const id = r.u();
        const layout = r.opt();
        const module = this.get<GPUShaderModule>(r.u());
        this.put(
          id,
          d.createComputePipeline({
            layout: layout === undefined
              ? "auto"
              : this.get<GPUPipelineLayout>(layout),
            compute: { module, entryPoint: r.text() },
          }),
        );
        return;
      }
      case op.QUERY_SET: {
        const id = r.u();
        this.put(
          id,
          d.createQuerySet({ type: QUERY_TYPE[r.u()], count: r.u() }),
        );
        return;
      }
      case op.DESTROY: {
        const id = r.u();
        const o = this.get<GpuObject>(id);
        if (
          o instanceof GPUBuffer || o instanceof GPUTexture ||
          o instanceof GPUQuerySet
        ) o.destroy();
        this.forget(id);
        return;
      }
      case op.WRITE_BUFFER: {
        const buffer = this.get<GPUBuffer>(r.u());
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
        const buffer = this.get<GPUBuffer>(r.u());
        const offset = r.u();
        d.queue.writeBuffer(
          buffer,
          offset,
          this.memory() as ArrayBuffer,
          r.u(),
          r.u(),
        );
        return;
      }
      case op.WRITE_TEXTURE:
      case op.WRITE_TEXTURE_AT: {
        const place = this.texturePlace(r, this.places[0]);
        const layout = this.layout;
        layout.bytesPerRow = r.u();
        layout.rowsPerImage = r.u();
        const size = this.extent(r);
        // The bytes where they lie: in the record, a count then them, or at
        // an address and size in the guest's memory; the layout says where.
        let data: ArrayBuffer;
        if (code === op.WRITE_TEXTURE) {
          const n = r.u();
          data = r.w.buffer as ArrayBuffer;
          layout.offset = r.w.byteOffset + 4 * r.at;
          r.skip(Math.ceil(n / 4));
        } else {
          data = this.memory() as ArrayBuffer;
          layout.offset = r.u();
          r.skip(1);
        }
        d.queue.writeTexture(place, data, layout, size);
        return;
      }
      case op.COPY_EXTERNAL_IMAGE: {
        const to = this.texturePlace(r, place());
        const [flipY, premultipliedAlpha] = [r.flag(), r.flag()];
        // Copied now: the guest's bytes are only its until the next turn.
        const image = new Blob([this.bytesAt(r.u(), r.u()).slice()]);
        const [word, ticket] = [r.u(), r.u()];
        // An image the browser cannot decode is left out, its word not written.
        createImageBitmap(image, {
          imageOrientation: flipY ? "flipY" : "from-image",
          premultiplyAlpha: premultipliedAlpha ? "premultiply" : "none",
        }).then((source) => {
          if (this.objects.includes(to.texture)) {
            (d.queue as ExternalImageQueue).copyExternalImageToTexture(
              { source },
              to,
              [source.width, source.height],
            );
            this.done(word, ticket);
          }
          source.close();
        }, () => {});
        return;
      }
      case op.ENCODER:
        this.put(r.u(), d.createCommandEncoder());
        return;
      case op.COPY_BUFFER_TO_BUFFER: {
        const e = this.get<GPUCommandEncoder>(r.u());
        e.copyBufferToBuffer(
          this.get<GPUBuffer>(r.u()),
          r.u(),
          this.get<GPUBuffer>(r.u()),
          r.u(),
          r.u(),
        );
        return;
      }
      case op.COPY_BUFFER_TO_TEXTURE: {
        const e = this.get<GPUCommandEncoder>(r.u());
        e.copyBufferToTexture(
          this.bufferLayout(r),
          this.texturePlace(r, this.places[0]),
          this.extent(r),
        );
        return;
      }
      case op.COPY_TEXTURE_TO_BUFFER: {
        const e = this.get<GPUCommandEncoder>(r.u());
        e.copyTextureToBuffer(
          this.texturePlace(r, this.places[0]),
          this.bufferLayout(r),
          this.extent(r),
        );
        return;
      }
      case op.COPY_TEXTURE_TO_TEXTURE: {
        const e = this.get<GPUCommandEncoder>(r.u());
        e.copyTextureToTexture(
          this.texturePlace(r, this.places[0]),
          this.texturePlace(r, this.places[1]),
          this.extent(r),
        );
        return;
      }
      case op.CLEAR_BUFFER: {
        const e = this.get<GPUCommandEncoder>(r.u());
        const [buffer, offset, size] = [
          this.get<GPUBuffer>(r.u()),
          r.u(),
          r.opt(),
        ];
        e.clearBuffer(buffer, offset, size);
        return;
      }
      case op.RESOLVE_QUERY_SET: {
        const e = this.get<GPUCommandEncoder>(r.u());
        e.resolveQuerySet(
          this.get<GPUQuerySet>(r.u()),
          r.u(),
          r.u(),
          this.get<GPUBuffer>(r.u()),
          r.u(),
        );
        return;
      }
      case op.RENDER_PASS: {
        const id = r.u();
        const e = this.get<GPUCommandEncoder>(r.u());
        const n = r.u();
        const colors = this.pass
          .colorAttachments as GPURenderPassColorAttachment[];
        colors.length = 0;
        for (let k = 0; k < n; k++) {
          const c = this.colors[k] ??= {
            view: undefined as unknown as GPUTextureView,
            loadOp: "load",
            storeOp: "store",
            clearValue: [0, 0, 0, 0],
          };
          c.view = this.get<View>(r.u()).view;
          const resolve = r.opt();
          c.resolveTarget = resolve === undefined
            ? undefined
            : this.get<View>(resolve).view;
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
        this.pass.depthStencilAttachment = undefined;
        if (depth !== undefined) {
          // WebGPU refuses ops on an aspect the format lacks, or one read only.
          const { view, format } = this.get<View>(depth);
          const a = this.depth;
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
          this.pass.depthStencilAttachment = a;
        }
        this.pass.timestampWrites = this.timestamps(r);
        const occlusion = r.opt();
        this.pass.occlusionQuerySet = occlusion === undefined
          ? undefined
          : this.get<GPUQuerySet>(occlusion);
        this.put(id, e.beginRenderPass(this.pass));
        return;
      }
      case op.COMPUTE_PASS: {
        const id = r.u();
        const e = this.get<GPUCommandEncoder>(r.u());
        this.compute.timestampWrites = this.timestamps(r);
        this.put(id, e.beginComputePass(this.compute));
        return;
      }
      case op.SET_PIPELINE: {
        const pass = this.get<GPURenderPassEncoder | GPUComputePassEncoder>(
          r.u(),
        );
        const pipeline = this.get<GPURenderPipeline | GPUComputePipeline>(
          r.u(),
        );
        if (pass instanceof GPUComputePassEncoder) {
          pass.setPipeline(pipeline as GPUComputePipeline);
        } else pass.setPipeline(pipeline as GPURenderPipeline);
        return;
      }
      case op.SET_BIND_GROUP: {
        const pass = this.get<GPURenderPassEncoder | GPUComputePassEncoder>(
          r.u(),
        );
        const index = r.u();
        const group = this.get<GPUBindGroup>(r.u());
        const offsets = r.u();
        // Hundreds a frame: the offsets are read where they lie, no array made.
        pass.setBindGroup(index, group, r.w, r.at, offsets);
        r.skip(offsets);
        return;
      }
      case op.SET_VERTEX_BUFFER: {
        const pass = this.get<GPURenderPassEncoder>(r.u());
        pass.setVertexBuffer(r.u(), this.get<GPUBuffer>(r.u()), r.u(), r.opt());
        return;
      }
      case op.SET_INDEX_BUFFER: {
        const pass = this.get<GPURenderPassEncoder>(r.u());
        pass.setIndexBuffer(
          this.get<GPUBuffer>(r.u()),
          INDEX_FORMAT[r.u()],
          r.u(),
          r.opt(),
        );
        return;
      }
      case op.SET_VIEWPORT:
        this.get<GPURenderPassEncoder>(r.u()).setViewport(
          r.f32(),
          r.f32(),
          r.f32(),
          r.f32(),
          r.f32(),
          r.f32(),
        );
        return;
      case op.SET_SCISSOR:
        this.get<GPURenderPassEncoder>(r.u()).setScissorRect(
          r.u(),
          r.u(),
          r.u(),
          r.u(),
        );
        return;
      case op.SET_BLEND_CONSTANT:
        this.get<GPURenderPassEncoder>(r.u()).setBlendConstant([
          r.f32(),
          r.f32(),
          r.f32(),
          r.f32(),
        ]);
        return;
      case op.SET_STENCIL_REFERENCE:
        this.get<GPURenderPassEncoder>(r.u()).setStencilReference(r.u());
        return;
      case op.DRAW:
        this.get<GPURenderPassEncoder>(r.u()).draw(r.u(), r.u(), r.u(), r.u());
        return;
      case op.DRAW_INDEXED:
        this.get<GPURenderPassEncoder>(r.u()).drawIndexed(
          r.u(),
          r.u(),
          r.u(),
          r.i(),
          r.u(),
        );
        return;
      case op.DRAW_INDIRECT:
        this.get<GPURenderPassEncoder>(r.u()).drawIndirect(
          this.get<GPUBuffer>(r.u()),
          r.u(),
        );
        return;
      case op.DRAW_INDEXED_INDIRECT:
        this.get<GPURenderPassEncoder>(r.u()).drawIndexedIndirect(
          this.get<GPUBuffer>(r.u()),
          r.u(),
        );
        return;
      case op.DISPATCH:
        this.get<GPUComputePassEncoder>(r.u()).dispatchWorkgroups(
          r.u(),
          r.u(),
          r.u(),
        );
        return;
      case op.DISPATCH_INDIRECT:
        this.get<GPUComputePassEncoder>(r.u()).dispatchWorkgroupsIndirect(
          this.get<GPUBuffer>(r.u()),
          r.u(),
        );
        return;
      case op.END: {
        const id = r.u();
        this.get<GPURenderPassEncoder | GPUComputePassEncoder>(id).end();
        this.forget(id);
        return;
      }
      case op.SUBMIT: {
        const n = r.u();
        const finished = this.finished;
        for (let k = 0; k < n; k++) {
          const id = r.u();
          finished.push(this.get<GPUCommandEncoder>(id).finish());
          this.forget(id);
        }
        d.queue.submit(finished);
        finished.length = 0;
        return;
      }
      case op.MAP_ASYNC: {
        const id = r.u();
        const buffer = this.get<GPUBuffer>(id);
        const offset = r.u();
        const size = r.u();
        const address = r.u();
        const word = r.u();
        const ticket = r.u();
        // Where the device went since (`lost`) it lands nowhere, as the native
        // host's pending reads are dropped.
        buffer.mapAsync(GPUMapMode.READ, offset, size).then(() => {
          if (this.get<GPUBuffer>(id) !== buffer) return;
          this.bytesAt(address, size).set(
            new Uint8Array(buffer.getMappedRange(offset, size)),
          );
          buffer.unmap();
          new Uint32Array(this.memory(), word, 1)[0] = (ticket | DONE) >>> 0;
          this.wake();
        }, () => {});
        return;
      }
      case op.QUEUE_DONE: {
        const [word, ticket] = [r.u(), r.u()];
        d.queue.onSubmittedWorkDone().then(() => this.done(word, ticket));
        return;
      }
      case op.CONFIGURE: {
        const context = this.surfaces.get(r.u())?.context;
        const format = r.u();
        const [usage, alphaMode] = [r.u(), CANVAS_ALPHA_MODE[r.u()]];
        context?.configure({
          device: d,
          format: format === SURFACE_PREFERRED
            ? navigator.gpu.getPreferredCanvasFormat()
            : TEXTURE_FORMAT[format],
          usage,
          alphaMode,
        });
        return;
      }
      case op.RESIZE: {
        const canvas = this.surfaces.get(r.u())?.context.canvas;
        const [width, height] = [r.u(), r.u()];
        if (canvas && (canvas.width !== width || canvas.height !== height)) {
          [canvas.width, canvas.height] = [width, height];
        }
        return;
      }
      case op.CURRENT: {
        const id = r.u();
        const texture = this.surfaces.get(r.u())?.context.getCurrentTexture();
        if (!texture) return;
        this.put(id, { view: texture.createView(), format: texture.format });
        return;
      }
      case op.DEVICE: {
        const [address, words] = [r.u(), r.u()];
        this.block = { address, words };
        const out = new Uint32Array(this.memory(), address, words);
        const limits = d.limits as unknown as Record<string, number>;
        LIMIT_NAME.forEach((
          name,
          k,
        ) => (out[k] = Math.min(limits[name] ?? 0, 0xffffffff)));
        FEATURE_NAME.forEach((name, k) => {
          if (d.features.has(name as GPUFeatureName)) {
            out[LIMIT_NAME.length + (k >> 5)] |= 1 << (k & 31);
          }
        });
        // WebGPU resolves timestamps in nanoseconds.
        const period = LIMIT_NAME.length + Math.ceil(FEATURE_NAME.length / 32);
        new Float32Array(this.memory(), address, words)[period] = 1;
        out[period + 1] = (TEXTURE_FORMAT as readonly string[]).indexOf(
          navigator.gpu.getPreferredCanvasFormat(),
        );
        out[words - 1] = STATUS.READY;
        return;
      }
      default:
        throw new Error(`[gpu] op ${code} is no op of spec/gpu/api.json`);
    }
  }
}
