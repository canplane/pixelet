// @pxl/web/render/gpu/stage.tsx

/**
 * The WebGPU canvas, and the frame's uniforms: the camera and the root
 * frame from the view (module/world/frame/view.ts), the environment from the
 * module. Everything is drawn here.
 */

import { useEffect, useRef } from "react";

import { mat4, mat4_invert, mat4_mul } from "@xpute/core/kit/mat4.ts";

import { useSettingsStore } from "@/app/setting/store.ts";
import { CORE_PXL } from "@/wasm/core_pool.ts";
import { fade_end_u, fade_start_u, LOD_CELL_PER_DISTANCE } from "@/module/world/lod.ts";
import { billboard_sheet } from "@/module/world/billboard/sheet.ts";
import { request_world_tick } from "@/module/world/change.ts";
import { view } from "@/module/world/frame/view.ts";
import { decor_bundle_assets, type DecorBundleAssets } from "@/module/world/decor/asset_store.ts";
import { GpuRenderer } from "./renderer.ts";
import { GL_TO_WGPU } from "./depth.ts";
import { rim_cfg } from "@/module/world/environment/config.ts";

/** The one renderer, once its device has answered. */
export let gpu_renderer: GpuRenderer | null = null;

/** The sky dome's radius in environment.tsx, which the sky's rays meet. */
let sky_radius = 1;
/** The environment's output, at a fixed address in the module: one view. */
let env_view: Float32Array | null = null;
export function set_sky_radius(r: number): void {
  sky_radius = r;
}

const EDGE_STRENGTH = 0.3;
const EDGE_WIDTH = 0.03;
/** crates/pxl/core/src/world/environment.rs's ENV_FLOATS. */
const ENV_FLOATS = 56;
/** Where the second light starts in it (ENV_OTHER_LIGHT). */
const ENV_OTHER_LIGHT = 48;

const $m = mat4();
const $light_target = { x: 0, y: 0, z: 0 };
const $proj = mat4();

/** decor/asset_store.ts's models and skins by group, told to the renderer
 * when the assets change. */
let decor_assets_seen: readonly DecorBundleAssets[] | null = null;

function sync_decor_models(gpu: GpuRenderer): void {
  const assets = decor_bundle_assets();
  if (assets === decor_assets_seen) return;
  decor_assets_seen = assets;
  const pxl = CORE_PXL!;
  let group = 0;
  for (const { models, skins } of assets) {
    models.forEach((model, v) => {
      gpu.decor.set_model(group++, model ? pxl.pxl_model_handle(model.model) : 0, skins[v]);
    });
  }
}

/**
 * Writes the frame's uniforms and draws: called from the chunk stage's
 * visible step after the module's tick, from the view as it stands.
 */
export function gpu_frame(dpr: number): void {
  const gpu = gpu_renderer;
  if (!gpu) return;
  sync_decor_models(gpu);
  gpu.badges.sync_sheet(billboard_sheet);
  gpu.resize(view.width * dpr, view.height * dpr);
  mat4_mul($proj, GL_TO_WGPU, view.proj);
  // The badges take the view and the projection apart: their size is
  // measured in view space.
  const badges = gpu.badges;
  badges.view.set(mat4_mul($m, view.view_matrix, view.root));
  badges.proj.set($proj);
  badges.px_k = view.height / (2 * Math.tan((view.fov * Math.PI) / 360));
  badges.fade_start = fade_start_u();
  badges.fade_end = fade_end_u();
  badges.near = view.near;
  badges.far = view.far;
  badges.cell = LOD_CELL_PER_DISTANCE.value;
  const u = gpu.uniforms;
  const at = gpu.frame_at;
  u.set(mat4_mul($m, GL_TO_WGPU, view.local_view_proj), at.view_proj);
  // The camera's position in the root's frame, where the meshes are.
  u[at.camera_pos] = view.eye_local.x;
  u[at.camera_pos + 1] = view.eye_local.y;
  u[at.camera_pos + 2] = view.eye_local.z;
  const settings = useSettingsStore.getState();
  u[at.edge_strength] = settings.show_edge_shade ? EDGE_STRENGTH : 0;
  // The light, the fog and the sky's colours and directions are the
  // module's (crates/pxl/core/src/world/environment.rs), laid out as these
  // uniforms take them.
  const pxl = CORE_PXL!;
  env_view ??= new Float32Array(pxl.memory.buffer, pxl.pxl_env_at(), ENV_FLOATS);
  const env = env_view;
  u.set(env.subarray(0, 24), at.env);
  u.set(env.subarray(ENV_OTHER_LIGHT, ENV_OTHER_LIGHT + 8), at.other);
  u[at.low_spec] = settings.texture_pack ? 0 : 1;
  u[at.edge_width] = EDGE_WIDTH;
  u[at.rim_intensity] = settings.show_rim_light ? rim_cfg.INTENSITY : 0;
  u[at.viewport] = gpu.canvas.width;
  u[at.viewport + 1] = gpu.canvas.height;
  u[at.vignette] = 1;
  // The sun's shadow, over the map centre — the root frame's origin
  // undone — from the light the environment lit the terrain with.
  gpu.shadows = settings.show_shadow_map;
  u[at.shadow_strength] = gpu.shadows ? 1 : 0;
  u[at.shadow_texel] = gpu.shadow.texel;
  if (gpu.shadows) {
    $light_target.x = -view.tx;
    $light_target.y = -view.ty;
    $light_target.z = -view.tz;
    gpu.shadow.aim(env.subarray(0, 3), $light_target);
    u.set(gpu.shadow.light_view_proj, at.light_view_proj);
  }

  // The sky, in the view frame, where the dome stood.
  const sky = gpu.sky;
  sky.set(mat4_invert($m, mat4_mul($m, $proj, view.view_matrix)), 0);
  sky[16] = view.eye.x;
  sky[17] = view.eye.y;
  sky[18] = view.eye.z;
  sky[19] = sky_radius;
  sky.set(env.subarray(24, 48), 20);
  gpu.frame();
}

/** The canvas, and the renderer once the device answers. `on_canvas` is
 * told the element when it mounts, for the frame loop and the pointer. */
export function GpuCanvas({ on_canvas }: { on_canvas: (canvas: HTMLCanvasElement | null) => void }) {
  const ref = useRef<HTMLCanvasElement>(null);
  // MSAA is the renderer's targets and every pipeline's sample count: a
  // change is a new renderer over the same canvas.
  const msaa = useSettingsStore((state) => state.msaa);
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
      decor_assets_seen = null;
      request_world_tick();
    });
    return () => {
      alive = false;
      gpu_renderer = null;
      made?.destroy();
      on_canvas(null);
    };
  }, [on_canvas, msaa]);
  return <canvas ref={ref} className="absolute inset-0 h-full w-full touch-none" />;
}
