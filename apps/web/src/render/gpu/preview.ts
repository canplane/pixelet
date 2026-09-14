// @pxl/web/render/gpu/preview.ts

/**
 * A display case: one of the module's actors drawn alone into a small
 * canvas of its own, under the main frame's light, by the same skinned
 * pass the world uses. The actor stands somewhere the world never looks —
 * far below it — and the case's view puts that place at its own origin,
 * so the model fills the case whatever the map is showing.
 *
 * Drawn as part of the renderer's frame, after the world's pass, from the
 * same lists: the renderer keeps the cases open and hands each the draws.
 */

import type { f64, u32 } from "@xpute/core/kit/word.ts";
import { mat4, mat4_look_at, mat4_mul, mat4_perspective, mat4_translation } from "@xpute/core/kit/mat4.ts";

import { type ModelSkin, SkinnedPass } from "./skinned.ts";
import type { FrameUniforms } from "./renderer.ts";
import type { FrameLayout, PipelineBuilder } from "./pipeline.ts";
import { DEPTH_CLEAR, DEPTH_FORMAT, GL_TO_WGPU } from "./depth.ts";

const DRAW_WORDS = 5;
const UNIFORM_FLOATS = 8;
const PIPELINE_SKINNED = 5;

const $proj = mat4();
const $view = mat4();
const $t = mat4();
const $m = mat4();

export class PreviewCase {
  private readonly context: GPUCanvasContext;
  private readonly frame_buffer: GPUBuffer;
  private readonly pass: SkinnedPass;
  private readonly uniforms: FrameUniforms;
  private depth: GPUTexture | null = null;
  private width = 0;
  private height = 0;
  /** The model the case shows, and only it; 0 shows nothing. */
  model: u32 = 0;
  /** The eye and what it looks at, in the case's own Z-up frame. */
  eye = { x: 0, y: -2.2, z: 0.9 };
  target = { x: 0, y: 0, z: 0.55 };
  fov: f64 = 35;

  constructor(private readonly device: GPUDevice, pipelines: PipelineBuilder, private readonly at: FrameLayout, readonly canvas: HTMLCanvasElement, format: GPUTextureFormat) {
    const context = canvas.getContext("webgpu");
    if (!context) throw new Error("gpu: the preview canvas has no webgpu context");
    this.context = context;
    context.configure({ device, format, alphaMode: "premultiplied" });
    this.uniforms = new Float32Array(new ArrayBuffer(at.floats * 4));
    this.frame_buffer = device.createBuffer({ label: "preview/frame", size: at.floats * 4, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST });
    this.pass = new SkinnedPass(device, pipelines, format, this.frame_buffer, null, 1);
  }

  set_skin(model: u32, image: ModelSkin): void {
    this.pass.set_skin(model, image);
  }

  /** The canvas's size in device pixels. */
  resize(width: u32, height: u32): void {
    width = Math.max(1, Math.floor(width));
    height = Math.max(1, Math.floor(height));
    if (width === this.width && height === this.height) return;
    this.width = width;
    this.height = height;
    this.canvas.width = width;
    this.canvas.height = height;
    this.depth?.destroy();
    this.depth = this.device.createTexture({ label: "preview/depth", size: [width, height], format: DEPTH_FORMAT, usage: GPUTextureUsage.RENDER_ATTACHMENT });
  }

  /** Draws the case's model from the frame's lists, lit as `frame` lights
   * the world; nothing when the model has no draw this frame. */
  draw(encoder: GPUCommandEncoder, frame: FrameUniforms, draws: Uint32Array, uniforms: Float32Array, buffers: ReadonlyMap<u32, GPUBuffer>, counts: ReadonlyMap<u32, [u32, u32]>): void {
    if (!this.depth || this.model === 0) return;
    let own: Uint32Array | null = null;
    let at = 0;
    for (let k = 0; k < draws.length; k += DRAW_WORDS) {
      if (draws[k + 4] !== PIPELINE_SKINNED || draws[k + 2] !== this.model) continue;
      own = draws.subarray(k, k + DRAW_WORDS);
      at = draws[k + 3] * UNIFORM_FLOATS;
      break;
    }
    if (!own) return;
    // The case's view, with the actor's place in the world taken back out
    // so it stands at the case's origin; the eye where the fog reads it.
    const u = this.uniforms;
    const f = this.at;
    u.set(frame);
    mat4_perspective($proj, (this.fov * Math.PI) / 180, this.width / this.height, 0.1, 100);
    mat4_look_at($view, this.eye, this.target, { x: 0, y: 0, z: 1 });
    mat4_translation($t, -uniforms[at], -uniforms[at + 1], -uniforms[at + 2]);
    u.set(mat4_mul($m, mat4_mul($m, mat4_mul($m, GL_TO_WGPU, $proj), $view), $t), f.view_proj);
    u[f.camera_pos] = this.eye.x + uniforms[at];
    u[f.camera_pos + 1] = this.eye.y + uniforms[at + 1];
    u[f.camera_pos + 2] = this.eye.z + uniforms[at + 2];
    // No fog: the near and far pushed out of reach (environment.rs's
    // fog_near and fog_far in the run).
    u[f.env + 7] = 1e9;
    u[f.env + 11] = 2e9;
    u[f.viewport] = this.width;
    u[f.viewport + 1] = this.height;
    u[f.shadow_strength] = 0;
    u[f.vignette] = 0;
    this.device.queue.writeBuffer(this.frame_buffer, 0, u);
    const pass = encoder.beginRenderPass({
      label: "preview",
      colorAttachments: [{ view: this.context.getCurrentTexture().createView(), loadOp: "clear", clearValue: { r: 0, g: 0, b: 0, a: 0 }, storeOp: "store" }],
      depthStencilAttachment: { view: this.depth.createView(), depthLoadOp: "clear", depthClearValue: DEPTH_CLEAR, depthStoreOp: "store" },
    });
    this.pass.draw(pass, own, uniforms, buffers, counts);
    pass.end();
  }
}
