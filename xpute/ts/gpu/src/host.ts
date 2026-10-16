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
type View = { readonly view: GPUTextureView; readonly format: GPUTextureFormat };

/** `copyExternalImageToTexture`, which the lib's `GPUQueue` does not list. */
type ExternalImageQueue = GPUQueue & {
  copyExternalImageToTexture(source: { source: ImageBitmap }, destination: GPUTexelCopyTextureInfo, size: GPUExtent3D): void;
};

/** A record's words read in order, as the guest wrote them. */
class Words {
  private readonly f: Float32Array;
  constructor(private readonly w: U32Array, private at: u32) {
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

export class GpuHost {
  private readonly objects: (GpuObject | undefined)[] = [];
  private readonly surfaces = new Map<u32, GPUCanvasContext>();
  /** Where the guest asked the device's block written, for its status. */
  private block: { address: u32; words: u32 } | undefined;

  constructor(private readonly device: GPUDevice, private readonly memory: () => ArrayBufferLike) {}

  /** A surface the guest names by `surface` in CONFIGURE, RESIZE and CURRENT. */
  bindSurface(surface: u32, context: GPUCanvasContext): void {
    this.surfaces.set(surface, context);
  }

  unbindSurface(surface: u32): void {
    this.surfaces.get(surface)?.unconfigure();
    this.surfaces.delete(surface);
  }

  /** The device went: every object with it, and the guest told in its block. */
  lost(): void {
    this.objects.length = 0;
    if (this.block) new Uint32Array(this.memory(), this.block.address, this.block.words)[this.block.words - 1] = STATUS.LOST;
  }

  /** Runs the stream at `at`, `head` words of head, as the turn left it. */
  run(at: u32, head: u32): void {
    each_record(this.memory(), at, head, (code, w, a) => this.record(code, new Words(w, a)));
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

  /** A view's bytes as WebGPU takes them: its buffer, where it starts, how long. */
  private static span(v: Uint8Array): [ArrayBuffer, number, number] {
    return [v.buffer as ArrayBuffer, v.byteOffset, v.byteLength];
  }

  /** `ticket | DONE` at `word`, where the ticket still stands. */
  private done(word: u32, ticket: u32): void {
    const w = new Uint32Array(this.memory(), word, 1);
    if (w[0] === ticket) w[0] = (ticket | DONE) >>> 0;
  }

  private texturePlace(r: Words): GPUTexelCopyTextureInfo {
    return { texture: this.get<GPUTexture>(r.u()), mipLevel: r.u(), origin: [r.u(), r.u(), r.u()], aspect: TEXTURE_ASPECT[r.u()] };
  }

  private bufferLayout(r: Words): GPUTexelCopyBufferInfo {
    return { buffer: this.get<GPUBuffer>(r.u()), offset: r.u(), bytesPerRow: r.u(), rowsPerImage: r.u() };
  }

  private timestamps(r: Words): GPURenderPassTimestampWrites | undefined {
    const [set, begin, end] = [r.opt(), r.opt(), r.opt()];
    return set === undefined ? undefined : { querySet: this.get<GPUQuerySet>(set), beginningOfPassWriteIndex: begin, endOfPassWriteIndex: end };
  }

  private stencilFace(r: Words): GPUStencilFaceState {
    return { compare: COMPARE_FUNCTION[r.u()], failOp: STENCIL_OPERATION[r.u()], depthFailOp: STENCIL_OPERATION[r.u()], passOp: STENCIL_OPERATION[r.u()] };
  }

  private record(code: u32, r: Words): void {
    const d = this.device;
    switch (code) {
      case op.BUFFER: {
        const id = r.u();
        this.put(id, d.createBuffer({ size: r.u(), usage: r.u(), mappedAtCreation: r.flag() }));
        return;
      }
      case op.TEXTURE: {
        const id = r.u();
        const size = [r.u(), r.u(), r.u()];
        const [mipLevelCount, sampleCount, dimension, format, usage] = [r.u(), r.u(), TEXTURE_DIMENSION[r.u()], TEXTURE_FORMAT[r.u()], r.u()];
        const viewFormats = Array.from({ length: r.u() }, () => TEXTURE_FORMAT[r.u()]);
        this.put(id, d.createTexture({ size, mipLevelCount, sampleCount, dimension, format, usage, viewFormats }));
        return;
      }
      case op.VIEW: {
        const id = r.u();
        const texture = this.get<GPUTexture>(r.u());
        const [f, dim] = [r.opt(), r.opt()];
        const aspect = TEXTURE_ASPECT[r.u()];
        const [baseMipLevel, mipLevelCount, baseArrayLayer, arrayLayerCount] = [r.u(), r.opt(), r.u(), r.opt()];
        const format = f === undefined ? undefined : TEXTURE_FORMAT[f];
        this.put(id, {
          view: texture.createView({ format, dimension: dim === undefined ? undefined : TEXTURE_VIEW_DIMENSION[dim], aspect, baseMipLevel, mipLevelCount, baseArrayLayer, arrayLayerCount }),
          format: format ?? texture.format,
        });
        return;
      }
      case op.SAMPLER: {
        const id = r.u();
        const [u, v, w, mag, min, mip, lodMinClamp, lodMaxClamp] = [r.u(), r.u(), r.u(), r.u(), r.u(), r.u(), r.f32(), r.f32()];
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
            compare: compare === undefined ? undefined : COMPARE_FUNCTION[compare],
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
        const entries = Array.from({ length: r.u() }, (): GPUBindGroupLayoutEntry => {
          const [binding, visibility, kind, a, b, c] = [r.u(), r.u(), r.u(), r.u(), r.u(), r.u()];
          switch (kind) {
            case 0:
              return { binding, visibility, buffer: { type: BUFFER_BINDING_TYPE[a], hasDynamicOffset: b !== 0, minBindingSize: c } };
            case 1:
              return { binding, visibility, sampler: { type: SAMPLER_BINDING_TYPE[a] } };
            case 2:
              return { binding, visibility, texture: { sampleType: TEXTURE_SAMPLE_TYPE[a], viewDimension: TEXTURE_VIEW_DIMENSION[b], multisampled: c !== 0 } };
            default:
              return { binding, visibility, storageTexture: { access: STORAGE_TEXTURE_ACCESS[a], format: TEXTURE_FORMAT[b], viewDimension: TEXTURE_VIEW_DIMENSION[c] } };
          }
        });
        this.put(id, d.createBindGroupLayout({ entries }));
        return;
      }
      case op.PIPELINE_LAYOUT: {
        const id = r.u();
        this.put(id, d.createPipelineLayout({ bindGroupLayouts: Array.from({ length: r.u() }, () => this.get<GPUBindGroupLayout>(r.u())) }));
        return;
      }
      case op.BIND_GROUP: {
        const id = r.u();
        const layout = this.get<GPUBindGroupLayout>(r.u());
        const entries = Array.from({ length: r.u() }, (): GPUBindGroupEntry => {
          const [binding, kind, object, offset, size] = [r.u(), r.u(), r.u(), r.u(), r.u()];
          if (kind === 0) return { binding, resource: { buffer: this.get<GPUBuffer>(object), offset, size: size === NONE ? undefined : size } };
          return { binding, resource: kind === 1 ? this.get<GPUSampler>(object) : this.get<View>(object).view };
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
            attributes: Array.from({ length: r.u() }, (): GPUVertexAttribute => ({ format: VERTEX_FORMAT[r.u()] as GPUVertexFormat, offset: r.u(), shaderLocation: r.u() })),
          })),
        };
        const strip = (topology: u32, s: u32 | undefined) => ({ topology: PRIMITIVE_TOPOLOGY[topology], stripIndexFormat: s === undefined ? undefined : INDEX_FORMAT[s] });
        const primitive: GPUPrimitiveState = { ...strip(r.u(), r.opt()), frontFace: FRONT_FACE[r.u()], cullMode: CULL_MODE[r.u()], unclippedDepth: r.flag() };
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
        const multisample: GPUMultisampleState = { count: r.u(), mask: r.u(), alphaToCoverageEnabled: r.flag() };
        const module = r.opt();
        const entryPoint = r.text();
        const targets = Array.from({ length: r.u() }, (): GPUColorTargetState => {
          const format = TEXTURE_FORMAT[r.u()];
          const [cop, csrc, cdst, aop, asrc, adst] = [r.u(), r.u(), r.u(), r.u(), r.u(), r.u()];
          const blend = cop === NONE ? undefined : {
            color: { operation: BLEND_OPERATION[cop], srcFactor: BLEND_FACTOR[csrc], dstFactor: BLEND_FACTOR[cdst] },
            alpha: { operation: BLEND_OPERATION[aop], srcFactor: BLEND_FACTOR[asrc], dstFactor: BLEND_FACTOR[adst] },
          };
          return { format, blend, writeMask: r.u() };
        });
        this.put(
          id,
          d.createRenderPipeline({
            layout: layout === undefined ? "auto" : this.get<GPUPipelineLayout>(layout),
            vertex,
            primitive,
            depthStencil: depthFormat === undefined ? undefined : depth,
            multisample,
            fragment: module === undefined ? undefined : { module: this.get<GPUShaderModule>(module), entryPoint, targets },
          }),
        );
        return;
      }
      case op.COMPUTE_PIPELINE: {
        const id = r.u();
        const layout = r.opt();
        const module = this.get<GPUShaderModule>(r.u());
        this.put(id, d.createComputePipeline({ layout: layout === undefined ? "auto" : this.get<GPUPipelineLayout>(layout), compute: { module, entryPoint: r.text() } }));
        return;
      }
      case op.QUERY_SET: {
        const id = r.u();
        this.put(id, d.createQuerySet({ type: QUERY_TYPE[r.u()], count: r.u() }));
        return;
      }
      case op.DESTROY: {
        const id = r.u();
        const o = this.get<GpuObject>(id);
        if (o instanceof GPUBuffer || o instanceof GPUTexture || o instanceof GPUQuerySet) o.destroy();
        this.forget(id);
        return;
      }
      case op.WRITE_BUFFER: {
        const buffer = this.get<GPUBuffer>(r.u());
        const offset = r.u();
        d.queue.writeBuffer(buffer, offset, ...GpuHost.span(r.bytes()));
        return;
      }
      case op.WRITE_BUFFER_AT: {
        const buffer = this.get<GPUBuffer>(r.u());
        const offset = r.u();
        d.queue.writeBuffer(buffer, offset, ...GpuHost.span(this.bytesAt(r.u(), r.u())));
        return;
      }
      case op.WRITE_TEXTURE:
      case op.WRITE_TEXTURE_AT: {
        const place = this.texturePlace(r);
        const layout = { bytesPerRow: r.u(), rowsPerImage: r.u() };
        const size = [r.u(), r.u(), r.u()];
        const [data, at, length] = GpuHost.span(code === op.WRITE_TEXTURE ? r.bytes() : this.bytesAt(r.u(), r.u()));
        d.queue.writeTexture(place, new Uint8Array(data, at, length), layout, size);
        return;
      }
      case op.COPY_EXTERNAL_IMAGE: {
        const texture = this.get<GPUTexture>(r.u());
        const [mipLevel, origin, flipY, premultipliedAlpha] = [r.u(), [r.u(), r.u(), r.u()], r.flag(), r.flag()];
        // Copied now: the guest's bytes are only its until the next turn.
        const image = new Blob([this.bytesAt(r.u(), r.u()).slice()]);
        createImageBitmap(image, { imageOrientation: flipY ? "flipY" : "from-image", premultiplyAlpha: premultipliedAlpha ? "premultiply" : "none" }).then((source) => {
          if (this.objects.includes(texture)) (d.queue as ExternalImageQueue).copyExternalImageToTexture({ source }, { texture, mipLevel, origin }, [source.width, source.height]);
          source.close();
        });
        return;
      }
      case op.ENCODER:
        this.put(r.u(), d.createCommandEncoder());
        return;
      case op.COPY_BUFFER_TO_BUFFER: {
        const e = this.get<GPUCommandEncoder>(r.u());
        e.copyBufferToBuffer(this.get<GPUBuffer>(r.u()), r.u(), this.get<GPUBuffer>(r.u()), r.u(), r.u());
        return;
      }
      case op.COPY_BUFFER_TO_TEXTURE: {
        const e = this.get<GPUCommandEncoder>(r.u());
        e.copyBufferToTexture(this.bufferLayout(r), this.texturePlace(r), [r.u(), r.u(), r.u()]);
        return;
      }
      case op.COPY_TEXTURE_TO_BUFFER: {
        const e = this.get<GPUCommandEncoder>(r.u());
        e.copyTextureToBuffer(this.texturePlace(r), this.bufferLayout(r), [r.u(), r.u(), r.u()]);
        return;
      }
      case op.COPY_TEXTURE_TO_TEXTURE: {
        const e = this.get<GPUCommandEncoder>(r.u());
        e.copyTextureToTexture(this.texturePlace(r), this.texturePlace(r), [r.u(), r.u(), r.u()]);
        return;
      }
      case op.CLEAR_BUFFER: {
        const e = this.get<GPUCommandEncoder>(r.u());
        const [buffer, offset, size] = [this.get<GPUBuffer>(r.u()), r.u(), r.opt()];
        e.clearBuffer(buffer, offset, size);
        return;
      }
      case op.RESOLVE_QUERY_SET: {
        const e = this.get<GPUCommandEncoder>(r.u());
        e.resolveQuerySet(this.get<GPUQuerySet>(r.u()), r.u(), r.u(), this.get<GPUBuffer>(r.u()), r.u());
        return;
      }
      case op.RENDER_PASS: {
        const id = r.u();
        const e = this.get<GPUCommandEncoder>(r.u());
        const colorAttachments = Array.from({ length: r.u() }, (): GPURenderPassColorAttachment => {
          const [view, resolve, load, store] = [this.get<View>(r.u()).view, r.opt(), LOAD_OP[r.u()], STORE_OP[r.u()]];
          return { view, resolveTarget: resolve === undefined ? undefined : this.get<View>(resolve).view, loadOp: load, storeOp: store, clearValue: [r.f32(), r.f32(), r.f32(), r.f32()] };
        });
        const depth = r.opt();
        const [dload, dstore, dclear, dro, sload, sstore, sclear, sro] = [r.u(), r.u(), r.f32(), r.flag(), r.u(), r.u(), r.u(), r.flag()];
        let depthStencilAttachment: GPURenderPassDepthStencilAttachment | undefined;
        if (depth !== undefined) {
          // WebGPU refuses ops on an aspect the format lacks, or one read only.
          const { view, format } = this.get<View>(depth);
          const [hasDepth, hasStencil] = [format.startsWith("depth"), format.includes("stencil")];
          depthStencilAttachment = {
            view,
            ...(hasDepth && !dro ? { depthLoadOp: LOAD_OP[dload], depthStoreOp: STORE_OP[dstore], depthClearValue: dclear } : { depthReadOnly: dro }),
            ...(hasStencil && !sro ? { stencilLoadOp: LOAD_OP[sload], stencilStoreOp: STORE_OP[sstore], stencilClearValue: sclear } : { stencilReadOnly: sro }),
          };
        }
        const timestampWrites = this.timestamps(r);
        const occlusion = r.opt();
        this.put(
          id,
          e.beginRenderPass({ colorAttachments, depthStencilAttachment, timestampWrites, occlusionQuerySet: occlusion === undefined ? undefined : this.get<GPUQuerySet>(occlusion) }),
        );
        return;
      }
      case op.COMPUTE_PASS: {
        const id = r.u();
        const e = this.get<GPUCommandEncoder>(r.u());
        this.put(id, e.beginComputePass({ timestampWrites: this.timestamps(r) }));
        return;
      }
      case op.SET_PIPELINE: {
        const pass = this.get<GPURenderPassEncoder | GPUComputePassEncoder>(r.u());
        const pipeline = this.get<GPURenderPipeline | GPUComputePipeline>(r.u());
        if (pass instanceof GPUComputePassEncoder) pass.setPipeline(pipeline as GPUComputePipeline);
        else pass.setPipeline(pipeline as GPURenderPipeline);
        return;
      }
      case op.SET_BIND_GROUP: {
        const pass = this.get<GPURenderPassEncoder | GPUComputePassEncoder>(r.u());
        const [index, group] = [r.u(), this.get<GPUBindGroup>(r.u())];
        pass.setBindGroup(index, group, Array.from({ length: r.u() }, () => r.u()));
        return;
      }
      case op.SET_VERTEX_BUFFER: {
        const pass = this.get<GPURenderPassEncoder>(r.u());
        pass.setVertexBuffer(r.u(), this.get<GPUBuffer>(r.u()), r.u(), r.opt());
        return;
      }
      case op.SET_INDEX_BUFFER: {
        const pass = this.get<GPURenderPassEncoder>(r.u());
        pass.setIndexBuffer(this.get<GPUBuffer>(r.u()), INDEX_FORMAT[r.u()], r.u(), r.opt());
        return;
      }
      case op.SET_VIEWPORT:
        this.get<GPURenderPassEncoder>(r.u()).setViewport(r.f32(), r.f32(), r.f32(), r.f32(), r.f32(), r.f32());
        return;
      case op.SET_SCISSOR:
        this.get<GPURenderPassEncoder>(r.u()).setScissorRect(r.u(), r.u(), r.u(), r.u());
        return;
      case op.SET_BLEND_CONSTANT:
        this.get<GPURenderPassEncoder>(r.u()).setBlendConstant([r.f32(), r.f32(), r.f32(), r.f32()]);
        return;
      case op.SET_STENCIL_REFERENCE:
        this.get<GPURenderPassEncoder>(r.u()).setStencilReference(r.u());
        return;
      case op.DRAW:
        this.get<GPURenderPassEncoder>(r.u()).draw(r.u(), r.u(), r.u(), r.u());
        return;
      case op.DRAW_INDEXED:
        this.get<GPURenderPassEncoder>(r.u()).drawIndexed(r.u(), r.u(), r.u(), r.i(), r.u());
        return;
      case op.DRAW_INDIRECT:
        this.get<GPURenderPassEncoder>(r.u()).drawIndirect(this.get<GPUBuffer>(r.u()), r.u());
        return;
      case op.DRAW_INDEXED_INDIRECT:
        this.get<GPURenderPassEncoder>(r.u()).drawIndexedIndirect(this.get<GPUBuffer>(r.u()), r.u());
        return;
      case op.DISPATCH:
        this.get<GPUComputePassEncoder>(r.u()).dispatchWorkgroups(r.u(), r.u(), r.u());
        return;
      case op.DISPATCH_INDIRECT:
        this.get<GPUComputePassEncoder>(r.u()).dispatchWorkgroupsIndirect(this.get<GPUBuffer>(r.u()), r.u());
        return;
      case op.END: {
        const id = r.u();
        this.get<GPURenderPassEncoder | GPUComputePassEncoder>(id).end();
        this.forget(id);
        return;
      }
      case op.SUBMIT: {
        const ids = Array.from({ length: r.u() }, () => r.u());
        d.queue.submit(ids.map((id) => this.get<GPUCommandEncoder>(id).finish()));
        ids.forEach((id) => this.forget(id));
        return;
      }
      case op.READ_BUFFER: {
        const [buffer, offset, size, address, word, ticket] = [this.get<GPUBuffer>(r.u()), r.u(), r.u(), r.u(), r.u(), r.u()];
        const staging = d.createBuffer({ size, usage: GPUBufferUsage.MAP_READ | GPUBufferUsage.COPY_DST });
        const e = d.createCommandEncoder();
        e.copyBufferToBuffer(buffer, offset, staging, 0, size);
        d.queue.submit([e.finish()]);
        staging.mapAsync(GPUMapMode.READ).then(() => {
          if (new Uint32Array(this.memory(), word, 1)[0] === ticket) {
            this.bytesAt(address, size).set(new Uint8Array(staging.getMappedRange()));
            this.done(word, ticket);
          }
          staging.destroy();
        }, () => staging.destroy());
        return;
      }
      case op.QUEUE_DONE: {
        const [word, ticket] = [r.u(), r.u()];
        d.queue.onSubmittedWorkDone().then(() => this.done(word, ticket));
        return;
      }
      case op.CONFIGURE: {
        const context = this.surfaces.get(r.u());
        const format = r.u();
        const [usage, alphaMode] = [r.u(), CANVAS_ALPHA_MODE[r.u()]];
        context?.configure({ device: d, format: format === SURFACE_PREFERRED ? navigator.gpu.getPreferredCanvasFormat() : TEXTURE_FORMAT[format], usage, alphaMode });
        return;
      }
      case op.RESIZE: {
        const canvas = this.surfaces.get(r.u())?.canvas;
        const [width, height] = [r.u(), r.u()];
        if (canvas && (canvas.width !== width || canvas.height !== height)) [canvas.width, canvas.height] = [width, height];
        return;
      }
      case op.CURRENT: {
        const id = r.u();
        const texture = this.surfaces.get(r.u())?.getCurrentTexture();
        if (!texture) return;
        this.put(id, { view: texture.createView(), format: texture.format });
        return;
      }
      case op.DEVICE: {
        const [address, words] = [r.u(), r.u()];
        this.block = { address, words };
        const out = new Uint32Array(this.memory(), address, words);
        const limits = d.limits as unknown as Record<string, number>;
        LIMIT_NAME.forEach((name, k) => (out[k] = Math.min(limits[name] ?? 0, 0xffffffff)));
        FEATURE_NAME.forEach((name, k) => {
          if (d.features.has(name as GPUFeatureName)) out[LIMIT_NAME.length + (k >> 5)] |= 1 << (k & 31);
        });
        out[words - 1] = STATUS.READY;
        return;
      }
      default:
        throw new Error(`[gpu] op ${code} is no op of spec/gpu/api.json`);
    }
  }
}
