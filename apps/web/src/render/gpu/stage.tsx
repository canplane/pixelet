// @pxl/web/render/gpu/stage.tsx

/**
 * The WebGPU canvas, and the frame's uniforms laid out from the kernel's
 * lanes: the stage (crates/pxl/core/src/render/gpu/stage.rs) — the camera,
 * the root frame, the LOD fade and what the settings switch on — and the
 * environment (crates/pxl/core/src/world/environment.rs). Every number here
 * is the kernel's; this converts the projections to WebGPU's depth range,
 * lays the numbers into the renderer's buffers and draws.
 */

import { useEffect, useRef } from "react";

import { mat4, mat4_invert, mat4_mul } from "@xpute/core/kit/mat4.ts";
import { Cmd } from "@pxl/shared/spec/cmd.ts";

import { CORE_PXL } from "@/wasm/core_pool.ts";
import { CORE_BUILD } from "@/wasm/core_build.ts";
import { GpuRenderer } from "./renderer.ts";
import type { BadgeSheet } from "./badges.ts";
import { GL_TO_WGPU } from "./depth.ts";

/** The one renderer, once its device has answered. */
export let gpu_renderer: GpuRenderer | null = null;

/** crates/pxl/core/src/world/environment.rs's ENV_FLOATS. */
const ENV_FLOATS = 56;
/** Where the second light starts in it (ENV_OTHER_LIGHT). */
const ENV_OTHER_LIGHT = 48;

/** crates/pxl/core/src/render/gpu/stage.rs's STAGE_WORDS and STAGE_AT. */
const STAGE_WORDS = 96;
const STAGE_AT = {
  badge_view: 0,
  proj: 16,
  local_view_proj: 32,
  view_matrix: 48,
  eye_local: 64,
  eye: 67,
  translation: 70,
  viewport: 73,
  fov: 75,
  near: 76,
  far: 77,
  fade_start: 78,
  fade_end: 79,
  lod_cell: 80,
  edge_strength: 81,
  low_spec: 82,
  edge_width: 83,
  rim_intensity: 84,
  shadows: 85,
  sky_radius: 86,
} as const;

/** A decor group as the kernel's asset store has it: the renderer's mesh
 * handle and the image it is skinned with, -1 while that loads. */
export type DecorModelRow = { group: number; mesh: number; skin: number };
/** A model the kernel draws skinned, and its image. */
export type ModelSkinRow = { model: number; skin: number };

let env_view: Float32Array | null = null;
let stage_view: Float64Array | null = null;

const $a = mat4();
const $m = mat4();
const $proj = mat4();
const $light_target = { x: 0, y: 0, z: 0 };

function lanes(): { stage: Float64Array; env: Float32Array } {
  const buffer = CORE_PXL!.memory.buffer;
  stage_view ??= new Float64Array(buffer, CORE_BUILD.call(Cmd.WORLD_LANES, null, (_, reply) => reply!.at(0).get<number>()), STAGE_WORDS);
  env_view ??= new Float32Array(buffer, CORE_BUILD.call(Cmd.ENV_LANES, null, (_, reply) => reply!.at(0).get<number>()), ENV_FLOATS);
  return { stage: stage_view, env: env_view };
}

let decor_seen = "";

/**
 * Draws the frame the kernel's turn left: the decor groups' models and skins
 * told to the renderer when they changed, the badge sheet mirrored, the
 * uniforms laid out, and the lists drawn.
 */
export function gpu_frame(dpr: number, sheet: BadgeSheet, decor: readonly DecorModelRow[], skins: readonly ModelSkinRow[], image_of: (handle: number) => ImageBitmap | null): void {
  const gpu = gpu_renderer;
  if (!gpu) return;
  const decor_key = decor.map((d) => `${d.group}:${d.mesh}:${d.skin}:${d.skin >= 0 && image_of(d.skin) !== null}`).join(",");
  if (decor_key !== decor_seen) {
    decor_seen = decor_key;
    for (const d of decor) gpu.decor.set_model(d.group, d.mesh, d.skin >= 0 ? image_of(d.skin) : null);
  }
  for (const s of skins) {
    const image = image_of(s.skin);
    if (image) gpu.skinned.set_skin(s.model, image);
  }
  gpu.badges.sync_sheet(sheet);

  const { stage: s, env } = lanes();
  const width = s[STAGE_AT.viewport];
  const height = s[STAGE_AT.viewport + 1];
  gpu.resize(width * dpr, height * dpr);
  $a.set(s.subarray(STAGE_AT.proj, STAGE_AT.proj + 16));
  mat4_mul($proj, GL_TO_WGPU, $a);
  // The badges take the view and the projection apart: their size is
  // measured in view space.
  const badges = gpu.badges;
  badges.view.set(s.subarray(STAGE_AT.badge_view, STAGE_AT.badge_view + 16));
  badges.proj.set($proj);
  badges.px_k = height / (2 * Math.tan((s[STAGE_AT.fov] * Math.PI) / 360));
  badges.fade_start = s[STAGE_AT.fade_start];
  badges.fade_end = s[STAGE_AT.fade_end];
  badges.near = s[STAGE_AT.near];
  badges.far = s[STAGE_AT.far];
  badges.cell = s[STAGE_AT.lod_cell];

  const u = gpu.uniforms;
  const at = gpu.frame_at;
  $a.set(s.subarray(STAGE_AT.local_view_proj, STAGE_AT.local_view_proj + 16));
  u.set(mat4_mul($m, GL_TO_WGPU, $a), at.view_proj);
  // The camera's position in the root's frame, where the meshes are.
  u.set(s.subarray(STAGE_AT.eye_local, STAGE_AT.eye_local + 3), at.camera_pos);
  u[at.edge_strength] = s[STAGE_AT.edge_strength];
  u.set(env.subarray(0, 24), at.env);
  u.set(env.subarray(ENV_OTHER_LIGHT, ENV_OTHER_LIGHT + 8), at.other);
  u[at.low_spec] = s[STAGE_AT.low_spec];
  u[at.edge_width] = s[STAGE_AT.edge_width];
  u[at.rim_intensity] = s[STAGE_AT.rim_intensity];
  u[at.viewport] = gpu.canvas.width;
  u[at.viewport + 1] = gpu.canvas.height;
  u[at.vignette] = 1;
  // The sun's shadow, over the map centre — the root frame's origin undone —
  // from the light the environment lit the terrain with.
  gpu.shadows = s[STAGE_AT.shadows] !== 0;
  u[at.shadow_strength] = gpu.shadows ? 1 : 0;
  u[at.shadow_texel] = gpu.shadow.texel;
  if (gpu.shadows) {
    $light_target.x = -s[STAGE_AT.translation];
    $light_target.y = -s[STAGE_AT.translation + 1];
    $light_target.z = -s[STAGE_AT.translation + 2];
    gpu.shadow.aim(env.subarray(0, 3), $light_target);
    u.set(gpu.shadow.light_view_proj, at.light_view_proj);
  }

  // The sky, in the view frame, where the dome stood.
  const sky = gpu.sky;
  $a.set(s.subarray(STAGE_AT.view_matrix, STAGE_AT.view_matrix + 16));
  sky.set(mat4_invert($m, mat4_mul($m, $proj, $a)), 0);
  sky.set(s.subarray(STAGE_AT.eye, STAGE_AT.eye + 3), 16);
  sky[19] = s[STAGE_AT.sky_radius];
  sky.set(env.subarray(24, 48), 20);
  gpu.frame();
}

/** The canvas, and the renderer once the device answers. `on_canvas` is told
 * the element when it mounts, for the frame loop and the pointer; `on_ready`
 * when a renderer exists to draw a frame. */
export function GpuCanvas({ on_canvas, on_ready, msaa }: { on_canvas: (canvas: HTMLCanvasElement | null) => void; on_ready: () => void; msaa: boolean }) {
  const ref = useRef<HTMLCanvasElement>(null);
  // MSAA is the renderer's targets and every pipeline's sample count: a
  // change is a new renderer over the same canvas.
  useEffect(() => {
    const canvas = ref.current;
    on_canvas(canvas);
    if (!canvas || CORE_PXL === null) return;
    let alive = true;
    let made: GpuRenderer | null = null;
    GpuRenderer.create(canvas, CORE_PXL, { msaa }).then((r) => {
      if (!alive) {
        r?.destroy();
        return;
      }
      if (r === null) {
        console.error("[gpu] no WebGPU on this device; nothing can be drawn");
        return;
      }
      made = r;
      gpu_renderer = r;
      decor_seen = "";
      on_ready();
    });
    return () => {
      alive = false;
      gpu_renderer = null;
      made?.destroy();
      on_canvas(null);
    };
  }, [on_canvas, on_ready, msaa]);
  return <canvas ref={ref} className="absolute inset-0 h-full w-full touch-none" />;
}
