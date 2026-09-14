// @pxl/web/render/gpu/shadow.ts

/**
 * The sun's shadow map: the world drawn once more each frame from the sun,
 * depth only, into a square map over the map centre, which every lit pass
 * then compares its fragments against (frame_wgsl.ts's sun_lit). The
 * light's frame is a second frame uniform whose view_proj is the light's,
 * so each pass draws its depth with the vertex shader it already has.
 *
 * Orthographic, 400 units across at the sun's angle, near enough for a
 * texel to be a fifth of a unit on a 2048 map — the extent three's shadow
 * camera was tuned at: further than that the map's texels grow past a
 * cell and the shadow reads as blocks. What the map costs is its texels
 * times four bytes, once: 16 MB at 2048, 4 MB at 1024, the phone's.
 */

import type { f64, u32 } from "@xpute/core/kit/word.ts";
import { mat4, mat4_look_at, mat4_mul } from "@xpute/core/kit/mat4.ts";

import { HANDHELD } from "@/shared/lib/browser/handheld.ts";
import { FRAME_AT, FRAME_UNIFORM_FLOATS } from "./frame_wgsl.ts";

export const SHADOW_FORMAT: GPUTextureFormat = "depth32float";
const HALF_EXTENT: f64 = 200;
const LIGHT_DISTANCE: f64 = 1200;
const LIGHT_FAR: f64 = 2400;
const MAP_SIZE: u32 = HANDHELD ? 1024 : 2048;

const $view = mat4();
const $proj = mat4();

/** An orthographic projection into WebGPU clip space, z in [0, 1] from
 * near to far, column-major. */
function ortho(out: Float64Array, half: f64, near: f64, far: f64): Float64Array {
  out.fill(0);
  out[0] = 1 / half;
  out[5] = 1 / half;
  out[10] = -1 / (far - near);
  out[14] = -near / (far - near);
  out[15] = 1;
  return out;
}

export class ShadowPass {
  readonly map: GPUTexture;
  private readonly sampler: GPUSampler;
  private blank: GPUBindGroupEntry[] | null = null;
  /** The light's frame: the frame uniform with the light's view_proj. */
  readonly light_frame: GPUBuffer;
  readonly light_view_proj = mat4();
  private readonly light_uniforms = new Float32Array(new ArrayBuffer(FRAME_UNIFORM_FLOATS * 4));
  readonly bytes: u32;
  /** One texel of the map, in units: the normal offset a fragment takes. */
  readonly texel: f64;

  constructor(private readonly device: GPUDevice) {
    this.map = device.createTexture({ label: "shadow", size: [MAP_SIZE, MAP_SIZE], format: SHADOW_FORMAT, usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.TEXTURE_BINDING });
    this.bytes = MAP_SIZE * MAP_SIZE * 4;
    this.texel = (HALF_EXTENT * 2) / MAP_SIZE;
    this.sampler = device.createSampler({ compare: "less-equal", magFilter: "linear", minFilter: "linear" });
    this.light_frame = device.createBuffer({ label: "shadow/frame", size: FRAME_UNIFORM_FLOATS * 4, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST });
  }

  /** The map and its sampler as a bind group layout's entries, at
   * `binding` and the next, beside a pass's frame uniform. */
  static layout_entries(binding: u32): GPUBindGroupLayoutEntry[] {
    return [
      { binding, visibility: GPUShaderStage.FRAGMENT, texture: { sampleType: "depth" } },
      { binding: binding + 1, visibility: GPUShaderStage.FRAGMENT, sampler: { type: "comparison" } },
    ];
  }

  /** The same, as a bind group's entries. */
  group_entries(binding: u32): GPUBindGroupEntry[] {
    return [{ binding, resource: this.map.createView() }, { binding: binding + 1, resource: this.sampler }];
  }

  /** A map nothing was drawn into and a sampler, for a pass with no
   * shadow — a case, or the depth pass itself, which cannot read the map it
   * is writing: every fragment reads as lit. */
  static blank_entries(device: GPUDevice, binding: u32): GPUBindGroupEntry[] {
    const blank = device.createTexture({ size: [1, 1], format: SHADOW_FORMAT, usage: GPUTextureUsage.TEXTURE_BINDING });
    return [{ binding, resource: blank.createView() }, { binding: binding + 1, resource: device.createSampler({ compare: "less-equal" }) }];
  }

  /** The same, for the light's own bind groups. */
  light_entries(binding: u32): GPUBindGroupEntry[] {
    this.blank ??= ShadowPass.blank_entries(this.device, 0);
    return [{ binding, resource: this.blank[0].resource }, { binding: binding + 1, resource: this.blank[1].resource }];
  }

  /** Aims the light: from `sun_dir` at `target`, both in the root frame,
   * and writes the light's frame. */
  aim(sun_dir: ArrayLike<number>, target: { x: f64; y: f64; z: f64 }): void {
    const eye = { x: target.x + sun_dir[0] * LIGHT_DISTANCE, y: target.y + sun_dir[1] * LIGHT_DISTANCE, z: target.z + sun_dir[2] * LIGHT_DISTANCE };
    mat4_look_at($view, eye, target, { x: 0, y: 0, z: 1 });
    mat4_mul(this.light_view_proj, ortho($proj, HALF_EXTENT, 0.1, LIGHT_FAR), $view);
    const u = this.light_uniforms;
    u.set(this.light_view_proj, FRAME_AT.view_proj);
    u[FRAME_AT.camera_pos] = eye.x;
    u[FRAME_AT.camera_pos + 1] = eye.y;
    u[FRAME_AT.camera_pos + 2] = eye.z;
    this.device.queue.writeBuffer(this.light_frame, 0, u);
  }

  /** Opens the depth pass over the map; the caller draws into it and ends it. */
  begin(encoder: GPUCommandEncoder): GPURenderPassEncoder {
    return encoder.beginRenderPass({
      label: "shadow",
      colorAttachments: [],
      depthStencilAttachment: { view: this.map.createView(), depthLoadOp: "clear", depthClearValue: 1, depthStoreOp: "store" },
    });
  }
}
