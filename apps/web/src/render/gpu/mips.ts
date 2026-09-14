// @pxl/web/render/gpu/mips.ts

/**
 * A mip chain built on the device: each level from the one above,
 * box-filtered by one linear sample at the centre of each 2x2, a render
 * pass a level. WebGPU has no generateMipmap; this is the one every
 * texture that minifies asks for — the badge sheet, the texture pack's
 * layers.
 */

import type { u32 } from "@xpute/core/kit/word.ts";

export const MIP_SHADER: string = /* wgsl */ `
@group(0) @binding(0) var above: texture_2d<f32>;
@group(0) @binding(1) var above_sampler: sampler;

struct VertexOut {
  @builtin(position) clip: vec4f,
  @location(0) uv: vec2f,
};

@vertex
fn vs(@builtin(vertex_index) vi: u32) -> VertexOut {
  let p = array<vec2f, 3>(vec2f(-1.0, -1.0), vec2f(3.0, -1.0), vec2f(-1.0, 3.0));
  var o: VertexOut;
  o.clip = vec4f(p[vi], 0.0, 1.0);
  o.uv = vec2f(p[vi].x * 0.5 + 0.5, 0.5 - p[vi].y * 0.5);
  return o;
}

@fragment
fn fs(i: VertexOut) -> @location(0) vec4f {
  return textureSample(above, above_sampler, i.uv);
}
`;

/** Levels a square or rectangular texture of `size` has down to one texel. */
export function mip_level_count(width: u32, height: u32): u32 {
  return Math.floor(Math.log2(Math.max(width, height))) + 1;
}

export class MipBuilder {
  private readonly pipelines = new Map<GPUTextureFormat, GPURenderPipeline>();
  private readonly module: GPUShaderModule;
  private readonly sampler: GPUSampler;

  constructor(private readonly device: GPUDevice) {
    this.module = device.createShaderModule({ label: "mips", code: MIP_SHADER });
    this.sampler = device.createSampler({ magFilter: "linear", minFilter: "linear" });
  }

  private pipeline_for(format: GPUTextureFormat): GPURenderPipeline {
    let p = this.pipelines.get(format);
    if (!p) {
      p = this.device.createRenderPipeline({
        label: `mips/${format}`,
        layout: "auto",
        vertex: { module: this.module, entryPoint: "vs" },
        fragment: { module: this.module, entryPoint: "fs", targets: [{ format }] },
        primitive: { topology: "triangle-list" },
      });
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
          layout: pipeline.getBindGroupLayout(0),
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
