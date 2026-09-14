// @pxl/web/render/gpu/stage.tsx

/**
 * The WebGPU canvas, and the frame's uniforms: the camera and the root
 * frame from the view (module/world/view.ts), the environment from the
 * module. Everything is drawn here; three's canvas above it, cleared
 * transparent, draws only the dev gizmos.
 */

import { useEffect, useRef } from "react";

import { mat4, mat4_invert, mat4_mul } from "@xpute/core/kit/mat4.ts";

import { useSettingsStore } from "@/app/setting/store.ts";
import { CORE_PXL } from "@/render/layer/pipeline/core_pool.ts";
import { fade_end_u, fade_start_u, LOD_CELL_PER_DISTANCE } from "@/render/lod.ts";
import { billboard_sheet } from "@/module/world/billboard/sheet.ts";
import { view } from "@/module/world/view.ts";
import { decor_bundle_assets, type DecorBundleAssets } from "@/module/world/decor/asset_store.ts";
import { GpuRenderer } from "./renderer.ts";

/** The one renderer, once its device has answered. */
export let gpu_renderer: GpuRenderer | null = null;

/** The sky dome's radius in environment.tsx, which the sky's rays meet. */
let sky_radius = 1;
export function set_sky_radius(r: number): void {
  sky_radius = r;
}

const EDGE_STRENGTH = 0.3;
const EDGE_WIDTH = 0.03;
/** crates/pxl-core/src/environment.rs's ENV_FLOATS. */
const ENV_FLOATS = 48;

/** GL clip z is [-1, 1]; WebGPU's is [0, 1]. */
const GL_TO_WGPU = new Float64Array([1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0.5, 0, 0, 0, 0.5, 1]);
const $m = mat4();
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
  u.set(mat4_mul($m, GL_TO_WGPU, view.local_view_proj), 0);
  // The camera's position in the root's frame, where the meshes are.
  u[16] = view.eye_local.x;
  u[17] = view.eye_local.y;
  u[18] = view.eye_local.z;
  const settings = useSettingsStore.getState();
  u[19] = settings.show_edge_shade ? EDGE_STRENGTH : 0;
  // The light, the fog and the sky's colours and directions are the
  // module's (crates/pxl-core/src/environment.rs), laid out as these
  // uniforms take them from their twentieth float.
  const pxl = CORE_PXL!;
  const env = new Float32Array(pxl.memory.buffer, pxl.pxl_env_at(), ENV_FLOATS);
  u.set(env.subarray(0, 24), 20);
  u[23] = settings.texture_pack ? 0 : 1;
  u[39] = EDGE_WIDTH;

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

/** The canvas under three's; the renderer once the device answers. */
export function GpuCanvas() {
  const ref = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    const canvas = ref.current;
    if (!canvas || CORE_PXL === null) return;
    let alive = true;
    GpuRenderer.create(canvas, CORE_PXL).then((r) => {
      if (!alive) return;
      if (r === null) {
        console.error("[gpu] no WebGPU on this device; nothing can be drawn");
        return;
      }
      gpu_renderer = r;
    });
    return () => {
      alive = false;
      gpu_renderer = null;
    };
  }, []);
  return <canvas ref={ref} className="absolute inset-0 h-full w-full" style={{ pointerEvents: "none", zIndex: 0 }} />;
}
