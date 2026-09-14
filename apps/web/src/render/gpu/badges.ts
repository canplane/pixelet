// @pxl/web/render/gpu/badges.ts

/**
 * The billboards through WebGPU — the POI badges and the chat bubble: one
 * instanced quad over the module's billboard sets
 * (crates/pxl-core/src/billboard.rs), one draw a set, in a pass of their
 * own after the world with the world's depth attached read-only and bound
 * as a texture at once (shader/badge.wgsl asks both of its depth questions
 * of this frame's depth). The copy three needed, a frame stale and
 * reprojected, has no counterpart here.
 *
 * The sheet is billboard/atlas.ts's canvas, mirrored into a texture a badge
 * at a time and whole when the sheet is replaced, with its mip chain
 * rebuilt after. Magnification is nearest, for the pixel read up close;
 * minification is mipmapped linear, because a badge held at a constant
 * screen size is still slightly minified against the sheet's supersampled
 * resolution, and without a mip chain every camera micro-movement samples a
 * different single texel and the text visibly crawls.
 */

import type { f64, u32 } from "@xpute/core/kit/word.ts";

import { mip_level_count, MipBuilder } from "./mips.ts";
import type { PipelineBuilder } from "./pipeline.ts";

const DRAW_WORDS = 5;
const UNIFORM_FLOATS = 8;
const PIPELINE_BADGE = 8;

/** Floats the badge uniform holds (shader/badge.wgsl's Badge). */
const BADGE_UNIFORM_FLOATS = 44;

/** What the pass reads off billboard/atlas.ts's sheet. */
export type BadgeSheet = {
  /** Mirrors what changed since `generation`: `whole` when the sheet was
   * replaced, else `part` per badge drawn, into a scratch canvas at its
   * origin. The generation it leaves the mirror at. */
  flush_into(generation: u32, whole: (canvas: HTMLCanvasElement, size: u32) => void, part: (scratch: HTMLCanvasElement, w: u32, h: u32, x: u32, y: u32) => void): u32;
};

const SHEET_FORMAT: GPUTextureFormat = "rgba8unorm-srgb";

export class BadgePass {
  private readonly pipeline: GPURenderPipeline;
  private readonly layouts: GPUBindGroupLayout[];
  private readonly sampler: GPUSampler;
  private readonly mips: MipBuilder;
  private readonly scratch = new Float32Array(new ArrayBuffer(BADGE_UNIFORM_FLOATS * 4));
  private sheet: GPUTexture | null = null;
  private generation: u32 = -1 >>> 0;
  private sheet_group: GPUBindGroup | null = null;
  /** Per set drawn, by its place in the frame's list. */
  private readonly sets: { uniform: GPUBuffer; group: GPUBindGroup | null; block: GPUBuffer | null; depth: GPUTexture | null }[] = [];
  /** The per-frame inputs the glue writes: the view in the anchor root's
   * frame and the projection into WebGPU's clip space, the projection's
   * pixels per unit at one unit of depth, the fade's range, the camera's
   * near and far, and the tier ladder's cell per distance. */
  readonly view = new Float32Array(16);
  readonly proj = new Float32Array(16);
  px_k: f64 = 1;
  fade_start: f64 = 0;
  fade_end: f64 = 1;
  near: f64 = 0.1;
  far: f64 = 1000;
  cell: f64 = 0.01;

  constructor(private readonly device: GPUDevice, pipelines: PipelineBuilder, format: GPUTextureFormat, sample_count: u32, mips: MipBuilder) {
    const { pipeline, layouts } = pipelines.build(PIPELINE_BADGE, "badges", format, sample_count);
    this.pipeline = pipeline;
    this.layouts = layouts;
    this.mips = mips;
    this.sampler = device.createSampler({ magFilter: "nearest", minFilter: "linear", mipmapFilter: "linear" });
  }

  /** Mirrors the sheet, and rebuilds its mip chain when anything moved. */
  sync_sheet(sheet: BadgeSheet): void {
    const d = this.device;
    let changed = false;
    this.generation = sheet.flush_into(
      this.generation,
      (canvas, size) => {
        this.sheet?.destroy();
        this.sheet = d.createTexture({
          label: "badges/sheet",
          size: [size, size],
          format: SHEET_FORMAT,
          mipLevelCount: mip_level_count(size, size),
          usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST | GPUTextureUsage.RENDER_ATTACHMENT,
        });
        d.queue.copyExternalImageToTexture({ source: canvas }, { texture: this.sheet }, [size, size]);
        this.sheet_group = d.createBindGroup({
          layout: this.layouts[1],
          entries: [{ binding: 0, resource: this.sheet.createView() }, { binding: 1, resource: this.sampler }],
        });
        changed = true;
      },
      (scratch, w, h, x, y) => {
        if (!this.sheet) return;
        d.queue.copyExternalImageToTexture({ source: scratch }, { texture: this.sheet, origin: [x, y] }, [w, h]);
        changed = true;
      },
    );
    if (changed && this.sheet) this.mips.build(this.sheet);
  }

  /** Every billboard set's draw, in the order listed, in one pass of
   * their own over `color` (resolved into `resolve` where the colour is
   * multisampled) reading `depth`; how many instances it drew. */
  draw(
    encoder: GPUCommandEncoder,
    color: GPUTextureView,
    resolve: GPUTextureView | undefined,
    depth: GPUTexture,
    draws: Uint32Array,
    uniforms: Float32Array,
    buffers: ReadonlyMap<u32, GPUBuffer>,
  ): u32 {
    if (!this.sheet_group) return 0;
    let pass: GPURenderPassEncoder | null = null;
    let n = 0;
    let drawn: u32 = 0;
    for (let k = 0; k < draws.length; k += DRAW_WORDS) {
      if (draws[k + 4] !== PIPELINE_BADGE) continue;
      const block = buffers.get(draws[k]);
      const count = draws[k + 1];
      if (!block || count === 0) continue;
      // A uniform buffer a set: writes to one buffer all land before the
      // pass runs, so a shared one would place every set where the last is.
      if (n === this.sets.length) {
        this.sets.push({
          uniform: this.device.createBuffer({ label: "badges/uniform", size: BADGE_UNIFORM_FLOATS * 4, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST }),
          group: null,
          block: null,
          depth: null,
        });
      }
      const set = this.sets[n++];
      const at = draws[k + 3] * UNIFORM_FLOATS;
      const s = this.scratch;
      s.set(this.view, 0);
      s.set(this.proj, 16);
      s[32] = uniforms[at];
      s[33] = uniforms[at + 1];
      s[34] = uniforms[at + 2];
      s[35] = this.px_k;
      s[36] = this.fade_start;
      s[37] = this.fade_end;
      s[38] = this.near;
      s[39] = this.far;
      s[40] = this.cell;
      this.device.queue.writeBuffer(set.uniform, 0, s);
      if (set.block !== block || set.depth !== depth) {
        set.group = this.device.createBindGroup({
          layout: this.layouts[0],
          entries: [
            { binding: 0, resource: { buffer: set.uniform } },
            { binding: 1, resource: { buffer: block } },
            { binding: 2, resource: depth.createView() },
          ],
        });
        set.block = block;
        set.depth = depth;
      }
      if (!pass) {
        pass = encoder.beginRenderPass({
          label: "badges",
          colorAttachments: [{ view: color, resolveTarget: resolve, loadOp: "load", storeOp: "store" }],
          depthStencilAttachment: { view: depth.createView(), depthReadOnly: true },
        });
        pass.setPipeline(this.pipeline);
        pass.setBindGroup(1, this.sheet_group);
      }
      pass.setBindGroup(0, set.group!);
      pass.draw(4, count);
      drawn += count;
    }
    pass?.end();
    return drawn;
  }
}
