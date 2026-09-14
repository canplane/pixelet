// @pxl/web/render/gpu/mips.ts

/**
 * A mip chain built on the device: each level from the one above,
 * box-filtered by one linear sample at the centre of each 2x2, a render
 * pass a level (shader/mips.wgsl). WebGPU has no generateMipmap; this is
 * the one every texture that minifies asks for — the badge sheet, the
 * texture pack's layers.
 */

import type { u32 } from "@xpute/core/kit/word.ts";

import { PIPELINE_MIPS, type PipelineBuilder } from "./pipeline.ts";

/** Levels a square or rectangular texture of `size` has down to one texel. */
export function mip_level_count(width: u32, height: u32): u32 {
  return Math.floor(Math.log2(Math.max(width, height))) + 1;
}

export class MipBuilder {
  private readonly pipelines = new Map<GPUTextureFormat, GPURenderPipeline>();
  private layout: GPUBindGroupLayout | null = null;
  private readonly sampler: GPUSampler;

  constructor(private readonly device: GPUDevice, private readonly builder: PipelineBuilder) {
    this.sampler = device.createSampler({ magFilter: "linear", minFilter: "linear" });
  }

  /** The pipeline into `format`: the one record, a pipeline a format. */
  private pipeline_for(format: GPUTextureFormat): GPURenderPipeline {
    let p = this.pipelines.get(format);
    if (!p) {
      const built = this.builder.build(PIPELINE_MIPS, `mips/${format}`, format, 1);
      p = built.pipeline;
      this.layout = built.layouts[0];
      this.pipelines.set(format, p);
    }
    return p;
  }

  /** Builds every level of `texture` below the first, for `layers` array
   * layers from `first_layer`; the texture needs RENDER_ATTACHMENT and
   * TEXTURE_BINDING usage. */
  build(texture: GPUTexture, first_layer: u32 = 0, layers: u32 = 1): void {
    const d = this.device;
    const pipeline = this.pipeline_for(texture.format);
    const encoder = d.createCommandEncoder({ label: "mips" });
    for (let layer = first_layer; layer < first_layer + layers; layer++) {
      for (let level = 1; level < texture.mipLevelCount; level++) {
        const group = d.createBindGroup({
          layout: this.layout!,
          entries: [
            { binding: 0, resource: texture.createView({ dimension: "2d", baseMipLevel: level - 1, mipLevelCount: 1, baseArrayLayer: layer, arrayLayerCount: 1 }) },
            { binding: 1, resource: this.sampler },
          ],
        });
        const pass = encoder.beginRenderPass({
          colorAttachments: [{ view: texture.createView({ dimension: "2d", baseMipLevel: level, mipLevelCount: 1, baseArrayLayer: layer, arrayLayerCount: 1 }), loadOp: "clear", storeOp: "store" }],
        });
        pass.setPipeline(pipeline);
        pass.setBindGroup(0, group);
        pass.draw(3);
        pass.end();
      }
    }
    d.queue.submit([encoder.finish()]);
  }
}
