// @pxl/web/render/gpu/debug.ts

/**
 * The dev gizmos through WebGPU: the module's line segments
 * (crates/pxl-core/src/world/debug.rs) drawn as one line list over everything
 * (shader/debug.wgsl); the labels are DOM (marker/debug_layer.tsx).
 */

import type { u32 } from "@xpute/core/kit/word.ts";

import type { PipelineBuilder } from "./pipeline.ts";

const DRAW_WORDS = 5;
const UNIFORM_FLOATS = 8;
const PIPELINE_DEBUG = 9;

export class DebugPass {
  private readonly pipeline: GPURenderPipeline;
  private readonly draw_buffer: GPUBuffer;
  private readonly frame_group: GPUBindGroup;
  private readonly draw_group: GPUBindGroup;
  private readonly scratch = new Float32Array(new ArrayBuffer(16));

  constructor(private readonly device: GPUDevice, pipelines: PipelineBuilder, format: GPUTextureFormat, frame_buffer: GPUBuffer, sample_count: u32) {
    const { pipeline, layouts } = pipelines.build(PIPELINE_DEBUG, "debug", format, sample_count);
    this.pipeline = pipeline;
    this.draw_buffer = device.createBuffer({ label: "debug/draw", size: 16, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST });
    this.frame_group = device.createBindGroup({ layout: layouts[0], entries: [{ binding: 0, resource: { buffer: frame_buffer } }] });
    this.draw_group = device.createBindGroup({ layout: layouts[1], entries: [{ binding: 0, resource: { buffer: this.draw_buffer } }] });
  }

  /** The frame's segments into the open pass; how many vertices. */
  draw(pass: GPURenderPassEncoder, draws: Uint32Array, uniforms: Float32Array, buffers: ReadonlyMap<u32, GPUBuffer>): u32 {
    for (let k = 0; k < draws.length; k += DRAW_WORDS) {
      if (draws[k + 4] !== PIPELINE_DEBUG) continue;
      const block = buffers.get(draws[k]);
      const count = draws[k + 1];
      if (!block || count === 0) return 0;
      const at = draws[k + 3] * UNIFORM_FLOATS;
      this.scratch[0] = uniforms[at];
      this.scratch[1] = uniforms[at + 1];
      this.scratch[2] = uniforms[at + 2];
      this.device.queue.writeBuffer(this.draw_buffer, 0, this.scratch);
      pass.setPipeline(this.pipeline);
      pass.setBindGroup(0, this.frame_group);
      pass.setBindGroup(1, this.draw_group);
      pass.setVertexBuffer(0, block, 0, count * 24);
      pass.draw(count);
      return count;
    }
    return 0;
  }
}
