// @pxl/web/render/gpu/pipeline.ts

/**
 * The pipelines as the module defines them (crates/pxl-core/src/render/shader.rs):
 * each read out of the module as a record of words — its shaders, its
 * vertex lanes, its bind group layouts, its primitive, depth and blend
 * state — and turned into WebGPU's descriptor here, and nowhere else. A
 * pass owns no descriptor literal; what it decides at the call site is
 * the colour format and the sample count, which are the renderer's.
 *
 * Bind group layouts are shared by their words, so every pipeline that
 * declares the same group gets the same layout object, and a bind group
 * made for one serves the others.
 */

import type { u32 } from "@xpute/core/kit/word.ts";

import type { PxlExports } from "@/wasm/pxl.ts";
import { DEPTH_FORMAT } from "./depth.ts";

/** shader.rs's pipeline ids past draw.rs's. */
export const PIPELINE_CHUNK_DEPTH = 10;
export const PIPELINE_DECOR_DEPTH = 11;
export const PIPELINE_SKINNED_DEPTH = 12;
export const PIPELINE_SKY = 13;
export const PIPELINE_MIPS = 14;

/** shader.rs's codes. */
const TOPOLOGY: GPUPrimitiveTopology[] = ["triangle-list", "triangle-strip", "line-list"];
const CULL: GPUCullMode[] = ["none", "back"];
const COMPARE: GPUCompareFunction[] = ["always", "greater", "greater-equal", "less"];
const NO_DEPTH = 4;
const ALPHA = 1;
const ONE_SAMPLE = 1;
const STEP: GPUVertexStepMode[] = ["vertex", "instance"];
const FORMAT: GPUVertexFormat[] = ["float32", "float32x2", "float32x3", "float32x4", "uint32x4"];
const VERTEX = 1;
const FRAGMENT = 2;
const UNIFORM = 0;
const UNIFORM_DYNAMIC = 1;
const STORAGE = 2;
const TEXTURE = 3;
const TEXTURE_ARRAY = 4;
const DEPTH = 5;
const SAMPLER = 6;
const SAMPLER_COMPARISON = 7;
const FRAME_DEPTH = 8;

/** Straight alpha over what is drawn. */
const ALPHA_BLEND: GPUBlendState = {
  color: { srcFactor: "src-alpha", dstFactor: "one-minus-src-alpha", operation: "add" },
  alpha: { srcFactor: "one", dstFactor: "one-minus-src-alpha", operation: "add" },
};

export type VertexLane = { stride: u32; step: u32 };

/** A pipeline record read into its parts. */
export type PipelineRecord = {
  shader: u32;
  shader_msaa: u32;
  fragment: boolean;
  topology: u32;
  cull: u32;
  depth_write: boolean;
  depth_compare: u32;
  depth_bias: number;
  depth_bias_slope: number;
  blend: u32;
  samples: u32;
  /** Each vertex buffer's stride and step, and its attributes. */
  buffers: { stride: u32; step: u32; attributes: GPUVertexAttribute[] }[];
  /** Each group's entries as (binding, visibility, kind). */
  groups: [u32, u32, u32][][];
};

export function read_record(words: Uint32Array): PipelineRecord {
  let at = 11;
  const next = () => words[at++];
  const buffers = Array.from({ length: next() }, () => {
    const stride = next();
    const step = next();
    const attributes = Array.from({ length: next() }, (): GPUVertexAttribute => ({ shaderLocation: next(), offset: next(), format: FORMAT[next()] }));
    return { stride, step, attributes };
  });
  const groups = Array.from({ length: next() }, () => Array.from({ length: next() }, (): [u32, u32, u32] => [next(), next(), next()]));
  return {
    shader: words[0],
    shader_msaa: words[1],
    fragment: words[2] !== 0,
    topology: words[3],
    cull: words[4],
    depth_write: words[5] !== 0,
    depth_compare: words[6],
    depth_bias: words[7] | 0,
    depth_bias_slope: new Float32Array(words.buffer, words.byteOffset + 32, 1)[0],
    blend: words[9],
    samples: words[10],
    buffers,
    groups,
  };
}

/** The frame uniform's layout, as shader.rs's FRAME_AT names it. */
export type FrameLayout = {
  floats: u32;
  view_proj: u32;
  camera_pos: u32;
  edge_strength: u32;
  /** The environment's run: sun_dir through fog_color (environment.rs). */
  env: u32;
  low_spec: u32;
  edge_width: u32;
  rim_intensity: u32;
  viewport: u32;
  shadow_strength: u32;
  shadow_texel: u32;
  vignette: u32;
  light_view_proj: u32;
  /** The second light's run: other_dir through other_color
   * (environment.rs's ENV_OTHER_LIGHT). */
  other: u32;
};

export function read_frame_layout(pxl: PxlExports): FrameLayout {
  const t = new Uint32Array(pxl.memory.buffer, pxl.pxl_frame_uniform_at(), 14);
  return {
    floats: t[0],
    view_proj: t[1],
    camera_pos: t[2],
    edge_strength: t[3],
    env: t[4],
    low_spec: t[5],
    edge_width: t[6],
    rim_intensity: t[7],
    viewport: t[8],
    shadow_strength: t[9],
    shadow_texel: t[10],
    vignette: t[11],
    light_view_proj: t[12],
    other: t[13],
  };
}

export type BuiltPipeline = {
  pipeline: GPURenderPipeline;
  layouts: GPUBindGroupLayout[];
  record: PipelineRecord;
};

export class PipelineBuilder {
  private readonly modules = new Map<u32, GPUShaderModule>();
  private readonly layouts = new Map<string, GPUBindGroupLayout>();
  private readonly records = new Map<u32, PipelineRecord>();

  constructor(private readonly device: GPUDevice, private readonly pxl: PxlExports) {}

  /** Pipeline `id`'s record, as the module has it. */
  record(id: u32): PipelineRecord {
    let r = this.records.get(id);
    if (!r) {
      const pxl = this.pxl;
      const row = new Uint32Array(pxl.memory.buffer, pxl.pxl_pipeline(id), 2);
      if (row[1] === 0) throw new Error(`gpu: no pipeline ${id}`);
      r = read_record(new Uint32Array(pxl.memory.buffer, row[0], row[1]));
      this.records.set(id, r);
    }
    return r;
  }

  private module(kind: u32): GPUShaderModule {
    let m = this.modules.get(kind);
    if (!m) {
      const pxl = this.pxl;
      const row = new Uint32Array(pxl.memory.buffer, pxl.pxl_shader(kind), 2);
      if (row[1] === 0) throw new Error(`gpu: no shader ${kind}`);
      const code = new TextDecoder().decode(new Uint8Array(pxl.memory.buffer, row[0], row[1]));
      m = this.device.createShaderModule({ label: `shader/${kind}`, code });
      this.modules.set(kind, m);
    }
    return m;
  }

  /** The layout for a group's entries, one object per distinct group. */
  layout(entries: [u32, u32, u32][], samples: u32): GPUBindGroupLayout {
    const key = `${samples > 1 ? "m" : "s"}:${entries.join(",")}`;
    let l = this.layouts.get(key);
    if (!l) {
      l = this.device.createBindGroupLayout({
        entries: entries.map(([binding, visibility, kind]) => {
          const e: GPUBindGroupLayoutEntry = { binding, visibility: (visibility & VERTEX ? GPUShaderStage.VERTEX : 0) | (visibility & FRAGMENT ? GPUShaderStage.FRAGMENT : 0) };
          switch (kind) {
            case UNIFORM:
              e.buffer = { type: "uniform" };
              break;
            case UNIFORM_DYNAMIC:
              e.buffer = { type: "uniform", hasDynamicOffset: true };
              break;
            case STORAGE:
              e.buffer = { type: "read-only-storage" };
              break;
            case TEXTURE:
              e.texture = {};
              break;
            case TEXTURE_ARRAY:
              e.texture = { viewDimension: "2d-array" };
              break;
            case DEPTH:
              e.texture = { sampleType: "depth" };
              break;
            case FRAME_DEPTH:
              e.texture = { sampleType: "depth", multisampled: samples > 1 };
              break;
            case SAMPLER:
              e.sampler = {};
              break;
            case SAMPLER_COMPARISON:
              e.sampler = { type: "comparison" };
              break;
          }
          return e;
        }),
      });
      this.layouts.set(key, l);
    }
    return l;
  }

  /** Pipeline `id` over `format` at `samples` a pixel — the frame's, unless
   * the record fixes one. */
  build(id: u32, label: string, format: GPUTextureFormat, samples: u32): BuiltPipeline {
    const r = this.record(id);
    const count = r.samples === ONE_SAMPLE ? 1 : samples;
    const layouts = r.groups.map((g) => this.layout(g, count));
    const module = this.module(count > 1 ? r.shader_msaa : r.shader);
    const descriptor: GPURenderPipelineDescriptor = {
      label,
      layout: this.device.createPipelineLayout({ bindGroupLayouts: layouts }),
      vertex: { module, entryPoint: "vs", buffers: r.buffers.map((b) => ({ arrayStride: b.stride, stepMode: STEP[b.step], attributes: b.attributes })) },
      primitive: { topology: TOPOLOGY[r.topology], cullMode: CULL[r.cull], frontFace: "ccw" },
      multisample: { count },
    };
    if (r.fragment) descriptor.fragment = { module, entryPoint: "fs", targets: [{ format, blend: r.blend === ALPHA ? ALPHA_BLEND : undefined }] };
    if (r.depth_compare !== NO_DEPTH) {
      descriptor.depthStencil = { format: DEPTH_FORMAT, depthWriteEnabled: r.depth_write, depthCompare: COMPARE[r.depth_compare], depthBias: r.depth_bias, depthBiasSlopeScale: r.depth_bias_slope };
    }
    return { pipeline: this.device.createRenderPipeline(descriptor), layouts, record: r };
  }
}
