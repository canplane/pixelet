// @pxl/web/render/gpu/badges.ts

/**
 * The billboards through WebGPU — the POI badges and the chat bubble:
 * billboard/field.ts's instanced quad and anchor_occlusion.ts's two depth
 * questions, over the module's billboard sets
 * (crates/pxl-core/src/billboard.rs), one draw a set.
 *
 * They draw in a pass of their own after the world, with the world's
 * depth attached read-only and bound as a texture at once, so both questions
 * are asked of this frame's depth. Whether the labelled point is visible is
 * asked in the vertex shader, at five points up the badge from just above
 * the anchor, and answered for the whole badge; whether something stands
 * between the camera and the badge itself is the depth test's, against the
 * depth of an upright twin of the quad written as the fragment's. The copy
 * three needed, a frame stale and reprojected, has no counterpart here.
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

import { GRADE_WGSL } from "./grade.ts";

const DRAW_WORDS = 5;
const UNIFORM_FLOATS = 8;
const PIPELINE_BADGE = 8;

/** anchor_occlusion.ts's tolerances and sampling, and why they are what
 * they are. */
const DEPTH_BIAS_U = 1;
const DEPTH_BIAS_CELLS = 1.5;
const DEPTH_SAMPLES = 5;
const DEPTH_HIDE_AT = 3;
const DEPTH_LIFT_U = 2;

const BADGE_UNIFORM_FLOATS = 44;

const f = (v: number) => (Number.isInteger(v) ? `${v}.0` : `${v}`);

export const BADGE_SHADER: string = /* wgsl */ `
struct Badge {
  view: mat4x4f,
  proj: mat4x4f,
  translation: vec3f,
  px_k: f32,
  fade_start: f32,
  fade_end: f32,
  near: f32,
  far: f32,
  cell: f32,
  _p0: f32,
  _p1: f32,
  _p2: f32,
};

@group(0) @binding(0) var<uniform> badge: Badge;
@group(0) @binding(1) var<storage, read> instances: array<f32>;
@group(0) @binding(2) var world_depth: texture_depth_2d;
@group(1) @binding(0) var sheet: texture_2d<f32>;
@group(1) @binding(1) var sheet_sampler: sampler;

struct VertexOut {
  @builtin(position) clip: vec4f,
  @location(0) uv: vec2f,
  @location(1) opacity: f32,
  @location(2) upright_z: f32,
  @location(3) @interpolate(flat) hidden: u32,
};

// Distance from the camera, in world units, for a depth-buffer value in
// [0, 1] — which is GL's window depth too, so the formula is three's.
fn view_distance(d: f32) -> f32 {
  let z = d * 2.0 - 1.0;
  return (2.0 * badge.near * badge.far) / (badge.far + badge.near - z * (badge.far - badge.near));
}

// Whether the world stands in front of a point of the badge, given in clip
// space. A point off the screen has no depth to read and counts as visible.
fn behind_world(clip: vec4f) -> bool {
  if (clip.w <= 0.0) {
    return false;
  }
  let ndc = clip.xyz / clip.w;
  let uv = vec2f(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5);
  if (any(uv < vec2f(0.0)) || any(uv >= vec2f(1.0))) {
    return false;
  }
  let dims = vec2f(textureDimensions(world_depth));
  let world = view_distance(textureLoad(world_depth, vec2i(uv * dims), 0));
  let point = view_distance(ndc.z);
  return world < point - ${f(DEPTH_BIAS_U)} - point * badge.cell * ${f(DEPTH_BIAS_CELLS)};
}

@vertex
fn vs(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> VertexOut {
  let k = ii * 10u;
  let anchor = vec3f(instances[k], instances[k + 1u], instances[k + 2u]) + badge.translation;
  let size_px = vec2f(instances[k + 3u], instances[k + 4u]);
  let rect = vec4f(instances[k + 5u], instances[k + 6u], instances[k + 7u], instances[k + 8u]);
  let frac = instances[k + 9u];
  let corners = array<vec2f, 4>(vec2f(-0.5, -0.5), vec2f(0.5, -0.5), vec2f(-0.5, 0.5), vec2f(0.5, 0.5));
  let q = corners[vi];

  let anchor_view = (badge.view * vec4f(anchor, 1.0)).xyz;
  let dist = length(anchor_view);
  let depth = max(-anchor_view.z, 1e-3);
  let fade = 1.0 - smoothstep(badge.fade_start, badge.fade_end, dist);
  let size = size_px * depth / badge.px_k * fade;
  let drop = size.y * (frac - 0.5);
  let centre_view = (badge.view * vec4f(anchor + vec3f(0.0, 0.0, drop), 1.0)).xyz;
  let corner = q * size;

  var o: VertexOut;
  o.clip = badge.proj * vec4f(centre_view + vec3f(corner, 0.0), 1.0);
  // The sheet's v0 is the rectangle's top edge.
  o.uv = vec2f(mix(rect.x, rect.z, q.x + 0.5), mix(rect.w, rect.y, q.y + 0.5));
  o.opacity = fade;

  let view_proj = badge.proj * badge.view;
  let lifted = view_proj * vec4f(anchor + vec3f(0.0, 0.0, ${f(DEPTH_LIFT_U)}), 1.0);
  let rise = view_proj * vec4f(0.0, 0.0, size.y, 0.0);
  var hidden = 0u;
  for (var s = 0u; s < ${DEPTH_SAMPLES}u; s++) {
    if (behind_world(lifted + rise * (f32(s) / ${f(DEPTH_SAMPLES - 1)}))) {
      hidden++;
    }
  }
  o.hidden = select(0u, 1u, hidden >= ${DEPTH_HIDE_AT}u);

  let world_up_view = normalize((badge.view * vec4f(0.0, 0.0, 1.0, 0.0)).xyz);
  let push = ${f(DEPTH_BIAS_U)} + max(0.0, -centre_view.z) * badge.cell * ${f(DEPTH_BIAS_CELLS)};
  let upright = badge.proj * vec4f(centre_view + vec3f(corner.x, 0.0, push) + world_up_view * corner.y, 1.0);
  o.upright_z = clamp(upright.z / upright.w, 0.0, 1.0);
  return o;
}

${GRADE_WGSL}

struct FragOut {
  @location(0) color: vec4f,
  @builtin(frag_depth) depth: f32,
};

@fragment
fn fs(i: VertexOut) -> FragOut {
  let texel = textureSample(sheet, sheet_sampler, i.uv);
  if (i.hidden != 0u) {
    discard;
  }
  var o: FragOut;
  o.color = vec4f(to_srgb(grade(texel.rgb)), texel.a * i.opacity);
  o.depth = i.upright_z;
  return o;
}
`;

/** One level of the sheet's mip chain from the level above, box-filtered by
 * a linear sample at the centre of each 2x2. */
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
  private readonly mip_pipeline: GPURenderPipeline;
  private readonly sampler: GPUSampler;
  private readonly mip_sampler: GPUSampler;
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

  constructor(private readonly device: GPUDevice, format: GPUTextureFormat, depth_format: GPUTextureFormat) {
    const module = device.createShaderModule({ label: "badges", code: BADGE_SHADER });
    this.pipeline = device.createRenderPipeline({
      label: "badges",
      layout: "auto",
      vertex: { module, entryPoint: "vs" },
      fragment: {
        module,
        entryPoint: "fs",
        targets: [{
          format,
          blend: {
            color: { srcFactor: "src-alpha", dstFactor: "one-minus-src-alpha", operation: "add" },
            alpha: { srcFactor: "one", dstFactor: "one-minus-src-alpha", operation: "add" },
          },
        }],
      },
      primitive: { topology: "triangle-strip" },
      depthStencil: { format: depth_format, depthWriteEnabled: false, depthCompare: "less-equal" },
    });
    const mip = device.createShaderModule({ label: "badges/mip", code: MIP_SHADER });
    this.mip_pipeline = device.createRenderPipeline({
      label: "badges/mip",
      layout: "auto",
      vertex: { module: mip, entryPoint: "vs" },
      fragment: { module: mip, entryPoint: "fs", targets: [{ format: SHEET_FORMAT }] },
      primitive: { topology: "triangle-list" },
    });
    this.sampler = device.createSampler({ magFilter: "nearest", minFilter: "linear", mipmapFilter: "linear" });
    this.mip_sampler = device.createSampler({ magFilter: "linear", minFilter: "linear" });
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
          mipLevelCount: Math.floor(Math.log2(size)) + 1,
          usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST | GPUTextureUsage.RENDER_ATTACHMENT,
        });
        d.queue.copyExternalImageToTexture({ source: canvas }, { texture: this.sheet }, [size, size]);
        this.sheet_group = d.createBindGroup({
          layout: this.pipeline.getBindGroupLayout(1),
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
    if (changed && this.sheet) this.build_mips(this.sheet);
  }

  private build_mips(texture: GPUTexture): void {
    const d = this.device;
    const encoder = d.createCommandEncoder({ label: "badges/mip" });
    for (let level = 1; level < texture.mipLevelCount; level++) {
      const group = d.createBindGroup({
        layout: this.mip_pipeline.getBindGroupLayout(0),
        entries: [
          { binding: 0, resource: texture.createView({ baseMipLevel: level - 1, mipLevelCount: 1 }) },
          { binding: 1, resource: this.mip_sampler },
        ],
      });
      const pass = encoder.beginRenderPass({
        colorAttachments: [{ view: texture.createView({ baseMipLevel: level, mipLevelCount: 1 }), loadOp: "clear", storeOp: "store" }],
      });
      pass.setPipeline(this.mip_pipeline);
      pass.setBindGroup(0, group);
      pass.draw(3);
      pass.end();
    }
    d.queue.submit([encoder.finish()]);
  }

  /** Every billboard set's draw, in the order listed, in one pass of
   * their own over `color` reading `depth`; how many instances it drew. */
  draw(encoder: GPUCommandEncoder, color: GPUTextureView, depth: GPUTexture, draws: Uint32Array, uniforms: Float32Array, buffers: ReadonlyMap<u32, GPUBuffer>): u32 {
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
          layout: this.pipeline.getBindGroupLayout(0),
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
          colorAttachments: [{ view: color, loadOp: "load", storeOp: "store" }],
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
